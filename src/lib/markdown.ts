export function escapeHtml(str: string): string {
  if (!str) return "";
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function renderMarkdown(text: string): string {
  if (!text) return "";
  const lines = text.split("\n");
  let htmlResult = "";
  let inCode = false;
  let codeBlock = "";
  let inThink = false;
  let thinkBlock = "";

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.includes("<think>")) {
      inThink = true;
      thinkBlock = line.replace(/.*<think>/, "");
      continue;
    }
    if (inThink) {
      if (line.includes("</think>")) {
        inThink = false;
        thinkBlock += "\n" + line.replace(/<\/think>.*/, "");
        htmlResult += `
          <div class="ai-think-block">
            <div class="ai-think-header">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" /></svg>
              Thinking Process
            </div>
            <div class="ai-think-content">${escapeHtml(thinkBlock.trim())}</div>
          </div>`;
        thinkBlock = "";
      } else {
        thinkBlock += "\n" + line;
      }
      continue;
    }

    if (line.trim().startsWith("```")) {
      if (inCode) {
        htmlResult += `<pre><code>${escapeHtml(codeBlock)}</code></pre>`;
        inCode = false;
        codeBlock = "";
      } else {
        inCode = true;
      }
      continue;
    }

    if (inCode) {
      codeBlock += line + "\n";
      continue;
    }

    let html = escapeHtml(line);

    // Links [text](target): in the offline KB the target is an article slug
    // that Help.svelte reads back from a[data-slug] to navigate.
    html = html.replace(
      /\[([^\]]+)\]\(([^)\s]+)\)/g,
      '<a class="md-link" data-slug="$2">$1</a>'
    );

    html = html.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    html = html.replace(/`([^`]+)`/g, '<code style="background:rgba(255,255,255,0.06);padding:1px 4px;border-radius:3px;font-size:0.72rem">$1</code>');

    if (html.startsWith("#### ")) {
      htmlResult += `<h4 class="md-h">${html.slice(5)}</h4>`;
      continue;
    }
    if (html.startsWith("### ")) {
      htmlResult += `<h4 class="md-h">${html.slice(4)}</h4>`;
      continue;
    }
    if (html.startsWith("## ")) {
      htmlResult += `<h3 class="md-h">${html.slice(3)}</h3>`;
      continue;
    }
    if (html.startsWith("# ")) {
      htmlResult += `<h2 class="md-h md-h-title">${html.slice(2)}</h2>`;
      continue;
    }
    if (html.startsWith("- ")) html = `• ${html.slice(2)}`;
    html = html.replace(/^(\d+)\.\s/, "$1. ");

    if (html.trim() === "") {
      htmlResult += "<br />";
    } else {
      htmlResult += `<p>${html}</p>`;
    }
  }

  if (inCode && codeBlock) {
    htmlResult += `<pre><code>${escapeHtml(codeBlock)}</code></pre>`;
  }

  return htmlResult;
}
