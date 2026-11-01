import type { ChatMessage } from "./api";
import { redact } from "./redact";
import { normalizeOllamaBaseUrl, normalizeOpenAiBaseUrl } from "./aiProviderConfig";

// Use Tauri's fetch (bypasses CORS) when available, fall back to native fetch in browser mode.
// The lazy import avoids a crash when @tauri-apps/plugin-http is not available.
let _fetchFn: typeof globalThis.fetch | null = null;

async function getFetch(): Promise<typeof globalThis.fetch> {
  if (_fetchFn) return _fetchFn;
  try {
    const mod = await import("@tauri-apps/plugin-http");
    _fetchFn = mod.fetch as unknown as typeof globalThis.fetch;
  } catch {
    _fetchFn = globalThis.fetch.bind(globalThis);
  }
  return _fetchFn;
}

export const DEFAULT_MODELS = {
  anthropic: "claude-3-5-sonnet-latest",
  openai: "gpt-4o-mini",
  gemini: "gemini-2.0-flash",
  "ollama-local": "llama3",
  "ollama-cloud": "llama3",
  "openai-compatible": "gpt-4o-mini",
} as const;

export const POPULAR_MODELS = {
  anthropic: ["claude-3-5-sonnet-latest", "claude-3-7-sonnet-latest"],
  openai: ["gpt-4o", "gpt-4o-mini"],
  gemini: ["gemini-2.0-flash"],
} as const;

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
  // Any gateway that speaks the OpenAI wire format — OpenRouter, Groq, vLLM,
  // LM Studio — differs from OpenAI only in its base URL.
  const isOpenAI = provider === "openai" || provider === "openai-compatible";

  let url: string;
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  let body: Record<string, unknown>;

  if (isOllama) {
    // Normalized so a host pasted straight from Ollama's docs — which quote the
    // full `/api/chat` URL — does not end up with the path twice.
    const base = endpoint ? normalizeOllamaBaseUrl(endpoint) : "http://localhost:11434";
    url = `${base}/api/chat`;
    if (apiKey) headers["Authorization"] = `Bearer ${apiKey}`;
    body = { model: model || DEFAULT_MODELS["ollama-local"], messages, stream: true };
  } else if (isAnthropic) {
    url = "https://api.anthropic.com/v1/messages";
    headers["x-api-key"] = apiKey;
    headers["anthropic-version"] = "2023-06-01";
    headers["anthropic-dangerous-direct-browser-access"] = "true";
    const sys = messages.filter(m => m.role === "system").map(m => m.content).join("\n\n");
    const msg = messages.filter(m => m.role !== "system");
    body = { model: model || DEFAULT_MODELS.anthropic, system: sys, messages: msg, max_tokens: 4096, stream: true };
  } else if (isOpenAI) {
    // Normalized to a base here and by the same rules on the Rust side, so one
    // value configured in settings works for both the streaming and the
    // non-streaming path no matter which form the user pasted.
    const base = endpoint ? normalizeOpenAiBaseUrl(endpoint) : "https://api.openai.com/v1";
    url = `${base}/chat/completions`;
    if (apiKey) headers["Authorization"] = `Bearer ${apiKey}`;
    body = { model: model || DEFAULT_MODELS.openai, messages, stream: true };
  } else if (isGemini) {
    const m = model || DEFAULT_MODELS.gemini;
    // Key in a header, not the query string: a failing request surfaces its URL
    // in the error message, which the user then copies into a bug report.
    url = `https://generativelanguage.googleapis.com/v1beta/models/${m}:streamGenerateContent?alt=sse`;
    headers["x-goog-api-key"] = apiKey;
    const contents = messages.filter(m => m.role !== "system").map(m => ({
      role: m.role === "assistant" ? "model" : "user",
      parts: [{ text: m.content }]
    }));
    const sys = messages.filter(m => m.role === "system").map(m => m.content).join("\n\n");
    if (sys) {
      body = { system_instruction: { parts: { text: sys } }, contents };
    } else {
      body = { contents };
    }
  } else {
    throw new Error(`Unknown provider: ${provider}`);
  }

  const fetchFn = await getFetch();

  // A wrong Endpoint URL surfaces from fetch as a bare `TypeError: Failed to
  // fetch`, which names neither the provider nor the setting that produced it.
  let response: Response;
  try {
    response = await fetchFn(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      signal,
    });
  } catch (networkErr) {
    // An abort is the user pressing Stop, not a misconfiguration. Rethrow it
    // untouched so the caller's abort handling still recognises it.
    if (signal?.aborted) throw networkErr;
    // The query string is dropped and the result redacted: a user-pasted
    // endpoint can carry a key, and this message ends up in bug reports.
    const safeUrl = url.replace(/\?.*$/, "");
    throw new Error(
      redact(
        `Could not reach ${provider} at ${safeUrl}. ` +
        `Check the Endpoint URL in Settings. ` +
        `(${networkErr instanceof Error ? networkErr.message : String(networkErr)})`
      ),
      { cause: networkErr }
    );
  }

  if (!response.ok) {
    // The body is redacted because a provider can echo the credential back.
    throw new Error(redact(`HTTP Error ${response.status}: ${await response.text()}`));
  }

  if (!response.body) throw new Error("No response body");

  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8");

  let buffer = "";
  let inThinking = false;

  const handleThinking = (text: string) => {
    if (!text) return;
    const clean = text.replace(/<\/?think>/gi, "");
    if (!clean) return;
    if (!inThinking) {
      onChunk("<think>");
      inThinking = true;
    }
    onChunk(clean);
  };

  const handleContent = (text: string) => {
    if (!text) return;
    if (inThinking) {
      onChunk("</think>\n\n");
      inThinking = false;
    }
    onChunk(text);
  };

  const processLine = (trimmed: string) => {
    if (!trimmed) return;

    if (isOllama) {
      let dataStr = trimmed;
      if (dataStr.startsWith("data: ")) {
        dataStr = dataStr.slice(6).trim();
      }
      if (dataStr === "[DONE]") return;
      try {
        const data = JSON.parse(dataStr);
        if (typeof data.message?.thinking === "string" && data.message.thinking) {
          handleThinking(data.message.thinking);
        }
        if (typeof data.message?.content === "string" && data.message.content) {
          handleContent(data.message.content);
        }
      } catch {
        // A non-JSON line is not part of the Ollama stream; ignore it.
      }
    } else if (trimmed.startsWith("data: ")) {
      const dataStr = trimmed.slice(6).trim();
      if (dataStr === "[DONE]") return;
      try {
        const data = JSON.parse(dataStr);
        if (isGemini) {
          const parts = data.candidates?.[0]?.content?.parts;
          if (Array.isArray(parts)) {
            for (const part of parts) {
              if (part.thought) {
                if (typeof part.text === "string" && part.text) {
                  handleThinking(part.text);
                }
              } else if (typeof part.text === "string" && part.text) {
                handleContent(part.text);
              }
            }
          }
        } else if (isOpenAI) {
          const delta = data.choices?.[0]?.delta;
          if (delta) {
            const reasoning = delta.reasoning_content ?? delta.reasoning;
            if (typeof reasoning === "string" && reasoning) {
              handleThinking(reasoning);
            }
            if (typeof delta.content === "string" && delta.content) {
              handleContent(delta.content);
            }
          }
        } else if (isAnthropic) {
          if (data.type === "content_block_delta") {
            const thinking =
              data.delta?.type === "thinking_delta"
                ? data.delta?.thinking
                : (data.delta?.thinking ?? data.delta?.thinking_delta);
            if (typeof thinking === "string" && thinking) {
              handleThinking(thinking);
            }
            const text = data.delta?.text;
            if (typeof text === "string" && text) {
              handleContent(text);
            }
          }
        }
      } catch {
        // SSE keep-alive lines and malformed frames are skipped.
      }
    } else if (isGemini) {
      // Fallback in case a proxy strips alt=sse and returns raw JSON objects
      let jsonStr = trimmed;
      if (jsonStr.startsWith(", {") || jsonStr.startsWith(",{")) {
        jsonStr = jsonStr.slice(jsonStr.indexOf("{"));
      }
      if (jsonStr.startsWith("{")) {
        try {
          const data = JSON.parse(jsonStr);
          const parts = data.candidates?.[0]?.content?.parts;
          if (Array.isArray(parts)) {
            for (const part of parts) {
              if (part.thought) {
                if (typeof part.text === "string" && part.text) {
                  handleThinking(part.text);
                }
              } else if (typeof part.text === "string" && part.text) {
                handleContent(part.text);
              }
            }
          }
        } catch {
          // Incomplete or non-JSON chunk
        }
      }
    }
  };

  while (true) {
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
      processLine(line.trim());
    }
  }

  if (buffer.trim()) {
    processLine(buffer.trim());
  }

  if (inThinking) {
    onChunk("</think>\n\n");
    inThinking = false;
  }
}
