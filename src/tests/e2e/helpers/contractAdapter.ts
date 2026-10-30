/**
 * Contract Adapter & Specification Oracle for Colima UI AI Chat UI/UX Redesign
 * 
 * Authoritative Sources:
 * - ORIGINAL_REQUEST.md (Requirements R1-R5)
 * - PROJECT.md (Architecture, Interfaces, Feature Inventory)
 * - implementation_plan.md (Schemas, AST structures, Regexes)
 */

export type ChatBlockType =
  | 'think'
  | 'code'
  | 'table'
  | 'heading'
  | 'list'
  | 'paragraph'
  | 'hr'
  | 'blockquote';

export interface ThinkBlock {
  type: 'think';
  content: string;
  streaming: boolean;
}

export interface CodeBlock {
  type: 'code';
  language: string;
  code: string;
  isShell: boolean;
}

export interface TableBlock {
  type: 'table';
  headers: string[];
  alignments: ('left' | 'center' | 'right' | 'none')[];
  rows: string[][];
}

export interface HeadingBlock {
  type: 'heading';
  level: number;
  text: string;
}

export interface ListBlock {
  type: 'list';
  ordered: boolean;
  items: string[];
}

export interface BlockquoteBlock {
  type: 'blockquote';
  text: string;
}

export interface HrBlock {
  type: 'hr';
}

export interface ParagraphBlock {
  type: 'paragraph';
  text: string;
}

export type ChatBlock =
  | ThinkBlock
  | CodeBlock
  | TableBlock
  | HeadingBlock
  | ListBlock
  | BlockquoteBlock
  | HrBlock
  | ParagraphBlock;

export type ToolStepStatus =
  | 'pending_approval'
  | 'running'
  | 'succeeded'
  | 'denied'
  | 'failed';

export type ToolStepKind =
  | 'query'
  | 'event'
  | 'command'
  | 'diagnose'
  | 'security'
  | 'navigate'
  | 'schedule';

export interface ToolStepPayload {
  version: 1;
  kind: ToolStepKind;
  name: string;
  title: string;
  safety: 'SAFE' | 'NORMAL' | 'DANGEROUS';
  status: ToolStepStatus;
  rawInput?: string;
  output?: string;
  exitCode?: number;
  warning?: string;
  timestamp?: number;
}

export interface ChoiceOption {
  label: string;
  description?: string;
  value: string;
}

export interface ParsedChoicePrompt {
  question: string;
  multi: boolean;
  options: ChoiceOption[];
}

export interface StreamingPartialTagResult {
  cleanText: string;
  pendingTagKind: 'query' | 'event_approve' | 'run' | 'choice' | 'suggest' | null;
}

export interface StarterPrompt {
  id: string;
  title: string;
  prompt: string;
  icon: 'warning' | 'container' | 'vm' | 'compose' | 'k8s' | 'security' | 'general';
  category: 'warning' | 'context' | 'help';
}

export interface StarterPromptContext {
  currentPage: string;
  colimaInstances: Array<{ name: string; status: string }>;
  containers: Array<{ id: string; name: string; state: string }>;
  k8sStatus?: { connected: boolean; running: boolean };
}

export interface FallbackSuggestionContext {
  toolsExecuted: Array<{ kind: string; name: string }>;
  currentPage: string;
}

// ---------------------------------------------------------------------------
// Shell Language & Tag Constants
// ---------------------------------------------------------------------------
export const SHELL_LANGS = new Set([
  'sh',
  'bash',
  'zsh',
  'shell',
  'console',
  'docker',
  'colima',
  'kubectl',
  'limactl',
  'helm',
]);

export const SHELL_COMMAND_PREFIXES = [
  'colima ',
  'docker ',
  'kubectl ',
  'limactl ',
  'helm ',
];

export const TOOL_STEP_PREFIX = '<!--AI_TOOL_STEP:';
export const TOOL_STEP_SUFFIX = '-->';

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Chat Markdown Parser
// ---------------------------------------------------------------------------
export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

