import { useState, useCallback, useRef, useEffect } from "react";
import { aiApi, ChatMessage } from "../lib/api";

const TEMPLATES: Record<string, { label: string; base: string; dockerfile: string }> = {
  node: {
    label: "Node.js",
    base: "node:22-alpine",
    dockerfile: `FROM node:22-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
EXPOSE 3000
CMD ["node", "server.js"]`,
  },
  python: {
    label: "Python",
    base: "python:3.12-slim",
    dockerfile: `FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .
EXPOSE 8000
CMD ["python", "app.py"]`,
  },
  go: {
    label: "Go",
    base: "golang:1.22-alpine",
    dockerfile: `FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /app/server .

FROM alpine:3.19
COPY --from=builder /app/server /server
EXPOSE 8080
CMD ["/server"]`,
  },
  rust: {
    label: "Rust",
    base: "rust:1.77-slim",
    dockerfile: `FROM rust:1.77-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/app /usr/local/bin/app
EXPOSE 8080
CMD ["app"]`,
  },
  nginx: {
    label: "Nginx (Static)",
    base: "nginx:alpine",
    dockerfile: `FROM nginx:alpine
COPY ./dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]`,
  },
  custom: {
    label: "Custom",
    base: "alpine:3.19",
    dockerfile: `FROM alpine:3.19
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY . .
CMD ["/bin/sh"]`,
  },
};

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic" },
  { id: "openai", label: "OpenAI" },
  { id: "gemini", label: "Google Gemini" },
  { id: "ollama-local", label: "Ollama Local" },
  { id: "ollama-cloud", label: "Ollama Cloud" },
];

const SYSTEM_PROMPT = `You are a Dockerfile expert. Help users create, improve, and debug Dockerfiles.
When generating a Dockerfile, output ONLY the Dockerfile content without any markdown code fences or explanations unless the user asks for explanations.
Follow Docker best practices: multi-stage builds, minimal base images, proper layer caching, non-root users, and .dockerignore.`;

interface UiMessage {
  role: "user" | "assistant";
  content: string;
}

