import React, { useState, useEffect, useRef, useCallback } from "react";
import { useAtom } from "jotai";
import { aiMessagesAtom, aiBubbleOpenAtom, aiProcessingAtom, aiErrorCountAtom, type DiagMessage } from "../store/aiDiagnosticsAtom";
import { aiApi, type ChatMessage, type SearchResult } from "../lib/api";
import { onError } from "../lib/globalToast";

// ===== Types & Constants =====

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic" },
  { id: "openai", label: "OpenAI" },
  { id: "gemini", label: "Google Gemini" },
  { id: "ollama-local", label: "Ollama Local" },
  { id: "ollama-cloud", label: "Ollama Cloud" },
];

const CONTENT_MODES = [
  { id: "full", label: "Full (images + links)" },
  { id: "compact", label: "Compact (strip images)" },
  { id: "minimal", label: "Minimal (text only)" },
];

const MAX_SEARCH_ROUNDS = 3;
const MAX_FETCH_PER_TURN = 2;
const MAX_SEARCH_PER_TURN = 3;

const SYSTEM_PROMPT = `You are ColimaUI's expert diagnostic AI agent for Docker, Colima, Kubernetes, and Lima troubleshooting on macOS.

## YOUR TOOLS

### Tool 1: Web Search
    [SEARCH: your optimized search query]

### Tool 2: Fetch Page
    [FETCH: https://full-url-here]

### Tool 3: Collect Diagnostic Logs (reads VM logs, processes, lock files)
    [DIAGNOSE]
**ALWAYS use this FIRST for Colima/Lima start/stop errors.**

### Tool 4: Run Safe Command (auto-executed, read-only)
    [RUN: command here]
Allowed: ps, cat, tail, head, ls, grep, colima status/list/version, docker ps/images/info/logs/inspect, kubectl get/describe/logs, lsof, netstat, brew list/info, uname, df, which

### Tool 5: Run Approved Command (needs user click to execute)
    [RUN_APPROVE: command here]
Requires approval: colima start/stop/restart, pkill, kill, docker start/stop/restart/pull, kubectl delete/apply, brew install/upgrade

### BANNED (never use — will be rejected):
rm, sudo, chmod, chown, mv, cp, sed -i, eval, exec, bash -c, docker rm/rmi/prune, colima delete, file writes (> >>), command chaining (; && || $())

### Limits per response:
- Max ${MAX_SEARCH_PER_TURN} [SEARCH:], ${MAX_FETCH_PER_TURN} [FETCH:], 3 [RUN:], 2 [RUN_APPROVE:], 1 [DIAGNOSE]

## KNOWLEDGE BANK
When [Knowledge Bank] results are injected into context:
- Solutions with high likes → prioritize these proven fixes
- Anti-patterns → explicitly AVOID these approaches
- Say "📚 Based on a previously successful fix" when using KB solutions

## DIAGNOSTIC WORKFLOW
1. [DIAGNOSE] → read actual VM logs and process state
2. [RUN: ...] → gather additional info (ps, cat, lsof, etc.)
3. Cross-reference with Knowledge Bank matches
4. Provide SPECIFIC fix based on actual findings
5. For fixes needing system changes → [RUN_APPROVE: ...]
6. NEVER give generic FAQ advice — always reference real data

## RULES
1. ALWAYS use [DIAGNOSE] + [RUN:] before giving advice for errors
2. Reference actual log content and process states in your analysis
3. Keep responses concise and actionable
4. Explain WHY a fix works based on the specific root cause found`;

interface AiChatBubbleProps {
  onNavigate?: (page: string) => void;
}

// ===== Simple Markdown Renderer =====

