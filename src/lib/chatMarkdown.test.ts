/**
 * src/lib/chatMarkdown.test.ts
 *
 * Vitest test suite for chat markdown parsing, think streaming,
 * shell command detection, GFM tables, and XSS sanitization.
 */

import { describe, it, expect } from "vitest";
import {
  escapeHtml,
  sanitizeUrl,
  formatInline,
  isShellCode,
  parseTableDelimiter,
  parseTableRow,
  parseChatMarkdown,
  type CodeBlock,
} from "./chatMarkdown";

describe("chatMarkdown utilities", () => {
  describe("escapeHtml", () => {
    it("escapes all HTML special characters", () => {
      const input = '<script>alert("hello & \'world\'")</script>';
      const expected =
        "&lt;script&gt;alert(&quot;hello &amp; &#039;world&#039;&quot;)&lt;/script&gt;";
      expect(escapeHtml(input)).toBe(expected);
    });

    it("handles null, undefined, and empty string safely", () => {
      expect(escapeHtml("")).toBe("");
      expect(escapeHtml(null)).toBe("");
      expect(escapeHtml(undefined)).toBe("");
    });
  });

  describe("sanitizeUrl", () => {
    it("blocks javascript:, vbscript:, and data: schemes", () => {
      expect(sanitizeUrl("javascript:alert(1)")).toBe("#");
      expect(sanitizeUrl("JAVASCRIPT:alert(1)")).toBe("#");
      expect(sanitizeUrl("data:text/html,<script>alert(1)</script>")).toBe("#");
      expect(sanitizeUrl("vbscript:msgbox(1)")).toBe("#");
    });

    it("allows valid http, https, mailto, and relative urls", () => {
      expect(sanitizeUrl("https://github.com/abiosoft/colima")).toBe(
        "https://github.com/abiosoft/colima"
      );
      expect(sanitizeUrl("http://localhost:1420")).toBe("http://localhost:1420");
      expect(sanitizeUrl("mailto:support@colima.run")).toBe(
        "mailto:support@colima.run"
      );
      expect(sanitizeUrl("/docs/intro")).toBe("/docs/intro");
      expect(sanitizeUrl("#section")).toBe("#section");
    });
  });

  describe("formatInline", () => {
    it("formats bold, italic, and strikethrough", () => {
      expect(formatInline("**bold**")).toBe("<strong>bold</strong>");
      expect(formatInline("__bold__")).toBe("<strong>bold</strong>");
      expect(formatInline("*italic*")).toBe("<em>italic</em>");
      expect(formatInline("_italic_")).toBe("<em>italic</em>");
      expect(formatInline("~~deleted~~")).toBe("<del>deleted</del>");
    });

    it("formats inline code with HTML escaping inside", () => {
      const result = formatInline("Use `<script>alert(1)</script>`");
      expect(result).toBe(
        'Use <code class="ai-inline-code">&lt;script&gt;alert(1)&lt;/script&gt;</code>'
      );
    });

    it("formats markdown links with target=_blank and rel=noopener", () => {
      const input = "[Colima](https://github.com/abiosoft/colima)";
      const expected =
        '<a href="https://github.com/abiosoft/colima" target="_blank" rel="noopener noreferrer" class="ai-link">Colima</a>';
      expect(formatInline(input)).toBe(expected);
    });

    it("supports markdown links containing balanced parentheses in URLs", () => {
      const input = "[Colima Wiki](https://en.wikipedia.org/wiki/Colima_(software))";
      const result = formatInline(input);
      expect(result).toContain(
        'href="https://en.wikipedia.org/wiki/Colima_(software)"'
      );
      expect(result).toContain(">Colima Wiki</a>");
    });

    it("sanitizes malicious javascript: links inside markdown syntax", () => {
      const input = "[Click for free RAM](javascript:alert(document.cookie))";
      const result = formatInline(input);
      expect(result).toContain('href="#"');
      expect(result).not.toContain("javascript:");
    });
  });

  describe("isShellCode", () => {
    it("returns true for explicit shell languages", () => {
      expect(isShellCode("bash", "echo hello")).toBe(true);
      expect(isShellCode("sh", "echo hello")).toBe(true);
      expect(isShellCode("zsh", "echo hello")).toBe(true);
      expect(isShellCode("shell", "echo hello")).toBe(true);
      expect(isShellCode("terminal", "echo hello")).toBe(true);
      expect(isShellCode("console", "echo hello")).toBe(true);
    });

    it("returns true for Colima CLI and container languages", () => {
      expect(isShellCode("colima", "colima status")).toBe(true);
      expect(isShellCode("docker", "docker ps")).toBe(true);
      expect(isShellCode("kubectl", "kubectl get pods")).toBe(true);
      expect(isShellCode("limactl", "limactl list")).toBe(true);
      expect(isShellCode("helm", "helm list")).toBe(true);
    });

    it("detects shell commands when language is omitted or text", () => {
      expect(isShellCode("", "colima start --cpu 4")).toBe(true);
      expect(isShellCode("text", "$ colima status")).toBe(true);
      expect(isShellCode("txt", "# docker restart\ndocker restart my-app")).toBe(
        true
      );
      expect(isShellCode("", "sudo nerdctl run -d alpine")).toBe(true);
      expect(isShellCode("", "brew install colima")).toBe(true);
      expect(isShellCode("", "curl -sL https://colima.run")).toBe(true);
    });

    it("returns false for non-shell languages and code", () => {
      expect(isShellCode("python", "import sys\nprint(sys.version)")).toBe(false);
      expect(isShellCode("json", '{"name": "colima"}')).toBe(false);
      expect(isShellCode("yaml", "services:\n  web:\n    image: nginx")).toBe(
        false
      );
      expect(isShellCode("typescript", "const x: number = 10;")).toBe(false);
      expect(isShellCode("", "This is just a regular sentence about colima.")).toBe(
        false
      );
    });
  });

  describe("parseTableDelimiter and parseTableRow", () => {
    it("parses alignments accurately", () => {
      expect(parseTableDelimiter("| :--- | :---: | ---: | --- |")).toEqual([
        "left",
        "center",
        "right",
        "none",
      ]);
      expect(parseTableDelimiter("--- | :---:")).toEqual(["none", "center"]);
      expect(parseTableDelimiter("| not | delimiter |")).toBeNull();
    });

    it("parses table row and pads missing columns", () => {
      expect(parseTableRow("| Col A | Col B |", 3)).toEqual([
        "Col A",
        "Col B",
        "",
      ]);
      expect(parseTableRow("| Col A | Col B | Col C | Col D |", 2)).toEqual([
        "Col A",
        "Col B",
      ]);
    });

    it("handles escaped pipes inside cells", () => {
      const row = "| Command | `ls \\| grep foo` |";
      const parsed = parseTableRow(row, 2);
      expect(parsed[1]).toContain("ls | grep foo");
    });
  });
});

