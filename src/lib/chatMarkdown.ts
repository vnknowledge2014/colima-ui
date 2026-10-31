/**
 * src/lib/chatMarkdown.ts
 *
 * Modular AST parser for AI Chat messages with streaming <think> handling,
 * GFM tables, shell code block detection, and XSS sanitization.
 */

export type TableAlignment = "left" | "center" | "right" | "none";

export interface ThinkBlock {
  type: "think";
  content: string;
  streaming: boolean;
}

export interface CodeBlock {
  type: "code";
  language: string;
  code: string;
  isShell: boolean;
}

export interface TableBlock {
  type: "table";
  headers: string[];
  alignments: TableAlignment[];
  rows: string[][];
}

export interface HeadingBlock {
  type: "heading";
  level: 1 | 2 | 3 | 4 | 5 | 6;
  text: string;
}

export interface ListBlock {
  type: "list";
  ordered: boolean;
  start?: number;
  items: string[];
}

export interface BlockquoteBlock {
  type: "blockquote";
  text: string;
}

export interface HrBlock {
  type: "hr";
}

export interface ParagraphBlock {
  type: "paragraph";
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

export function escapeHtml(str: string | null | undefined): string {
  if (!str) return "";
  if (!/[&<>"']/.test(str)) return str;
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function sanitizeUrl(url: string | null | undefined): string {
  const trimmed = (url || "").trim();
  if (/^(?:javascript|vbscript|data):/i.test(trimmed)) {
    return "#";
  }
  return trimmed;
}

function applyInlineFormatting(text: string): string {
  let res = text;
  // Bold: **text** or __text__
  if (res.includes("**")) {
    res = res.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  }
  if (res.includes("__")) {
    res = res.replace(/__([^_]+)__/g, "<strong>$1</strong>");
  }

  // Italic: *text* or _text_
  if (res.includes("*")) {
    res = res.replace(/(^|[^*])\*([^*]+)\*(?!\*)/g, "$1<em>$2</em>");
  }
  if (res.includes("_")) {
    res = res.replace(/(^|[^_])_([^_]+)_(?!_)/g, "$1<em>$2</em>");
  }

  // Strikethrough: ~~text~~
  if (res.includes("~~")) {
    res = res.replace(/~~([^~]+)~~/g, "<del>$1</del>");
  }

  return res;
}

export function formatInline(text: string): string {
  if (!text) return "";
  let html = escapeHtml(text);

  // Step 1: Protect inline code blocks with placeholder tokens.
  // Note: Tokens must NOT contain underscores, asterisks, or tildes to prevent
  // downstream italic/bold regexes from matching placeholder characters.
  const codeTokens: string[] = [];
  if (html.includes("`")) {
    html = html.replace(/`([^`]+)`/g, (_match, code) => {
      const token = `\uE000CODE${codeTokens.length}\uE000`;
      codeTokens.push(`<code class="ai-inline-code">${code}</code>`);
      return token;
    });
  }

  // Step 2: Extract and replace Markdown links with placeholder tokens.
  // URLs are sanitized and preserved verbatim without inline formatting corruption.
  // Link labels are formatted safely.
  const linkTokens: string[] = [];
  if (html.includes("[") && html.includes("]") && html.includes("(")) {
    const linkRegex = /\[([^\][]+)\]\(((?:[^()\s]|\([^()\s]*\))+)\)/g;
    html = html.replace(linkRegex, (_match, label, url) => {
      const safeUrl = sanitizeUrl(url);
      const formattedLabel = applyInlineFormatting(label);
      const token = `\uE000LINK${linkTokens.length}\uE000`;
      linkTokens.push(
        `<a href="${safeUrl}" target="_blank" rel="noopener noreferrer" class="ai-link">${formattedLabel}</a>`
      );
      return token;
    });
  }

  // Step 3: Apply bold, italic, and strikethrough formatting to remaining prose
  html = applyInlineFormatting(html);

  // Step 4: Restore link tokens and code tokens in order
  if (linkTokens.length > 0) {
    html = html.replace(/\uE000LINK(\d+)\uE000/g, (_, i) => linkTokens[Number(i)]);
  }
  if (codeTokens.length > 0) {
    html = html.replace(/\uE000CODE(\d+)\uE000/g, (_, i) => codeTokens[Number(i)]);
  }

  return html;
}

const SHELL_LANGUAGES = new Set([
  "bash",
  "sh",
  "zsh",
  "shell",
  "console",
  "terminal",
  "zshrc",
  "bashrc",
  "docker",
  "colima",
  "kubectl",
  "limactl",
  "helm",
]);

const SHELL_COMMAND_PREFIXES = [
  "colima",
  "docker",
  "kubectl",
  "limactl",
  "helm",
  "nerdctl",
  "sudo",
  "systemctl",
  "brew",
  "curl",
  "git",
  "ssh",
];

export function isShellCode(language: string, code: string): boolean {
  const lang = (language || "").toLowerCase().trim();
  if (SHELL_LANGUAGES.has(lang)) {
    return true;
  }

  // If language is empty or generic, check the first executable command line
  if (!lang || lang === "text" || lang === "txt") {
    const lines = (code || "").split("\n");
    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#") || line.startsWith("//")) {
        continue;
      }
      const stripped = line.replace(/^[$#>]\s*/, "").trim();
      if (!stripped) continue;

      const firstWord = stripped.split(/\s+/)[0];
      if (SHELL_COMMAND_PREFIXES.includes(firstWord)) {
        return true;
      }
      break;
    }
  }

  return false;
}

export function parseTableDelimiter(line: string): TableAlignment[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes("-")) return null;

  let inner = trimmed;
  if (inner.startsWith("|")) inner = inner.slice(1);
  if (inner.endsWith("|")) inner = inner.slice(0, -1);

  const cells = inner.split("|").map((c) => c.trim());
  if (cells.length === 0) return null;

  const alignments: TableAlignment[] = [];
  for (const cell of cells) {
    if (!/^:?-+:?$/.test(cell)) {
      return null;
    }
    const leftColon = cell.startsWith(":");
    const rightColon = cell.endsWith(":");
    if (leftColon && rightColon) {
      alignments.push("center");
    } else if (rightColon) {
      alignments.push("right");
    } else if (leftColon) {
      alignments.push("left");
    } else {
      alignments.push("none");
    }
  }

  return alignments;
}

export function parseTableRow(line: string, colCount?: number): string[] {
  let inner = line.trim();
  if (inner.startsWith("|")) inner = inner.slice(1);
  if (inner.endsWith("|")) inner = inner.slice(0, -1);

  // Split on unescaped pipes
  const cells = inner
    .split(/(?<!\\)\|/)
    .map((c) => formatInline(c.trim().replace(/\\\|/g, "|")));

  if (colCount !== undefined) {
    while (cells.length < colCount) {
      cells.push("");
    }
    if (cells.length > colCount) {
      return cells.slice(0, colCount);
    }
  }

  return cells;
}

export function parseChatMarkdown(text: string): ChatBlock[] {
  if (!text) return [];

  const blocks: ChatBlock[] = [];
  const lines = text.split("\n");

  let inCode = false;
  let codeLang = "";
  let codeLines: string[] = [];

  let inThink = false;
  let thinkLines: string[] = [];

  let inList = false;
  let listOrdered = false;
  let listStart: number | undefined = undefined;
  let listItems: string[] = [];

  let inBlockquote = false;
  let blockquoteLines: string[] = [];

  let paragraphLines: string[] = [];

  function flushParagraph() {
    if (paragraphLines.length > 0) {
      blocks.push({
        type: "paragraph",
        text: paragraphLines.map(formatInline).join("<br />"),
      });
      paragraphLines = [];
    }
  }

  function flushList() {
    if (inList && listItems.length > 0) {
      blocks.push({
        type: "list",
        ordered: listOrdered,
        start: listStart,
        items: [...listItems],
      });
      inList = false;
      listOrdered = false;
      listStart = undefined;
      listItems = [];
    }
  }

  function flushBlockquote() {
    if (inBlockquote && blockquoteLines.length > 0) {
      blocks.push({
        type: "blockquote",
        text: blockquoteLines.map(formatInline).join("<br />"),
      });
      inBlockquote = false;
      blockquoteLines = [];
    }
  }

  function flushAllTextBlocks() {
    flushParagraph();
    flushList();
    flushBlockquote();
  }

  let i = 0;
  while (i < lines.length) {
    let line = lines[i];

    // 1. In Code Block
    if (inCode) {
      if (line.trim().startsWith("```")) {
        inCode = false;
        const code = codeLines.join("\n");
        blocks.push({
          type: "code",
          language: codeLang,
          code,
          isShell: isShellCode(codeLang, code),
        });
        codeLines = [];
        codeLang = "";
      } else {
        codeLines.push(line);
      }
      i++;
      continue;
    }

    // 2. In Think Block
    if (inThink) {
      const thinkCloseIdx = line.indexOf("</think>");
      if (thinkCloseIdx !== -1) {
        const thinkEndPart = line.slice(0, thinkCloseIdx);
        if (thinkEndPart) thinkLines.push(thinkEndPart);
        inThink = false;
        blocks.push({
          type: "think",
          content: thinkLines.join("\n").trim(),
          streaming: false,
        });
        thinkLines = [];
        line = line.slice(thinkCloseIdx + 8);
        if (!line.trim()) {
          i++;
          continue;
        }
      } else {
        thinkLines.push(line);
        i++;
        continue;
      }
    }

    // 3. Code Block Opening
    if (line.trim().startsWith("```")) {
      flushAllTextBlocks();
      inCode = true;
      codeLang = (line.trim().slice(3).trim().split(/\s+/)[0] || "").toLowerCase();
      codeLines = [];
      i++;
      continue;
    }

    // 4. Think Block Opening (<think>)
    while (line.indexOf("<think>") !== -1) {
      const thinkOpenIdx = line.indexOf("<think>");
      const beforeThink = line.slice(0, thinkOpenIdx);
      if (beforeThink.trim()) {
        paragraphLines.push(beforeThink.trim());
      }
      flushAllTextBlocks();

      const afterOpen = line.slice(thinkOpenIdx + 7);
      const closeIdx = afterOpen.indexOf("</think>");
      if (closeIdx !== -1) {
        // Complete think block on the same line
        const thinkContent = afterOpen.slice(0, closeIdx);
        blocks.push({
          type: "think",
          content: thinkContent.trim(),
          streaming: false,
        });
        line = afterOpen.slice(closeIdx + 8).trim();
      } else {
        // Streaming unclosed think block starts here
        inThink = true;
        thinkLines = afterOpen ? [afterOpen] : [];
        line = "";
        break;
      }
    }

    if (inThink) {
      i++;
      continue;
    }

    // 5. Blank line
    if (!line.trim()) {
      flushAllTextBlocks();
      i++;
      continue;
    }

    // 6. Horizontal Rule (---, ***, ___)
    if (/^(?:---|\*\*\*|___)\s*$/.test(line.trim())) {
      flushAllTextBlocks();
      blocks.push({ type: "hr" });
      i++;
      continue;
    }

    // 7. Headings (# Title)
    const headingMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      flushAllTextBlocks();
      const level = headingMatch[1].length as 1 | 2 | 3 | 4 | 5 | 6;
      blocks.push({
        type: "heading",
        level,
        text: formatInline(headingMatch[2].trim()),
      });
      i++;
      continue;
    }

    // 8. GFM Table Check
    if (line.includes("|") && i + 1 < lines.length) {
      const alignments = parseTableDelimiter(lines[i + 1]);
      if (alignments) {
        flushAllTextBlocks();
        const headers = parseTableRow(line, alignments.length);
        const rows: string[][] = [];
        i += 2; // skip header and delimiter lines

        while (i < lines.length) {
          const tableLine = lines[i];
          if (!tableLine.trim() || !tableLine.includes("|")) {
            break;
          }
          rows.push(parseTableRow(tableLine, alignments.length));
          i++;
        }

        blocks.push({
          type: "table",
          headers,
          alignments,
          rows,
        });
        continue;
      }
    }

    // 9. Blockquotes (> quote)
    const quoteMatch = line.match(/^>\s?(.*)$/);
    if (quoteMatch) {
      flushParagraph();
      flushList();
      inBlockquote = true;
      blockquoteLines.push(quoteMatch[1]);
      i++;
      continue;
    } else {
      flushBlockquote();
    }

    // 10. List Items
    const unorderedMatch = line.match(/^(\s*)[-*+]\s+(.+)$/);
    const orderedMatch = line.match(/^(\s*)(\d+)\.\s+(.+)$/);

    if (unorderedMatch) {
      flushParagraph();
      if (inList && listOrdered) {
        flushList();
      }
      inList = true;
      listOrdered = false;
      listItems.push(formatInline(unorderedMatch[2].trim()));
      i++;
      continue;
    } else if (orderedMatch) {
      flushParagraph();
      if (inList && !listOrdered) {
        flushList();
      }
      if (!inList) {
        inList = true;
        listOrdered = true;
        listStart = parseInt(orderedMatch[2], 10);
      }
      listItems.push(formatInline(orderedMatch[3].trim()));
      i++;
      continue;
    } else {
      flushList();
    }

    // 11. Normal paragraph line
    paragraphLines.push(line);
    i++;
  }

  // Final flush
  flushAllTextBlocks();

  if (inCode) {
    const code = codeLines.join("\n");
    blocks.push({
      type: "code",
      language: codeLang,
      code,
      isShell: isShellCode(codeLang, code),
    });
  }

  if (inThink) {
    blocks.push({
      type: "think",
      content: thinkLines.join("\n").trim(),
      streaming: true,
    });
  }

  return blocks;
}