function renderMarkdown(text: string): React.ReactNode[] {
  const elements: React.ReactNode[] = [];
  const lines = text.split("\n");
  let inCode = false;
  let codeBlock = "";
  let codeIdx = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("```")) {
      if (inCode) {
        const code = codeBlock;
        const idx = codeIdx;
        elements.push(
          <pre key={`code-${idx}`}>
            <code>{code}</code>
            <button
              className="ai-copy-btn"
              onClick={() => navigator.clipboard.writeText(code)}
            >
              Copy
            </button>
          </pre>
        );
        codeBlock = "";
        inCode = false;
      } else {
        inCode = true;
        codeIdx = i;
      }
      continue;
    }
    if (inCode) {
      codeBlock += (codeBlock ? "\n" : "") + line;
      continue;
    }
    // Bold
    let html = line.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    // Inline code
    html = html.replace(/`([^`]+)`/g, '<code style="background:rgba(255,255,255,0.06);padding:1px 4px;border-radius:3px;font-size:0.72rem">$1</code>');
    // Headers
    if (html.startsWith("### ")) html = `<strong style="color:var(--text-primary)">${html.slice(4)}</strong>`;
    else if (html.startsWith("## ")) html = `<strong style="color:var(--text-primary);font-size:0.9rem">${html.slice(3)}</strong>`;
    // Bullet points
    if (html.startsWith("- ")) html = `• ${html.slice(2)}`;
    // Numbered list
    html = html.replace(/^(\d+)\.\s/, "$1. ");

    if (html.trim() === "") {
      elements.push(<br key={`br-${i}`} />);
    } else {
      elements.push(<p key={`p-${i}`} dangerouslySetInnerHTML={{ __html: html }} />);
    }
  }

  // Close unclosed code block
  if (inCode && codeBlock) {
    elements.push(<pre key="code-last"><code>{codeBlock}</code></pre>);
  }

  return elements;
}

// ===== Main Component =====

export default function AiChatBubble({ onNavigate }: AiChatBubbleProps) {
  const [messages, setMessages] = useAtom(aiMessagesAtom);
  const [isOpen, setIsOpen] = useAtom(aiBubbleOpenAtom);
  const [isProcessing, setIsProcessing] = useAtom(aiProcessingAtom);
  const [errorCount, setErrorCount] = useAtom(aiErrorCountAtom);

  const [userInput, setUserInput] = useState("");
  const [showConfig, setShowConfig] = useState(false);
  const [statusText, setStatusText] = useState("");
  const [pendingApprovals, setPendingApprovals] = useState<{ id: number; command: string; resolve: (v: boolean) => void }[]>([]);
  const [feedbackDismissed, setFeedbackDismissed] = useState(() => localStorage.getItem("ai_feedback_dismissed") === "true");

  // Config — shared localStorage keys
  const [provider, setProvider] = useState(() => localStorage.getItem("ai_provider") || "anthropic");
  const [model, setModel] = useState(() => localStorage.getItem("ai_model") || "");
  const [contentMode, setContentMode] = useState(() => localStorage.getItem("ai_diag_content_mode") || "full");

  // Read-only from Settings
  const getApiKey = () => localStorage.getItem("ai_api_key") || "";
  const getEndpoint = () => localStorage.getItem("ai_endpoint") || "";
  const getInstances = (): string[] => {
    try { return JSON.parse(localStorage.getItem("ai_searxng_instances") || '["http://localhost:8888/search","https://search.inetol.net/search","https://searx.be/search","https://search.brave4u.com/search","https://priv.au/search"]'); }
    catch { return ["http://localhost:8888/search","https://search.inetol.net/search","https://searx.be/search","https://search.brave4u.com/search","https://priv.au/search"]; }
  };
  const getMaxPageSize = () => parseInt(localStorage.getItem("ai_diag_max_page_size") || "8000", 10);
  const isAutoTrigger = () => localStorage.getItem("ai_diag_auto_trigger") !== "false";

  // Persist writable config
  useEffect(() => { localStorage.setItem("ai_provider", provider); }, [provider]);
  useEffect(() => { localStorage.setItem("ai_model", model); }, [model]);
  useEffect(() => { localStorage.setItem("ai_diag_content_mode", contentMode); }, [contentMode]);

  // Model list
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [modelsFetching, setModelsFetching] = useState(false);

  const fetchModels = useCallback(async () => {
    setModelsFetching(true);
    try {
      const raw = await aiApi.listModels(provider, getApiKey(), getEndpoint());
      const parsed: string[] = JSON.parse(typeof raw === "string" ? raw : "[]");
      setAvailableModels([...new Set(parsed)]);
    } catch {
      setAvailableModels([]);
    } finally {
      setModelsFetching(false);
    }
  }, [provider]);

  useEffect(() => {
    if (showConfig) fetchModels();
  }, [provider, showConfig, fetchModels]);

  // Auto-scroll
  const messagesEndRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isProcessing]);

  // ===== Error Auto-Trigger =====
  useEffect(() => {
    return onError((errorText) => {
      if (!isAutoTrigger()) return;
      const msg: DiagMessage = {
        id: Date.now(),
        role: "system",
        content: `⚠️ Error detected: ${errorText}`,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, msg]);
      if (!isOpen) {
        setErrorCount((c) => c + 1);
      }
      // Auto-run agent analysis
      runAgentLoop(`System error occurred: ${errorText}\n\nPlease analyze this error and help me troubleshoot it.`);
    });
  }, [isOpen]);

  // ===== Agent Loop =====
  const runAgentLoop = async (userMessage: string) => {
    const apiKey = getApiKey();
    if (!apiKey && provider !== "ollama-local") {
      setMessages((prev) => [...prev, {
        id: Date.now(), role: "system",
        content: "⚠️ API Key not configured. Go to Settings to set up your AI provider.",
        timestamp: Date.now(),
      }]);
      return;
    }

    setIsProcessing(true);
    const { invoke } = await import("@tauri-apps/api/core");

    // Query Knowledge Bank BEFORE calling AI
    let kbContext = "";
    try {
      const kbResult = await invoke<{ context_text: string }>("kb_query", { errorText: userMessage });
      if (kbResult.context_text) {
        kbContext = kbResult.context_text;
      }
    } catch { /* KB unavailable — continue without */ }

    const chatHistory: ChatMessage[] = [
      { role: "system", content: SYSTEM_PROMPT },
      ...(kbContext ? [{ role: "system" as const, content: `[Knowledge Bank]\n${kbContext}` }] : []),
      ...messages.filter(m => m.role !== "system").map(m => ({
        role: m.role as "user" | "assistant",
        content: m.content,
      })),
      { role: "user", content: userMessage },
    ];

    try {
      for (let round = 0; round < MAX_SEARCH_ROUNDS; round++) {
        setStatusText(round === 0 ? "Analyzing..." : `Round ${round + 1}...`);

        const response = await aiApi.chat(provider, model, apiKey, chatHistory, getEndpoint());
        const responseText = typeof response === "string" ? response : String(response);

        // Parse ALL tool markers
        const searches = [...responseText.matchAll(/\[SEARCH:\s*(.+?)\]/g)];
        const fetches = [...responseText.matchAll(/\[FETCH:\s*(.+?)\]/g)];
        const hasDiagnose = /\[DIAGNOSE\]/.test(responseText);
        const runs = [...responseText.matchAll(/\[RUN:\s*(.+?)\]/g)];
        const runApprovals = [...responseText.matchAll(/\[RUN_APPROVE:\s*(.+?)\]/g)];

        // Clean response (remove ALL markers for display)
        const cleanText = responseText
          .replace(/\[SEARCH:\s*.+?\]/g, "")
          .replace(/\[FETCH:\s*.+?\]/g, "")
          .replace(/\[DIAGNOSE\]/g, "")
          .replace(/\[RUN:\s*.+?\]/g, "")
          .replace(/\[RUN_APPROVE:\s*.+?\]/g, "")
          .trim();

        const hasTools = searches.length > 0 || fetches.length > 0 || hasDiagnose || runs.length > 0 || runApprovals.length > 0;

        if (!hasTools) {
          // No tools needed — final response
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "assistant",
            content: cleanText, timestamp: Date.now(),
          }]);
          break;
        }

        // Show intermediate thinking
        if (cleanText) {
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "assistant",
            content: cleanText, timestamp: Date.now(),
          }]);
        }

        let toolContext = "";
        const instances = getInstances();
        const maxPageSize = getMaxPageSize();

        // Tool 3: Diagnostic log collection
        if (hasDiagnose) {
          setStatusText("🔬 Collecting diagnostic logs...");
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "system",
            content: "🔬 Reading VM logs, checking processes, inspecting lock files...",
            timestamp: Date.now(),
          }]);
          try {
            const diagReport = await invoke<string>("collect_diagnostic_logs", { profile: "default" });
            toolContext += `\n\n### Diagnostic Report (collected from local system)\n${diagReport}\n`;
          } catch (e) {
            toolContext += `\n(Diagnostic collection failed: ${e})\n`;
          }
        }

        // Tool 4: Run safe commands (auto-executed)
        for (const [, cmd] of runs.slice(0, 3)) {
          setStatusText(`🔧 Running: ${cmd.slice(0, 40)}...`);
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "system",
            content: `🔧 Running: \`${cmd}\``,
            timestamp: Date.now(),
          }]);
          try {
            const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>("sandbox_execute", { command: cmd });
            const output = result.stdout || result.stderr || "(no output)";
            toolContext += `\n\n### Command: \`${cmd}\` (exit: ${result.exit_code})\n\`\`\`\n${output}\n\`\`\`\n`;
            setMessages((prev) => [...prev, {
              id: Date.now(), role: "system",
              content: `✅ \`${cmd}\` → exit ${result.exit_code}`,
              timestamp: Date.now(),
            }]);
          } catch (e) {
            const errStr = String(e);
            if (errStr.startsWith("banned:")) {
              toolContext += `\n\n### Command BLOCKED: \`${cmd}\`\nReason: ${errStr.replace("banned:", "")}\n`;
              setMessages((prev) => [...prev, {
                id: Date.now(), role: "system",
                content: `🚫 Blocked: \`${cmd}\` — ${errStr.replace("banned:", "")}`,
                timestamp: Date.now(),
              }]);
            } else {
              toolContext += `\n(Command failed: ${e})\n`;
            }
          }
        }

        // Tool 5: Run approved commands (need user confirmation)
        for (const [, cmd] of runApprovals.slice(0, 2)) {
          setStatusText(`⏳ Awaiting approval: ${cmd}`);
          // Show approval request in chat
          const approvalId = Date.now();
          setMessages((prev) => [...prev, {
            id: approvalId, role: "system",
            content: `⚠️ AI wants to run:\n\`${cmd}\`\n\n_Awaiting your approval..._`,
            timestamp: Date.now(),
          }]);

          // Use a promise-based approval mechanism
          const approved = await new Promise<boolean>((resolve) => {
            setPendingApprovals((prev) => [...prev, { id: approvalId, command: cmd, resolve }]);
          });

          if (approved) {
            try {
              const result = await invoke<{ stdout: string; stderr: string; exit_code: number }>("sandbox_execute_approved", { command: cmd });
              const output = result.stdout || result.stderr || "(no output)";
              toolContext += `\n\n### Approved Command: \`${cmd}\` (exit: ${result.exit_code})\n\`\`\`\n${output}\n\`\`\`\n`;
              setMessages((prev) => [...prev, {
                id: Date.now(), role: "system",
                content: `✅ Approved & ran: \`${cmd}\` → exit ${result.exit_code}`,
                timestamp: Date.now(),
              }]);
            } catch (e) {
              toolContext += `\n(Approved command failed: ${e})\n`;
            }
          } else {
            toolContext += `\n\n### Command DENIED by user: \`${cmd}\`\n`;
            setMessages((prev) => [...prev, {
              id: Date.now(), role: "system",
              content: `❌ Denied: \`${cmd}\``,
              timestamp: Date.now(),
            }]);
          }
        }

        // Tool 1: Web searches
        for (const [, query] of searches.slice(0, MAX_SEARCH_PER_TURN)) {
          setStatusText(`🔍 Searching: ${query}`);
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "system",
            content: `🔍 Searching: "${query}"`,
            timestamp: Date.now(),
          }]);
          try {
            const results = await aiApi.search(query, instances, 5);
            if (results.length > 0) {
              toolContext += `\n\n### Search results for "${query}":\n`;
              results.forEach((r: SearchResult, i: number) => {
                toolContext += `${i + 1}. **${r.title}** (${r.url})\n   ${r.content}\n`;
              });
              setStatusText(`📄 Reading: ${results[0].title}`);
              setMessages((prev) => [...prev, {
                id: Date.now(), role: "system",
                content: `📄 Reading: ${results[0].title}`,
                timestamp: Date.now(),
              }]);
              try {
                const md = await aiApi.fetchPageMarkdown(results[0].url, maxPageSize, contentMode);
                toolContext += `\n\n--- Content from: ${results[0].title} ---\n${md}\n`;
              } catch {
                toolContext += `\n(Failed to fetch page content)\n`;
              }
            } else {
              toolContext += `\n(No search results found for "${query}")\n`;
            }
          } catch (e) {
            toolContext += `\n(Search failed: ${e})\n`;
          }
        }

        // Tool 2: Fetch URLs
        for (const [, url] of fetches.slice(0, MAX_FETCH_PER_TURN)) {
          setStatusText(`📄 Fetching: ${url.slice(0, 40)}...`);
          setMessages((prev) => [...prev, {
            id: Date.now(), role: "system",
            content: `📄 Fetching: ${url}`,
            timestamp: Date.now(),
          }]);
          try {
            const md = await aiApi.fetchPageMarkdown(url, maxPageSize, contentMode);
            toolContext += `\n\n--- Fetched: ${url} ---\n${md}\n`;
          } catch (e) {
            toolContext += `\n(Failed to fetch ${url}: ${e})\n`;
          }
        }

        // Inject tool results back into conversation
        chatHistory.push(
          { role: "assistant", content: responseText },
          { role: "user", content: `[Tool Results]\n${toolContext}\n\nBased on the above diagnostic data, provide your SPECIFIC diagnosis and targeted fixes.` }
        );
      }
    } catch (e) {
      setMessages((prev) => [...prev, {
        id: Date.now(), role: "assistant",
        content: `Error: ${e}`,
        timestamp: Date.now(),
      }]);
    } finally {
      setIsProcessing(false);
      setStatusText("");
    }
  };

  // ===== User Send =====
  const handleSend = () => {
    const text = userInput.trim();
    if (!text || isProcessing) return;

    setMessages((prev) => [...prev, {
      id: Date.now(), role: "user",
      content: text, timestamp: Date.now(),
    }]);
    setUserInput("");
    runAgentLoop(text);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleClear = () => {
    setMessages([]);
    setErrorCount(0);
  };

  const toggleOpen = () => {
    setIsOpen(!isOpen);
    if (!isOpen) setErrorCount(0);
  };

  // ===== Feedback (Like/Dislike) =====
  const handleFeedback = async (msgId: number, isLike: boolean) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      // Find the error context (the system error message)
      const errorMsg = messages.find(m => m.role === "system" && m.content.includes("Error detected"));
      const aiMsg = messages.find(m => m.id === msgId);
      if (!aiMsg) return;

      const errorPattern = errorMsg
        ? errorMsg.content.replace(/^⚠️ Error detected:\s*/, "").slice(0, 200)
        : "general";

      if (isLike) {
        await invoke("kb_save_solution", {
          errorPattern,
          errorCategory: "learned",
          solutionText: aiMsg.content,
          rootCause: "",
        });
      } else {
        await invoke("kb_save_anti_pattern", {
          errorPattern,
          badSuggestion: aiMsg.content.slice(0, 500),
          reason: "User reported this fix did not work",
        });
      }

      // Mark message as feedback given
      setMessages(prev => prev.map(m =>
        m.id === msgId ? { ...m, feedback: isLike ? "liked" : "disliked" } : m
      ));
    } catch { /* feedback failed silently */ }
  };

  // ===== Command Approval =====
  const handleApproval = (approvalId: number, approved: boolean) => {
    setPendingApprovals(prev => {
      const item = prev.find(a => a.id === approvalId);
      if (item) item.resolve(approved);
      return prev.filter(a => a.id !== approvalId);
    });
  };

  const dismissFeedbackBanner = () => {
    setFeedbackDismissed(true);
    localStorage.setItem("ai_feedback_dismissed", "true");
  };

  // ===== Render =====
  return (
    <>
      {/* Floating Trigger Button — hidden when panel is open */}
      <button className={`ai-bubble-trigger${isOpen ? " ai-bubble-trigger-hidden" : ""}`} onClick={toggleOpen} title="AI Diagnostics">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" />
          <path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
        </svg>
        {errorCount > 0 && (
          <span className="ai-bubble-badge">{errorCount > 9 ? "9+" : errorCount}</span>
        )}
      </button>

      {/* Chat Panel */}
      {isOpen && (
        <div className="ai-bubble-panel">
          {/* Header */}
          <div className="ai-bubble-header">
            <div className="ai-bubble-header-title">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" />
                <path d="M15 13v2" /><path d="M9 13v2" />
              </svg>
              AI Diagnostics
              {isProcessing && (
                <span style={{ fontSize: "11px", color: "var(--accent-blue)", fontWeight: 400 }}>
                  {statusText || "Processing..."}
                </span>
              )}
            </div>
            <div className="ai-bubble-header-actions">
              <button onClick={() => setShowConfig(!showConfig)} title="Settings">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                  <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
              </button>
              <button onClick={handleClear} title="Clear chat">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                  <path d="M3 6h18" /><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                  <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                </svg>
              </button>
              <button onClick={toggleOpen} title="Minimize">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                  <path d="M5 12h14" />
                </svg>
              </button>
            </div>
          </div>

          {/* Config Panel */}
          {showConfig && (
            <div className="ai-bubble-config">
              <div className="ai-bubble-config-row">
                <div style={{ flex: 1 }}>
                  <label>Provider</label>
                  <select value={provider} onChange={e => { setProvider(e.target.value); setModel(""); }}>
                    {PROVIDERS.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}
                  </select>
                </div>
                <div style={{ flex: 1 }}>
                  <label>Model {modelsFetching && <span style={{ color: "var(--accent-blue)" }}>⟳</span>}</label>
                  <input
                    type="text"
                    list="ai-diag-models"
                    value={model}
                    onChange={e => setModel(e.target.value)}
                    placeholder="Select model..."
                  />
                  <datalist id="ai-diag-models">
                    {availableModels.map(m => <option key={m} value={m} />)}
                  </datalist>
                </div>
              </div>
              <div>
                <label>Content Mode</label>
                <select value={contentMode} onChange={e => setContentMode(e.target.value)}>
                  {CONTENT_MODES.map(m => <option key={m.id} value={m.id}>{m.label}</option>)}
                </select>
              </div>
              {!getApiKey() && provider !== "ollama-local" && (
                <div className="ai-bubble-config-warning">
                  ⚠️ API Key not set
                  <button onClick={() => { onNavigate?.("settings"); setShowConfig(false); }}>
                    Go to Settings
                  </button>
                </div>
              )}
              <button
                className="btn btn-ghost"
                style={{ fontSize: "var(--text-xs)", alignSelf: "flex-end" }}
                onClick={() => setShowConfig(false)}
              >
                Done
              </button>
            </div>
          )}

          {/* Feedback Info Banner */}
          {!feedbackDismissed && messages.length > 0 && (
            <div className="ai-feedback-banner">
              <span>💡 Your feedback improves AI over time. <strong>👍</strong> = save fix for future. <strong>👎</strong> = AI will avoid this approach.</span>
              <button onClick={dismissFeedbackBanner} title="Dismiss">✕</button>
            </div>
          )}

          {/* Messages */}
          <div className="ai-bubble-messages">
            {messages.length === 0 && (
              <div style={{ textAlign: "center", color: "var(--text-muted)", marginTop: 40 }}>
                <div style={{ fontSize: 28, marginBottom: 8 }}>🤖</div>
                <div style={{ fontWeight: 500, fontSize: "var(--text-sm)" }}>AI Diagnostics Agent</div>
                <div style={{ fontSize: "var(--text-xs)", marginTop: 4, maxWidth: 280, margin: "4px auto 0" }}>
                  Investigates errors using diagnostic logs, web search, and command execution. Paste error messages or describe your issue.
                </div>
              </div>
            )}
            {messages.map((msg) => (
              <div key={msg.id} className={`ai-msg ai-msg-${msg.role}`}>
                {msg.role === "system" ? (
                  <span>{msg.content}</span>
                ) : (
                  renderMarkdown(msg.content)
                )}
                {/* Like/Dislike buttons on assistant messages */}
                {msg.role === "assistant" && (
                  <div className="ai-msg-feedback">
                    <button
                      onClick={() => handleFeedback(msg.id, true)}
                      className={(msg as any).feedback === "liked" ? "active-like" : ""}
                      disabled={!!(msg as any).feedback}
                      title="This fix worked — save to knowledge bank"
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                        <path d="M7 10v12" /><path d="M15 5.88 14 10h5.83a2 2 0 0 1 1.92 2.56l-2.33 8A2 2 0 0 1 17.5 22H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2.76a2 2 0 0 0 1.79-1.11L12 2h0a3.13 3.13 0 0 1 3 3.88Z" />
                      </svg>
                    </button>
                    <button
                      onClick={() => handleFeedback(msg.id, false)}
                      className={(msg as any).feedback === "disliked" ? "active-dislike" : ""}
                      disabled={!!(msg as any).feedback}
                      title="This fix did NOT work — AI will avoid it"
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                        <path d="M17 14V2" /><path d="M9 18.12 10 14H4.17a2 2 0 0 1-1.92-2.56l2.33-8A2 2 0 0 1 6.5 2H20a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2.76a2 2 0 0 0-1.79 1.11L12 22h0a3.13 3.13 0 0 1-3-3.88Z" />
                      </svg>
                    </button>
                    {(msg as any).feedback && (
                      <span className="ai-feedback-status">
                        {(msg as any).feedback === "liked" ? "✓ Saved" : "✓ Noted"}
                      </span>
                    )}
                  </div>
                )}
                {/* Approval buttons for pending commands */}
                {pendingApprovals.some(a => a.id === msg.id) && (
                  <div className="ai-approval-buttons">
                    <button className="ai-approve-btn" onClick={() => handleApproval(msg.id, true)}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><polyline points="20 6 9 17 4 12" /></svg>
                      Allow
                    </button>
                    <button className="ai-deny-btn" onClick={() => handleApproval(msg.id, false)}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                      Deny
                    </button>
                  </div>
                )}
              </div>
            ))}
            {isProcessing && (
              <div className="ai-msg ai-msg-assistant">
                <span className="ai-typing-dots">
                  <span>●</span><span>●</span><span>●</span>
                </span>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* Input */}
          <div className="ai-bubble-input">
            <input
              value={userInput}
              onChange={e => setUserInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Describe your issue or paste error..."
              disabled={isProcessing}
            />
            <button onClick={handleSend} disabled={isProcessing || !userInput.trim()}>
              {isProcessing ? "..." : "Send"}
            </button>
          </div>
        </div>
      )}
    </>
  );
}