describe("parseChatMarkdown AST parsing", () => {
  it("returns empty array for empty or whitespace text", () => {
    expect(parseChatMarkdown("")).toEqual([]);
    expect(parseChatMarkdown("   \n\n  ")).toEqual([]);
  });

  describe("reasoning blocks (<think>)", () => {
    it("emits streaming: true when <think> is unclosed during streaming", () => {
      const input = "<think>\nAnalyzing Colima configuration...\nChecking memory";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "Analyzing Colima configuration...\nChecking memory",
        streaming: true,
      });
    });

    it("emits streaming: false when </think> arrives", () => {
      const input =
        "<think>\nAnalysis complete.\n</think>\nColima is healthy and running.";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(2);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "Analysis complete.",
        streaming: false,
      });
      expect(blocks[1]).toEqual({
        type: "paragraph",
        text: "Colima is healthy and running.",
      });
    });

    it("handles single-line inline <think> tags", () => {
      const input = "Lead<think>quick thought</think>Follow up.";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(3);
      expect(blocks[0]).toEqual({ type: "paragraph", text: "Lead" });
      expect(blocks[1]).toEqual({
        type: "think",
        content: "quick thought",
        streaming: false,
      });
      expect(blocks[2]).toEqual({ type: "paragraph", text: "Follow up." });
    });

    it("handles multiple inline <think> tags on a single line", () => {
      const input = "<think>first thought</think><think>second thought</think>";
      const blocks = parseChatMarkdown(input);
      expect(blocks).toEqual([
        { type: "think", content: "first thought", streaming: false },
        { type: "think", content: "second thought", streaming: false },
      ]);
    });

    it("handles multiple inline <think> tags interspersed with prose", () => {
      const input = "Intro <think>first</think> middle <think>second</think> outro";
      const blocks = parseChatMarkdown(input);
      expect(blocks).toHaveLength(5);
      expect(blocks[0]).toEqual({ type: "paragraph", text: "Intro" });
      expect(blocks[1]).toEqual({ type: "think", content: "first", streaming: false });
      expect(blocks[2]).toEqual({ type: "paragraph", text: "middle" });
      expect(blocks[3]).toEqual({ type: "think", content: "second", streaming: false });
      expect(blocks[4]).toEqual({ type: "paragraph", text: "outro" });
    });

    it("preserves code fences written inside reasoning blocks", () => {
      const input = "<think>\nLet us test:\n```bash\ncolima start\n```\n</think>";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "Let us test:\n```bash\ncolima start\n```",
        streaming: false,
      });
    });

    it("does not treat <think> as reasoning when inside a fenced code block", () => {
      const input = "```xml\n<think>not a real think block</think>\n```";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "code",
        language: "xml",
        code: "<think>not a real think block</think>",
        isShell: false,
      });
    });
  });

  describe("code blocks (```)", () => {
    it("parses fenced code blocks and identifies shell execution suitability", () => {
      const input = "```bash\ncolima start --cpu 4 --memory 8\n```";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "code",
        language: "bash",
        code: "colima start --cpu 4 --memory 8",
        isShell: true,
      });
    });

    it("emits code blocks while streaming when the block is unclosed", () => {
      const input = "Here is the command:\n```colima\ncolima restart";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(2);
      expect(blocks[0]).toEqual({
        type: "paragraph",
        text: "Here is the command:",
      });
      expect(blocks[1]).toEqual({
        type: "code",
        language: "colima",
        code: "colima restart",
        isShell: true,
      });
    });

    it("preserves raw unescaped code string in AST for clipboard copy", () => {
      const input = '```sh\necho "Hello <world> & \'friends\'"\n```';
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as CodeBlock).code).toBe('echo "Hello <world> & \'friends\'"');
    });
  });

  describe("GFM tables", () => {
    it("parses full GFM table with headers, alignments, and formatted rows", () => {
      const input = [
        "| Profile | Status | Architecture |",
        "| :--- | :---: | ---: |",
        "| `default` | **Running** | aarch64 |",
        "| `k8s` | Stopped | x86_64 |",
      ].join("\n");

      const blocks = parseChatMarkdown(input);
      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "table",
        headers: ["Profile", "Status", "Architecture"],
        alignments: ["left", "center", "right"],
        rows: [
          [
            '<code class="ai-inline-code">default</code>',
            "<strong>Running</strong>",
            "aarch64",
          ],
          ['<code class="ai-inline-code">k8s</code>', "Stopped", "x86_64"],
        ],
      });
    });

    it("emits empty rows array when only header and delimiter have streamed", () => {
      const input = "| VM | Cores |\n| --- | --- |";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "table",
        headers: ["VM", "Cores"],
        alignments: ["none", "none"],
        rows: [],
      });
    });

    it("does not treat plain text with pipes as table if delimiter is absent", () => {
      const input = "Use the pipe operator | to chain commands.";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "paragraph",
        text: "Use the pipe operator | to chain commands.",
      });
    });
  });

  describe("headings, lists, blockquotes, and horizontal rules", () => {
    it("parses markdown headings 1 to 6", () => {
      const input = "# Title 1\n## Title 2\n### Title 3";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(3);
      expect(blocks[0]).toEqual({ type: "heading", level: 1, text: "Title 1" });
      expect(blocks[1]).toEqual({ type: "heading", level: 2, text: "Title 2" });
      expect(blocks[2]).toEqual({ type: "heading", level: 3, text: "Title 3" });
    });

    it("parses unordered lists", () => {
      const input = "- Item 1\n- Item 2 with **bold**\n* Item 3";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "list",
        ordered: false,
        items: ["Item 1", "Item 2 with <strong>bold</strong>", "Item 3"],
      });
    });

    it("parses ordered lists with starting index", () => {
      const input = "1. First step\n2. Second step\n3. Third step";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "list",
        ordered: true,
        start: 1,
        items: ["First step", "Second step", "Third step"],
      });
    });

    it("parses blockquotes", () => {
      const input = "> Notice: Colima requires Lima daemon.\n> Please ensure Lima is running.";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "blockquote",
        text: "Notice: Colima requires Lima daemon.<br />Please ensure Lima is running.",
      });
    });

    it("parses horizontal rules", () => {
      const input = "Before\n---\nAfter";
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(3);
      expect(blocks[0]).toEqual({ type: "paragraph", text: "Before" });
      expect(blocks[1]).toEqual({ type: "hr" });
      expect(blocks[2]).toEqual({ type: "paragraph", text: "After" });
    });
  });

  describe("XSS Prevention and Sanitization", () => {
    it("escapes script injection attempts in paragraphs", () => {
      const input = '<script>alert("XSS")</script>';
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "paragraph",
        text: "&lt;script&gt;alert(&quot;XSS&quot;)&lt;/script&gt;",
      });
    });

    it("escapes img onerror handlers in headings", () => {
      const input = '## Warning <img src="x" onerror="alert(1)">';
      const blocks = parseChatMarkdown(input);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "heading",
        level: 2,
        text: "Warning &lt;img src=&quot;x&quot; onerror=&quot;alert(1)&quot;&gt;",
      });
    });

    it("sanitizes iframe injection", () => {
      const input = '<iframe src="https://evil.com"></iframe>';
      const blocks = parseChatMarkdown(input);

      expect(blocks[0]).toEqual({
        type: "paragraph",
        text: "&lt;iframe src=&quot;https://evil.com&quot;&gt;&lt;/iframe&gt;",
      });
    });
  });

  describe("Simulated DeepSeek-R1 Streaming Lifecycle", () => {
    it("smoothly updates AST through all 4 streaming phases without UI freeze", () => {
      // Phase 1: Only <think> arrives
      const chunk1 = "<think>";
      const blocks1 = parseChatMarkdown(chunk1);
      expect(blocks1).toEqual([{ type: "think", content: "", streaming: true }]);

      // Phase 2: Reasoning content streams in
      const chunk2 = "<think>\nInspecting docker daemon status...\nChecking VM";
      const blocks2 = parseChatMarkdown(chunk2);
      expect(blocks2).toEqual([
        {
          type: "think",
          content: "Inspecting docker daemon status...\nChecking VM",
          streaming: true,
        },
      ]);

      // Phase 3: Reasoning closes, answer begins
      const chunk3 =
        "<think>\nInspecting docker daemon status...\nChecking VM\n</think>\nHere is the command to restart:";
      const blocks3 = parseChatMarkdown(chunk3);
      expect(blocks3).toEqual([
        {
          type: "think",
          content: "Inspecting docker daemon status...\nChecking VM",
          streaming: false,
        },
        {
          type: "paragraph",
          text: "Here is the command to restart:",
        },
      ]);

      // Phase 4: Final response with shell code block
      const chunk4 =
        "<think>\nInspecting docker daemon status...\nChecking VM\n</think>\nHere is the command to restart:\n```bash\ncolima restart\n```";
      const blocks4 = parseChatMarkdown(chunk4);
      expect(blocks4).toEqual([
        {
          type: "think",
          content: "Inspecting docker daemon status...\nChecking VM",
          streaming: false,
        },
        {
          type: "paragraph",
          text: "Here is the command to restart:",
        },
        {
          type: "code",
          language: "bash",
          code: "colima restart",
          isShell: true,
        },
      ]);
    });
  });
});
