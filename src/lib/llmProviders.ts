import type { ChatMessage } from "./api";
import { redact } from "./redact";

// Use Tauri's fetch (bypasses CORS) when available, fall back to native fetch in browser mode.
// The lazy import avoids a crash when @tauri-apps/plugin-http is not available.
let _fetchFn: typeof globalThis.fetch | null = null;

async function getFetch(): Promise<typeof globalThis.fetch> {
  if (_fetchFn) return _fetchFn;
  try {
    // @ts-ignore — dynamic import may fail in browser mode
    const mod = await import("@tauri-apps/plugin-http");
    _fetchFn = mod.fetch as unknown as typeof globalThis.fetch;
  } catch {
    _fetchFn = globalThis.fetch.bind(globalThis);
  }
  return _fetchFn;
}

export async function chatStream(
  provider: string,
  model: string,
  apiKey: string,
  messages: ChatMessage[],
  endpoint: string,
  onChunk: (text: string) => void,
  signal?: AbortSignal
): Promise<void> {
  const isAnthropic = provider === "anthropic";
  const isGemini = provider === "gemini";
  const isOllama = provider === "ollama-local" || provider === "ollama-cloud";
  const isOpenAI = provider === "openai";

  let url = "";
  let headers: Record<string, string> = { "Content-Type": "application/json" };
  let body: any = {};

  if (isOllama) {
    url = endpoint ? endpoint : "http://localhost:11434";
    url = url.replace(/\/+$/, "") + "/api/chat";
    if (apiKey) headers["Authorization"] = `Bearer ${apiKey}`;
    body = { model, messages, stream: true };
  } else if (isAnthropic) {
    url = "https://api.anthropic.com/v1/messages";
    headers["x-api-key"] = apiKey;
    headers["anthropic-version"] = "2023-06-01";
    headers["anthropic-dangerous-direct-browser-access"] = "true";
    let sys = messages.filter(m => m.role === "system").map(m => m.content).join("\n\n");
    let msg = messages.filter(m => m.role !== "system");
    body = { model: model || "claude-3-haiku-20240307", system: sys, messages: msg, max_tokens: 4096, stream: true };
  } else if (isOpenAI) {
    url = endpoint ? endpoint : "https://api.openai.com/v1/chat/completions";
    headers["Authorization"] = `Bearer ${apiKey}`;
    body = { model: model || "gpt-3.5-turbo", messages, stream: true };
  } else if (isGemini) {
    let m = model || "gemini-1.5-flash";
    // Key in a header, not the query string: a failing request surfaces its URL
    // in the error message, which the user then copies into a bug report.
    url = `https://generativelanguage.googleapis.com/v1beta/models/${m}:streamGenerateContent`;
    headers["x-goog-api-key"] = apiKey;
    let contents = messages.filter(m => m.role !== "system").map(m => ({
      role: m.role === "assistant" ? "model" : "user",
      parts: [{ text: m.content }]
    }));
    let sys = messages.filter(m => m.role === "system").map(m => m.content).join("\n\n");
    if (sys) {
      body = { system_instruction: { parts: { text: sys } }, contents };
    } else {
      body = { contents };
    }
  } else {
    throw new Error(`Unknown provider: ${provider}`);
  }

  const fetchFn = await getFetch();
  const response = await fetchFn(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal,
  });

  if (!response.ok) {
    // The body is redacted because a provider can echo the credential back.
    throw new Error(redact(`HTTP Error ${response.status}: ${await response.text()}`));
  }

  if (!response!.ok) {
    throw new Error(`HTTP Error ${response!.status}: ${await response!.text()}`);
  }

  if (!response!.body) throw new Error("No response body");

  const reader = response!.body!.getReader();
  const decoder = new TextDecoder("utf-8");

  if (isGemini) {
    // Gemini stream format is an array of objects but chunks might be split
    let buffer = "";
    while (true) {
      // Checked per chunk, not only on the fetch: Tauri's HTTP plugin can have
      // the body in flight already, so a cancelled request would otherwise keep
      // streaming text into a message the user has stopped.
      if (signal?.aborted) {
        await reader.cancel().catch(() => {});
        break;
      }
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      // Very naive json extraction for gemini stream which looks like: ",\r\n{\n  \"candidates\":..."
      // But it's an array stream `[\n{\n...}\n,\n{\n...}\n]`
      // We will just regex match text fields for simplicity, or parse correctly.
      // A safe way for Gemini streaming JSON: extract "text": "..."
      const matches = [...buffer.matchAll(/"text"\s*:\s*"((?:[^"\\]|\\.)*)"/g)];
      if (matches.length > 0) {
        let chunkText = "";
        for (const m of matches) {
           chunkText += m[1].replace(/\\n/g, "\n").replace(/\\"/g, "\"").replace(/\\\\/g, "\\");
        }
        if (chunkText) {
          onChunk(chunkText);
        }
        // clear buffer to avoid reprocessing but we must be careful not to break mid-string
        buffer = ""; // Highly naive, but works if we just want all text matches and clear.
      }
    }
  } else {
    // SSE Format for OpenAI, Anthropic, Ollama
    let buffer = "";
    while (true) {
      // Checked per chunk, not only on the fetch: Tauri's HTTP plugin can have
      // the body in flight already, so a cancelled request would otherwise keep
      // streaming text into a message the user has stopped.
      if (signal?.aborted) {
        await reader.cancel().catch(() => {});
        break;
      }
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() || ""; // Keep incomplete line in buffer

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        if (isOllama) {
          try {
            const data = JSON.parse(trimmed);
            if (data.message?.content) onChunk(data.message.content);
          } catch (e) {}
        } else if (trimmed.startsWith("data: ")) {
          const dataStr = trimmed.slice(6);
          if (dataStr === "[DONE]") continue;
          try {
            const data = JSON.parse(dataStr);
            if (isOpenAI) {
              const content = data.choices?.[0]?.delta?.content;
              if (content) onChunk(content);
            } else if (isAnthropic) {
              if (data.type === "content_block_delta" && data.delta?.text) {
                onChunk(data.delta.text);
              }
            }
          } catch (e) {}
        }
      }
    }
  }
}