export default function DockerfileGen() {
  const [selectedTemplate, setSelectedTemplate] = useState("node");
  const [dockerfile, setDockerfile] = useState(TEMPLATES.node.dockerfile);
  const [notification, setNotification] = useState<{ type: "success" | "error"; text: string } | null>(null);

  // AI Chat state
  const [chatOpen, setChatOpen] = useState(false);
  const [provider, setProvider] = useState(() => localStorage.getItem("ai_provider") || "anthropic");
  const [model, setModel] = useState(() => localStorage.getItem("ai_model") || "");
  const [apiKey, setApiKey] = useState(() => localStorage.getItem("ai_api_key") || "");
  const [endpoint, setEndpoint] = useState(() => localStorage.getItem("ai_endpoint") || "");
  const [chatMessages, setChatMessages] = useState<UiMessage[]>([]);
  const [userInput, setUserInput] = useState("");
  const [aiLoading, setAiLoading] = useState(false);
  const [showConfig, setShowConfig] = useState(false);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [modelsFetching, setModelsFetching] = useState(false);
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Persist settings
  useEffect(() => { localStorage.setItem("ai_provider", provider); }, [provider]);
  useEffect(() => { localStorage.setItem("ai_model", model); }, [model]);
  useEffect(() => { localStorage.setItem("ai_api_key", apiKey); }, [apiKey]);
  useEffect(() => { localStorage.setItem("ai_endpoint", endpoint); }, [endpoint]);
  useEffect(() => { chatEndRef.current?.scrollIntoView({ behavior: "smooth" }); }, [chatMessages]);

  // Dynamic model fetching for all providers
  const fetchModels = useCallback(async (pid: string) => {
    setModelsFetching(true);
    try {
      const raw = await aiApi.listModels(pid, apiKey, endpoint);
      const parsed: string[] = JSON.parse(typeof raw === 'string' ? raw : '[]');
      setAvailableModels([...new Set(parsed)]);
    } catch {
      setAvailableModels([]);
    } finally {
      setModelsFetching(false);
    }
  }, [apiKey, endpoint]);

  // Fetch models when provider changes or config panel opens
  useEffect(() => {
    if (showConfig) fetchModels(provider);
  }, [provider, showConfig, fetchModels]);

  const notify = useCallback((type: "success" | "error", text: string) => {
    setNotification({ type, text });
    setTimeout(() => setNotification(null), 3000);
  }, []);

  const handleTemplateChange = (key: string) => {
    setSelectedTemplate(key);
    setDockerfile(TEMPLATES[key].dockerfile);
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(dockerfile).then(() => notify("success", "Copied to clipboard!"));
  };

  const handleDownload = () => {
    const blob = new Blob([dockerfile], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "Dockerfile";
    a.click();
    URL.revokeObjectURL(url);
    notify("success", "Dockerfile downloaded!");
  };

  const handleProviderChange = (pid: string) => {
    setProvider(pid);
    setModel("");
    fetchModels(pid);
  };

  const sendMessage = async () => {
    const text = userInput.trim();
    if (!text || aiLoading) return;
    if (!apiKey && provider !== "ollama-local") {
      notify("error", "Please configure your API key first");
      setShowConfig(true);
      return;
    }

    const newUserMsg: UiMessage = { role: "user", content: text };
    setChatMessages(prev => [...prev, newUserMsg]);
    setUserInput("");
    setAiLoading(true);

    try {
      // Include current Dockerfile as context
      const contextMsg = `Current Dockerfile:\n\`\`\`dockerfile\n${dockerfile}\n\`\`\``;
      const messages: ChatMessage[] = [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content: contextMsg },
        ...chatMessages.map(m => ({ role: m.role, content: m.content } as ChatMessage)),
        { role: "user" as const, content: text },
      ];

      const response = await aiApi.chat(provider, model, apiKey, messages, endpoint);
      const aiMsg: UiMessage = { role: "assistant", content: typeof response === 'string' ? response : String(response) };
      setChatMessages(prev => [...prev, aiMsg]);
    } catch (e) {
      const errMsg: UiMessage = { role: "assistant", content: `Error: ${e}` };
      setChatMessages(prev => [...prev, errMsg]);
    } finally {
      setAiLoading(false);
    }
  };

  const applyToEditor = (content: string) => {
    // Extract Dockerfile from response (remove markdown fences if present)
    let df = content;
    const fenceMatch = df.match(/```(?:dockerfile)?\n([\s\S]*?)```/);
    if (fenceMatch) df = fenceMatch[1];
    // If content looks like a Dockerfile (starts with FROM), use it directly
    if (df.trim().startsWith("FROM") || df.trim().startsWith("#")) {
      setDockerfile(df.trim());
      notify("success", "Applied to editor!");
    } else {
      notify("error", "Could not extract Dockerfile from response");
    }
  };

  const currentProvider = PROVIDERS.find(p => p.id === provider);
  const modelOptions = availableModels;
  const lineCount = dockerfile.split("\n").length;

  return (
    <>
      <div className="content-header">
        <h1>Dockerfile Generator</h1>
        <div className="content-header-actions" style={{ display: "flex", gap: 8 }}>
          <button className={`btn ${chatOpen ? "btn-primary" : "btn-ghost"}`}
            onClick={() => setChatOpen(!chatOpen)} style={{ gap: 6 }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
            </svg>
            AI Chat
          </button>
          <button className="btn btn-ghost" onClick={handleCopy}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
            </svg>
            Copy
          </button>
          <button className="btn btn-ghost" onClick={handleDownload}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            Download
          </button>
        </div>
      </div>

      <div className="content-body">
        {notification && (
          <div style={{
            position: "fixed", top: 16, right: 16, padding: "10px 16px",
            borderRadius: "var(--radius-md)",
            background: notification.type === "success" ? "rgba(63,185,80,0.15)" : "rgba(248,81,73,0.15)",
            border: `1px solid ${notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)"}`,
            color: notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            fontSize: "var(--text-sm)", zIndex: 200, boxShadow: "var(--shadow-lg)",
          }}>
            {notification.type === "success" ? "✓" : "✕"} {notification.text}
          </div>
        )}

        {/* Template Selector */}
        <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap" }}>
          {Object.entries(TEMPLATES).map(([key, tmpl]) => (
            <button key={key} className="btn" onClick={() => handleTemplateChange(key)} style={{
              background: selectedTemplate === key ? "rgba(88,166,255,0.15)" : "var(--bg-secondary)",
              border: `1px solid ${selectedTemplate === key ? "var(--accent-blue)" : "var(--border-primary)"}`,
              color: selectedTemplate === key ? "var(--accent-blue)" : "var(--text-secondary)",
              fontWeight: selectedTemplate === key ? 600 : 400,
              borderRadius: 8, padding: "8px 14px", fontSize: "var(--text-sm)",
            }}>
              {tmpl.label}
            </button>
          ))}
        </div>

        <div style={{ display: "flex", gap: 16 }}>
          {/* Editor */}
          <div style={{ flex: chatOpen ? "1 1 55%" : "1 1 100%", minWidth: 0, transition: "flex 300ms" }}>
            <div className="card" style={{ padding: 0, overflow: "hidden" }}>
              <div style={{
                display: "flex", justifyContent: "space-between", alignItems: "center",
                padding: "8px 16px", borderBottom: "1px solid var(--border-primary)",
                background: "rgba(0,0,0,0.15)",
              }}>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                  Dockerfile — {lineCount} lines
                </span>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--accent-blue)" }}>
                  {TEMPLATES[selectedTemplate].base}
                </span>
              </div>
              <div style={{ display: "flex", background: "var(--bg-primary)" }}>
                <div style={{
                  padding: "12px 8px", textAlign: "right", userSelect: "none",
                  borderRight: "1px solid var(--border-primary)", minWidth: 40,
                  fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-muted)",
                  lineHeight: "1.6",
                }}>
                  {Array.from({ length: lineCount }, (_, i) => (
                    <div key={i}>{i + 1}</div>
                  ))}
                </div>
                <textarea
                  value={dockerfile}
                  onChange={e => setDockerfile(e.target.value)}
                  spellCheck={false}
                  style={{
                    flex: 1, padding: "12px 16px", minHeight: chatOpen ? 300 : 400,
                    background: "transparent", border: "none", outline: "none", resize: "vertical",
                    fontFamily: "var(--font-mono)", fontSize: "13px", color: "var(--text-primary)",
                    lineHeight: "1.6", whiteSpace: "pre", tabSize: 2,
                  }}
                />
              </div>
            </div>

            {/* Best Practices */}
            {!chatOpen && (
              <div className="card" style={{ marginTop: 16 }}>
                <h3 style={{ fontSize: "var(--text-md)", fontWeight: 600, marginBottom: 12 }}>Best Practices</h3>
                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 12 }}>
                  {[
                    { icon: "📦", title: "Multi-stage builds", desc: "Smaller final images" },
                    { icon: "🔒", title: "Non-root user", desc: "Security hardening" },
                    { icon: "📋", title: ".dockerignore", desc: "Skip unnecessary files" },
                    { icon: "🏷️", title: "Specific tags", desc: "Avoid :latest" },
                    { icon: "⚡", title: "Layer caching", desc: "COPY deps first" },
                    { icon: "🧹", title: "Cleanup", desc: "Remove temp files" },
                  ].map(tip => (
                    <div key={tip.title} style={{
                      padding: 10, background: "var(--bg-primary)", borderRadius: 8,
                      border: "1px solid var(--border-subtle)",
                    }}>
                      <div style={{ fontSize: "16px", marginBottom: 4 }}>{tip.icon}</div>
                      <div style={{ fontSize: "var(--text-xs)", fontWeight: 600 }}>{tip.title}</div>
                      <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{tip.desc}</div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* AI Chat Panel */}
          {chatOpen && (
            <div style={{ flex: "1 1 45%", minWidth: 0, display: "flex", flexDirection: "column" }}>
              <div className="card" style={{ padding: 0, overflow: "hidden", display: "flex", flexDirection: "column", height: "100%" }}>
                {/* Chat Header */}
                <div style={{
                  display: "flex", justifyContent: "space-between", alignItems: "center",
                  padding: "8px 12px", borderBottom: "1px solid var(--border-primary)",
                  background: "rgba(0,0,0,0.15)",
                }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: "var(--text-sm)" }}>
                    <span style={{ color: "var(--accent-purple)", fontWeight: 600 }}>AI</span>
                    <span style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
                      {currentProvider?.label} · {model}
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: 4 }}>
                    <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", padding: "2px 6px" }}
                      onClick={() => setShowConfig(!showConfig)}>⚙</button>
                    <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", padding: "2px 6px" }}
                      onClick={() => setChatMessages([])}>Clear</button>
                  </div>
                </div>

                {/* Config Panel */}
                {showConfig && (
                  <div style={{
                    padding: 12, borderBottom: "1px solid var(--border-primary)",
                    background: "var(--bg-secondary)", display: "flex", flexDirection: "column", gap: 8,
                  }}>
                    <div style={{ display: "flex", gap: 8 }}>
                      <div style={{ flex: 1 }}>
                        <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 2 }}>Provider</label>
                        <select value={provider} onChange={e => handleProviderChange(e.target.value)} style={{
                          width: "100%", padding: "6px 8px", background: "var(--bg-primary)",
                          border: "1px solid var(--border-primary)", borderRadius: 6,
                          color: "var(--text-primary)", fontSize: "var(--text-xs)",
                        }}>
                          {PROVIDERS.map(p => (
                            <option key={p.id} value={p.id}>{p.label}</option>
                          ))}
                        </select>
                      </div>
                      <div style={{ flex: 1 }}>
                        <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 2 }}>
                          Model {modelsFetching && <span style={{ color: "var(--accent-blue)" }}>⟳</span>}
                        </label>
                        <input
                          type="text" list={`models-${provider}`}
                          value={model} onChange={e => setModel(e.target.value)}
                          placeholder="Type or select a model..."
                          style={{
                            width: "100%", padding: "6px 8px", background: "var(--bg-primary)",
                            border: "1px solid var(--border-primary)", borderRadius: 6,
                            color: "var(--text-primary)", fontSize: "var(--text-xs)",
                            fontFamily: "var(--font-mono)",
                          }}
                        />
                        <datalist id={`models-${provider}`}>
                          {modelOptions.map((m: string) => (
                            <option key={m} value={m} />
                          ))}
                        </datalist>
                        <button className="btn btn-ghost" style={{ fontSize: "10px", padding: "1px 6px", marginTop: 2 }}
                            onClick={() => fetchModels(provider)} disabled={modelsFetching}>
                            {modelsFetching ? "Fetching..." : "↻ Refresh models"}
                          </button>
                      </div>
                    </div>
                    {provider !== "ollama-local" && (
                    <div>
                      <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 2 }}>
                        API Key
                      </label>
                      <input type="password" value={apiKey} onChange={e => setApiKey(e.target.value)}
                        placeholder={provider === "gemini" ? "Gemini API key from ai.google.dev" : provider === "ollama-cloud" ? "Bearer token (if required)" : "Enter API key..."}
                        style={{
                          width: "100%", padding: "6px 8px", background: "var(--bg-primary)",
                          border: "1px solid var(--border-primary)", borderRadius: 6,
                          color: "var(--text-primary)", fontSize: "var(--text-xs)",
                          fontFamily: "var(--font-mono)",
                        }} />
                    </div>
                    )}
                    {provider === "ollama-cloud" && (
                      <div>
                        <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 2 }}>
                          Endpoint URL
                        </label>
                        <input type="text" value={endpoint} onChange={e => setEndpoint(e.target.value)}
                          placeholder="https://your-ollama-server.com"
                          style={{
                            width: "100%", padding: "6px 8px", background: "var(--bg-primary)",
                            border: "1px solid var(--border-primary)", borderRadius: 6,
                            color: "var(--text-primary)", fontSize: "var(--text-xs)",
                            fontFamily: "var(--font-mono)",
                          }} />
                      </div>
                    )}
                    <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", alignSelf: "flex-end" }}
                      onClick={() => setShowConfig(false)}>Done</button>
                  </div>
                )}

                {/* Chat Messages */}
                <div style={{
                  flex: 1, overflow: "auto", padding: 12,
                  display: "flex", flexDirection: "column", gap: 10,
                  minHeight: 200, maxHeight: "calc(100vh - 360px)",
                }}>
                  {chatMessages.length === 0 && (
                    <div style={{ textAlign: "center", color: "var(--text-muted)", marginTop: 40, fontSize: "var(--text-sm)" }}>
                      <div style={{ fontSize: 24, marginBottom: 8 }}>🤖</div>
                      <div style={{ fontWeight: 500 }}>AI Dockerfile Assistant</div>
                      <div style={{ fontSize: "var(--text-xs)", marginTop: 4 }}>
                        Describe what you need and I'll generate a Dockerfile
                      </div>
                      <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 12, fontSize: "var(--text-xs)" }}>
                        {[
                          "Create a Dockerfile for a React app with Nginx",
                          "Add multi-stage build to optimize image size",
                          "Add health check and non-root user",
                        ].map(s => (
                          <button key={s} className="btn btn-ghost" style={{
                            fontSize: "var(--text-xs)", textAlign: "left", padding: "6px 10px",
                            border: "1px solid var(--border-subtle)", borderRadius: 6,
                          }} onClick={() => { setUserInput(s); }}>
                            💬 {s}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {chatMessages.map((msg, i) => (
                    <div key={i} style={{
                      display: "flex", flexDirection: "column",
                      alignItems: msg.role === "user" ? "flex-end" : "flex-start",
                    }}>
                      <div style={{
                        maxWidth: "90%", padding: "8px 12px", borderRadius: 10,
                        background: msg.role === "user"
                          ? "rgba(88,166,255,0.15)"
                          : "var(--bg-secondary)",
                        border: `1px solid ${msg.role === "user" ? "rgba(88,166,255,0.3)" : "var(--border-primary)"}`,
                        fontSize: "var(--text-sm)", lineHeight: 1.5,
                        whiteSpace: "pre-wrap", wordBreak: "break-word",
                      }}>
                        {msg.content}
                      </div>
                      {msg.role === "assistant" && msg.content.includes("FROM") && (
                        <button className="btn btn-ghost" style={{
                          fontSize: "11px", marginTop: 4, color: "var(--accent-green)",
                          padding: "2px 8px",
                        }} onClick={() => applyToEditor(msg.content)}>
                          ✓ Apply to Editor
                        </button>
                      )}
                    </div>
                  ))}
                  {aiLoading && (
                    <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted)", fontSize: "var(--text-sm)" }}>
                      <div className="spinner" style={{ width: 14, height: 14 }} />
                      Thinking...
                    </div>
                  )}
                  <div ref={chatEndRef} />
                </div>

                {/* Chat Input */}
                <div style={{
                  padding: "8px 12px", borderTop: "1px solid var(--border-primary)",
                  display: "flex", gap: 8,
                }}>
                  <input type="text" value={userInput} onChange={e => setUserInput(e.target.value)}
                    onKeyDown={e => e.key === "Enter" && !e.shiftKey && sendMessage()}
                    placeholder="Describe your Dockerfile needs..."
                    disabled={aiLoading}
                    style={{
                      flex: 1, padding: "8px 12px", background: "var(--bg-primary)",
                      border: "1px solid var(--border-primary)", borderRadius: 8,
                      color: "var(--text-primary)", fontSize: "var(--text-sm)",
                    }} />
                  <button className="btn btn-primary" onClick={sendMessage} disabled={aiLoading}
                    style={{ padding: "8px 16px" }}>
                    {aiLoading ? "..." : "Send"}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