export function renderInlineChatMarkdown(text: string): string {
  let html = escapeHtml(text);
  // **bold**
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  // *italic*
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  // `code`
  html = html.replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>');
  // [text](url)
  html = html.replace(
    /\[([^\]]+)\]\(([^)\s]+)\)/g,
    '<a href="$2" class="chat-link" target="_blank" rel="noopener noreferrer">$1</a>',
  );
  return html;
}

export function specParseChatMarkdown(text: string): ChatBlock[] {
  const blocks: ChatBlock[] = [];
  const lines = text.split('\n');
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // 1. Thinking block <think> ... </think> or unclosed <think>
    if (line.includes('<think>')) {
      const startIdx = line.indexOf('<think>');
      const beforeThink = line.slice(0, startIdx).trim();
      if (beforeThink) {
        blocks.push({ type: 'paragraph', text: beforeThink });
      }

      let content = line.slice(startIdx + 7);
      let closed = false;

      if (content.includes('</think>')) {
        const endIdx = content.indexOf('</think>');
        const afterThink = content.slice(endIdx + 8).trim();
        content = content.slice(0, endIdx);
        blocks.push({ type: 'think', content: content.trim(), streaming: false });
        if (afterThink) {
          blocks.push({ type: 'paragraph', text: afterThink });
        }
        i++;
        continue;
      }

      i++;
      while (i < lines.length) {
        const nextLine = lines[i];
        if (nextLine.includes('</think>')) {
          const endIdx = nextLine.indexOf('</think>');
          content += (content ? '\n' : '') + nextLine.slice(0, endIdx);
          const afterThink = nextLine.slice(endIdx + 8).trim();
          closed = true;
          blocks.push({ type: 'think', content: content.trim(), streaming: false });
          if (afterThink) {
            blocks.push({ type: 'paragraph', text: afterThink });
          }
          i++;
          break;
        } else {
          content += (content ? '\n' : '') + nextLine;
          i++;
        }
      }

      if (!closed) {
        blocks.push({ type: 'think', content: content.trim(), streaming: true });
      }
      continue;
    }

    // 2. Fenced Code Block ```lang ... ```
    if (line.trim().startsWith('```')) {
      const langMatch = line.trim().match(/^```([a-zA-Z0-9_-]*)/);
      const language = langMatch ? langMatch[1].toLowerCase() : '';
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      if (i < lines.length && lines[i].trim().startsWith('```')) {
        i++; // skip closing ```
      }
      const code = codeLines.join('\n');
      const firstLine = codeLines[0]?.trim() || '';
      const isShell =
        SHELL_LANGS.has(language) ||
        SHELL_COMMAND_PREFIXES.some((p) => firstLine.startsWith(p));
      blocks.push({ type: 'code', language, code, isShell });
      continue;
    }

    // 3. GFM Table Header | col1 | col2 |
    if (line.trim().startsWith('|') && line.includes('|') && i + 1 < lines.length) {
      const nextLine = lines[i + 1].trim();
      if (nextLine.startsWith('|') && nextLine.includes('---')) {
        const headers = line
          .split('|')
          .slice(1, -1)
          .map((h) => h.trim());
        const alignSpecs = nextLine
          .split('|')
          .slice(1, -1)
          .map((a) => a.trim());
        const alignments: ('left' | 'center' | 'right' | 'none')[] = alignSpecs.map(
          (spec) => {
            const left = spec.startsWith(':');
            const right = spec.endsWith(':');
            if (left && right) return 'center';
            if (left) return 'left';
            if (right) return 'right';
            return 'none';
          },
        );

        i += 2;
        const rows: string[][] = [];
        while (i < lines.length && lines[i].trim().startsWith('|')) {
          const cells = lines[i]
            .split('|')
            .slice(1, -1)
            .map((c) => c.trim());
          rows.push(cells);
          i++;
        }

        blocks.push({ type: 'table', headers, alignments, rows });
        continue;
      }
    }

    // 4. Headings #, ##, ###
    const headingMatch = line.match(/^(#{1,6})\s+(.*)$/);
    if (headingMatch) {
      blocks.push({
        type: 'heading',
        level: headingMatch[1].length,
        text: headingMatch[2].trim(),
      });
      i++;
      continue;
    }

    // 5. Horizontal rule
    if (/^(?:---|\*\*\*|___)\s*$/.test(line.trim())) {
      blocks.push({ type: 'hr' });
      i++;
      continue;
    }

    // 6. Blockquote
    if (line.startsWith('>')) {
      const quoteLines: string[] = [];
      while (i < lines.length && lines[i].startsWith('>')) {
        quoteLines.push(lines[i].replace(/^>\s?/, ''));
        i++;
      }
      blocks.push({ type: 'blockquote', text: quoteLines.join('\n') });
      continue;
    }

    // 7. Unordered / Ordered Lists
    const unorderedMatch = line.match(/^([*-]|\+)\s+(.*)$/);
    const orderedMatch = line.match(/^(\d+)\.\s+(.*)$/);
    if (unorderedMatch || orderedMatch) {
      const ordered = Boolean(orderedMatch);
      const items: string[] = [];
      while (i < lines.length) {
        const cur = lines[i];
        const match = ordered
          ? cur.match(/^\d+\.\s+(.*)$/)
          : cur.match(/^[*-]\s+(.*)$/);
        if (match) {
          items.push(match[1]);
          i++;
        } else if (cur.startsWith('  ') && items.length > 0) {
          items[items.length - 1] += '\n' + cur.trim();
          i++;
        } else {
          break;
        }
      }
      blocks.push({ type: 'list', ordered, items });
      continue;
    }

    // 8. Paragraph / Empty Lines
    if (line.trim().length === 0) {
      i++;
      continue;
    }

    const paraLines: string[] = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim().length > 0 &&
      !lines[i].startsWith('#') &&
      !lines[i].startsWith('```') &&
      !lines[i].startsWith('|') &&
      !lines[i].startsWith('>') &&
      !lines[i].includes('<think>') &&
      !/^[*-]\s+/.test(lines[i]) &&
      !/^\d+\.\s+/.test(lines[i])
    ) {
      paraLines.push(lines[i]);
      i++;
    }
    blocks.push({ type: 'paragraph', text: paraLines.join('\n') });
  }

  return blocks;
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Tool Step Serialization
// ---------------------------------------------------------------------------
export function specSerializeToolStep(step: ToolStepPayload): string {
  const json = JSON.stringify(step).replace(/-->/g, '--\\u003e');
  const statusIcon =
    step.status === 'succeeded'
      ? '✓'
      : step.status === 'failed' || step.status === 'denied'
        ? '✕'
        : '⏳';
  const summary = `${statusIcon} [${step.kind.toUpperCase()}] ${step.title}`;
  return `${TOOL_STEP_PREFIX}${json}${TOOL_STEP_SUFFIX}\n${summary}`;
}

export function specParseToolStep(content: string): ToolStepPayload | null {
  if (!content.startsWith(TOOL_STEP_PREFIX)) return null;
  const endIdx = content.indexOf(TOOL_STEP_SUFFIX);
  if (endIdx === -1) return null;
  const jsonStr = content.slice(TOOL_STEP_PREFIX.length, endIdx);
  try {
    const parsed = JSON.parse(jsonStr) as ToolStepPayload;
    if (parsed.version === 1 && parsed.kind && parsed.name && parsed.status) {
      return parsed;
    }
    return null;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: [ASK_CHOICE] Extractor
// ---------------------------------------------------------------------------
export function specExtractChoicePrompt(text: string): {
  cleanText: string;
  choicePrompt: ParsedChoicePrompt | null;
} {
  const choiceOpenIdx = text.indexOf('[ASK_CHOICE:');
  if (choiceOpenIdx === -1) {
    return { cleanText: text, choicePrompt: null };
  }

  // Find closing bracket taking into account strings and nested brackets
  let bracketDepth = 0;
  let inString = false;
  let closeIdx = -1;

  for (let i = choiceOpenIdx; i < text.length; i++) {
    const ch = text[i];
    if (inString) {
      if (ch === '\\') {
        i++;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }

    if (ch === '"') {
      inString = true;
      continue;
    }

    if (ch === '[') {
      bracketDepth++;
    } else if (ch === ']') {
      bracketDepth--;
      if (bracketDepth === 0) {
        closeIdx = i;
        break;
      }
    }
  }

  if (closeIdx === -1) {
    return { cleanText: text, choicePrompt: null };
  }

  const inner = text.slice(choiceOpenIdx + '[ASK_CHOICE:'.length, closeIdx).trim();
  const cleanText = (text.slice(0, choiceOpenIdx) + text.slice(closeIdx + 1)).trim();

  // Try parsing as JSON first
  try {
    const jsonParsed = JSON.parse(inner) as Partial<ParsedChoicePrompt>;
    if (jsonParsed.question && Array.isArray(jsonParsed.options)) {
      return {
        cleanText,
        choicePrompt: {
          question: jsonParsed.question,
          multi: Boolean(jsonParsed.multi),
          options: jsonParsed.options.map((opt) => ({
            label: String(opt.label),
            description: opt.description ? String(opt.description) : undefined,
            value: String(opt.value ?? opt.label),
          })),
        },
      };
    }
  } catch {
    // Fall back to compact pipe syntax: "Question | Opt 1 | Opt 2 | Opt 3"
  }

  if (inner.includes('|')) {
    const parts = inner.split('|').map((p) => p.trim());
    const question = parts[0];
    const options: ChoiceOption[] = parts.slice(1).map((opt) => ({
      label: opt,
      value: opt,
    }));
    if (question && options.length > 0) {
      return {
        cleanText,
        choicePrompt: {
          question,
          multi: false,
          options,
        },
      };
    }
  }

  return { cleanText, choicePrompt: null };
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Streaming Tag Suppression
// ---------------------------------------------------------------------------
export function specStripStreamingPartialTags(text: string): StreamingPartialTagResult {
  const PARTIAL_TAG_RE =
    /\[(QUERY|EVENT_APPROVE|RUN|RUN_APPROVE|ASK_CHOICE|SUGGEST)(?::[^\]]*)?$/i;

  const match = text.match(PARTIAL_TAG_RE);
  if (match && match.index !== undefined) {
    const rawKind = match[1].toUpperCase();
    let pendingTagKind: StreamingPartialTagResult['pendingTagKind'] = null;
    if (rawKind === 'QUERY') pendingTagKind = 'query';
    else if (rawKind === 'EVENT_APPROVE') pendingTagKind = 'event_approve';
    else if (rawKind === 'RUN' || rawKind === 'RUN_APPROVE') pendingTagKind = 'run';
    else if (rawKind === 'ASK_CHOICE') pendingTagKind = 'choice';
    else if (rawKind === 'SUGGEST') pendingTagKind = 'suggest';

    const cleanText = text.slice(0, match.index).trimEnd();
    return { cleanText, pendingTagKind };
  }

  return { cleanText: text, pendingTagKind: null };
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Starter Prompts
// ---------------------------------------------------------------------------
export function specGetStarterPrompts(ctx: StarterPromptContext): StarterPrompt[] {
  const prompts: StarterPrompt[] = [];

  // 1. Priority Warnings
  for (const inst of (ctx.colimaInstances || [])) {
    if ((inst.status || '').toLowerCase() !== 'running') {
      prompts.push({
        id: `warn-instance-${inst.name}`,
        title: `Investigate stopped instance ${inst.name}`,
        prompt: `Why is Colima instance ${inst.name} stopped? Check its status and suggest how to start it.`,
        icon: 'warning',
        category: 'warning',
      });
      break;
    }
  }

  for (const c of (ctx.containers || [])) {
    if ((c.state || '').toLowerCase() === 'exited') {
      prompts.push({
        id: `warn-container-${c.id}`,
        title: `Analyze exited container ${c.name}`,
        prompt: `Container ${c.name} (${c.id}) exited. Analyze the container exit reason and logs.`,
        icon: 'warning',
        category: 'warning',
      });
      break;
    }
  }

  // 2. Active Tab Prompts
  const page = ctx.currentPage.toLowerCase();
  if (page === 'compose') {
    prompts.push({
      id: 'tab-compose-diagnose',
      title: 'Diagnose Compose services',
      prompt: 'Diagnose all services in the active Docker Compose project.',
      icon: 'compose',
      category: 'context',
    });
  } else if (page === 'containers') {
    prompts.push({
      id: 'tab-containers-resources',
      title: 'Inspect resource consumption',
      prompt: 'List running containers and inspect their CPU and memory usage.',
      icon: 'container',
      category: 'context',
    });
  } else if (page === 'kubernetes' || page === 'k8s') {
    prompts.push({
      id: 'tab-k8s-pods',
      title: 'Check Kubernetes pods',
      prompt: 'Check the health of Kubernetes pods and node status in the active namespace.',
      icon: 'k8s',
      category: 'context',
    });
  } else {
    prompts.push({
      id: 'tab-dashboard-overview',
      title: 'System health overview',
      prompt: 'Provide a system health overview of Colima, Docker, and Kubernetes.',
      icon: 'vm',
      category: 'context',
    });
  }

  prompts.push({
    id: 'tab-general-optimize',
    title: 'Optimize VM resources',
    prompt: 'What are the recommended CPU, memory, and disk settings for my Colima instances?',
    icon: 'general',
    category: 'help',
  });

  return prompts.slice(0, 4);
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Fallback Suggestions
// ---------------------------------------------------------------------------
export function specDeriveFallbackSuggestions(
  ctx: FallbackSuggestionContext,
): [string, string][] {
  const suggestions: [string, string][] = [];

  const tools = (ctx.toolsExecuted || []).map((t) => (typeof t === 'string' ? { kind: t, name: t } : t || {}));
  const toolKinds = new Set(tools.map((t) => t.kind || ''));
  const toolNames = new Set(tools.map((t) => t.name || ''));

  if (toolNames.has('list-containers') || toolNames.has('getContainers')) {
    suggestions.push(['Check CPU & RAM', 'Inspect CPU and memory usage of running containers']);
    suggestions.push(['View container logs', 'Show me the recent logs for the active container']);
  } else if (toolKinds.has('diagnose') || toolNames.has('DIAGNOSE')) {
    suggestions.push(['Apply recommended fix', 'Propose and execute automated fix steps for the diagnosed issue']);
    suggestions.push(['Explain root cause', 'Explain in detail why this failure occurred']);
  } else if (toolKinds.has('event') || toolKinds.has('command')) {
    suggestions.push(['Verify status', 'Verify that the operation completed successfully']);
    suggestions.push(['Next steps', 'What should I do next after this operation?']);
  } else {
    suggestions.push(['Show system status', 'Show current Colima and Docker status']);
    suggestions.push(['Inspect active VM', 'Inspect resource allocation and disk usage of active VM']);
  }

  return suggestions.slice(0, 3);
}

// ---------------------------------------------------------------------------
// Specification Reference Oracle: Assistant Turn Rebuilder
// ---------------------------------------------------------------------------
export function specRebuildAssistantTurnContent(
  cleanText: string,
  suggests: string[][],
  rawChoices?: string[],
): string {
  let out = cleanText.trim();

  if (rawChoices && rawChoices.length > 0) {
    for (const choice of rawChoices) {
      out += (out ? '\n\n' : '') + choice.trim();
    }
  }

  if (suggests && suggests.length > 0) {
    for (const [label, prompt] of suggests) {
      if (prompt && prompt !== label) {
        out += `\n[SUGGEST: ${label} | ${prompt}]`;
      } else {
        out += `\n[SUGGEST: ${label}]`;
      }
    }
  }

  return out.trim();
}

// ---------------------------------------------------------------------------
// Dynamic Implementation Loader (Contract Adapter)
// ---------------------------------------------------------------------------
export async function getChatMarkdownParser(): Promise<(t: string) => ChatBlock[]> {
  try {
    const modPath = '../../../lib/chatMarkdown';
    const mod = await import(/* @vite-ignore */ modPath);
    if (typeof mod.parseChatMarkdown === 'function') {
      return mod.parseChatMarkdown;
    }
  } catch {
    // Fall back to specification reference oracle
  }
  return specParseChatMarkdown;
}

export async function getToolStepSerializer(): Promise<{
  serializeToolStep: (s: ToolStepPayload) => string;
  parseToolStep: (c: string) => ToolStepPayload | null;
}> {
  try {
    const modPath = '../../../lib/aiToolStep';
    const mod = await import(/* @vite-ignore */ modPath);
    if (
      typeof mod.serializeToolStep === 'function' &&
      typeof mod.parseToolStep === 'function'
    ) {
      return {
        serializeToolStep: mod.serializeToolStep,
        parseToolStep: mod.parseToolStep,
      };
    }
  } catch {
    // Fall back to specification reference oracle
  }
  return {
    serializeToolStep: specSerializeToolStep,
    parseToolStep: specParseToolStep,
  };
}

export async function getAiToolParser(): Promise<{
  extractChoicePrompt: typeof specExtractChoicePrompt;
  stripStreamingPartialTags: typeof specStripStreamingPartialTags;
  rebuildAssistantTurnContent: typeof specRebuildAssistantTurnContent;
}> {
  try {
    const modPath = '../../../lib/aiToolParser';
    const mod = await import(/* @vite-ignore */ modPath);
    return {
      extractChoicePrompt:
        typeof mod.extractChoicePrompt === 'function'
          ? mod.extractChoicePrompt
          : specExtractChoicePrompt,
      stripStreamingPartialTags:
        typeof mod.stripStreamingPartialTags === 'function'
          ? mod.stripStreamingPartialTags
          : specStripStreamingPartialTags,
      rebuildAssistantTurnContent:
        typeof mod.rebuildAssistantTurnContent === 'function'
          ? mod.rebuildAssistantTurnContent
          : specRebuildAssistantTurnContent,
    };
  } catch {
    return {
      extractChoicePrompt: specExtractChoicePrompt,
      stripStreamingPartialTags: specStripStreamingPartialTags,
      rebuildAssistantTurnContent: specRebuildAssistantTurnContent,
    };
  }
}

export async function getAiSuggestionsEngine(): Promise<{
  getStarterPrompts: typeof specGetStarterPrompts;
  deriveFallbackSuggestions: typeof specDeriveFallbackSuggestions;
}> {
  try {
    const modPath = '../../../lib/aiSuggestions';
    const mod = await import(/* @vite-ignore */ modPath);
    return {
      getStarterPrompts:
        typeof mod.getStarterPrompts === 'function'
          ? (ctx: StarterPromptContext) => {
              // Ensure context compatibility between E2E contract fixtures and implementation
              const rawCtx = ctx as unknown as Record<string, unknown>;
              const adaptedCtx = {
                ...ctx,
                instances: ctx.colimaInstances || (rawCtx.instances as typeof ctx.colimaInstances | undefined),
                colimaInstances: ctx.colimaInstances || (rawCtx.instances as typeof ctx.colimaInstances | undefined),
                containers: ctx.containers || (rawCtx.dockerContainers as typeof ctx.containers | undefined),
                dockerContainers: ctx.containers || (rawCtx.dockerContainers as typeof ctx.containers | undefined),
              };
              return mod.getStarterPrompts(adaptedCtx);
            }
          : specGetStarterPrompts,
      deriveFallbackSuggestions:
        typeof mod.deriveFallbackSuggestions === 'function'
          ? (ctx: FallbackSuggestionContext) => {
              return mod.deriveFallbackSuggestions(ctx);
            }
          : specDeriveFallbackSuggestions,
    };
  } catch {
    return {
      getStarterPrompts: specGetStarterPrompts,
      deriveFallbackSuggestions: specDeriveFallbackSuggestions,
    };
  }
}
