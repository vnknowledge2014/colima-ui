import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import AiMessageBody from "./AiMessageBody.svelte";

afterEach(cleanup);

describe("AiMessageBody.svelte", () => {
  it("renders active streaming think block with open details and pulsing indicator", () => {
    const content = "<think>Analyzing container resources...";
    render(AiMessageBody, { content });

    const details = document.querySelector(".ai-think-details") as HTMLDetailsElement;
    expect(details).toBeInTheDocument();
    expect(details.open).toBe(true);
    expect(screen.getByText("Thinking…")).toBeInTheDocument();
    expect(screen.getByText("Analyzing container resources...")).toBeInTheDocument();
  });

  it("renders completed think block with collapsed details and thought process summary", () => {
    const content = "<think>Resource analysis complete.</think>\n\nAll containers are healthy.";
    render(AiMessageBody, { content });

    const details = document.querySelector(".ai-think-details") as HTMLDetailsElement;
    expect(details).toBeInTheDocument();
    expect(details.open).toBe(false);
    expect(screen.getByText("Thought process")).toBeInTheDocument();
    expect(screen.getByText("All containers are healthy.")).toBeInTheDocument();
  });

  it("renders code blocks and forwards onInsert callback", async () => {
    const onInsert = vi.fn();
    const content = "Run this command:\n```bash\ncolima status\n```";
    render(AiMessageBody, { content, onInsert });

    expect(screen.getByText("colima status")).toBeInTheDocument();
    expect(screen.getByText("bash")).toBeInTheDocument();

    const insertBtn = screen.getByRole("button", { name: /insert/i });
    await fireEvent.click(insertBtn);
    expect(onInsert).toHaveBeenCalledWith("colima status");
  });

  it("renders GFM responsive tables with headers and aligned rows", () => {
    const content = `| Name | Status | Replicas |
| :--- | :---: | ---: |
| web | running | 3 |
| db | stopped | 1 |`;

    render(AiMessageBody, { content });

    const tableContainer = document.querySelector(".ai-table-container");
    expect(tableContainer).toBeInTheDocument();

    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByText("Status")).toBeInTheDocument();
    expect(screen.getByText("Replicas")).toBeInTheDocument();

    expect(screen.getByText("web")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("db")).toBeInTheDocument();
    expect(screen.getByText("stopped")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("renders markdown headings across levels 1 through 6", () => {
    const content = `# Title 1
## Title 2
### Title 3
#### Title 4
##### Title 5
###### Title 6`;

    render(AiMessageBody, { content });

    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Title 1");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Title 2");
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("Title 3");
    expect(screen.getByRole("heading", { level: 4 })).toHaveTextContent("Title 4");
    expect(screen.getByRole("heading", { level: 5 })).toHaveTextContent("Title 5");
    expect(screen.getByRole("heading", { level: 6 })).toHaveTextContent("Title 6");
  });

  it("renders ordered and unordered lists", () => {
    const content = `Tasks:
- First task
- Second task

Steps:
1. Step one
2. Step two`;

    render(AiMessageBody, { content });

    expect(screen.getByText("First task")).toBeInTheDocument();
    expect(screen.getByText("Second task")).toBeInTheDocument();
    expect(screen.getByText("Step one")).toBeInTheDocument();
    expect(screen.getByText("Step two")).toBeInTheDocument();

    expect(document.querySelector("ul.ai-list-unordered")).toBeInTheDocument();
    expect(document.querySelector("ol.ai-list-ordered")).toBeInTheDocument();
  });

  it("renders blockquotes and horizontal rules", () => {
    const content = `> Important notice here.\n\n---\n\nNext section.`;
    render(AiMessageBody, { content });

    expect(document.querySelector("blockquote.ai-blockquote")).toHaveTextContent("Important notice here.");
    expect(document.querySelector("hr.ai-hr")).toBeInTheDocument();
  });

  it("safely sanitizes potential XSS injection vectors", () => {
    const content = `<script>window.__pwned = true;</script>
<img src="x" onerror="alert(1)" />
[Malicious](javascript:alert(1))`;

    render(AiMessageBody, { content });

    expect(document.querySelector("script")).toBeNull();
    // JavaScript protocol URL should be sanitized
    const link = document.querySelector("a");
    if (link) {
      expect(link.getAttribute("href")).not.toContain("javascript:");
    }
  });

  it("handles empty or whitespace-only markdown gracefully", () => {
    const { container } = render(AiMessageBody, { content: "" });
    expect(container.firstChild).toBeInTheDocument();
  });
});
