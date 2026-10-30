<script lang="ts">
  import { parseChatMarkdown, type ChatBlock } from "../../lib/chatMarkdown";
  import CodeBlock from "./CodeBlock.svelte";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    content: string;
    onInsert?: (code: string) => void;
    class?: string;
  }

  let { content, onInsert, class: className = "" }: Props = $props();
  const blocks = $derived<ChatBlock[]>(parseChatMarkdown(content));

  function tr(key: string, fallback: string): string {
    const res = t(key);
    return res && res !== key ? res : fallback;
  }

  function plainHeaderText(html: string): string {
    if (!html) return "";
    return html
      .replace(/<[^>]*>/g, "")
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&amp;/g, "&")
      .trim();
  }
</script>

<div class="ai-message-body {className}">
  {#each blocks as block, idx (idx)}
    {#if block.type === "think"}
      <details class="ai-think-details" open={block.streaming}>
        <summary class="ai-think-summary">
          <span class="ai-think-header-left">
            {#if block.streaming}
              <span class="ai-think-spinner" aria-hidden="true">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="ai-spin" aria-hidden="true">
                  <path d="M21 12a9 9 0 1 1-6.219-8.56" />
                </svg>
              </span>
              <span class="ai-think-title ai-pulsing">{tr("ai.thinking", "Thinking…")}</span>
            {:else}
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
              </svg>
              <span class="ai-think-title">{tr("ai.thought_process", "Thought process")}</span>
            {/if}
          </span>
          <span class="ai-think-chevron" aria-hidden="true">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </span>
        </summary>
        <div class="ai-think-content">
          <pre class="ai-think-pre" tabindex="0"><code>{block.content}</code></pre>
        </div>
      </details>
    {:else if block.type === "code"}
      <CodeBlock
        code={block.code}
        language={block.language}
        isShell={block.isShell}
        {onInsert}
      />
    {:else if block.type === "table"}
      <div
        class="ai-table-container"
        tabindex="0"
        role="region"
        aria-label={tr("ai.data_table", "Data table")}
      >
        <table class="ai-table">
          {#if block.headers.length > 0}
            <thead>
              <tr>
                {#each block.headers as header, colIdx (colIdx)}
                  <th scope="col" class="ai-th text-{block.alignments[colIdx] || 'left'}">
                    {@html header}
                  </th>
                {/each}
              </tr>
            </thead>
          {/if}
          <tbody>
            {#each block.rows as row, rowIdx (rowIdx)}
              <tr class="ai-tr">
                {#each row as cell, colIdx (colIdx)}
                  <td
                    class="ai-td text-{block.alignments[colIdx] || 'left'}"
                    data-label={plainHeaderText(block.headers[colIdx] || "")}
                  >
                    {@html cell}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if block.type === "heading"}
      {#if block.level === 1}
        <h1 class="ai-heading ai-h1">{@html block.text}</h1>
      {:else if block.level === 2}
        <h2 class="ai-heading ai-h2">{@html block.text}</h2>
      {:else if block.level === 3}
        <h3 class="ai-heading ai-h3">{@html block.text}</h3>
      {:else if block.level === 4}
        <h4 class="ai-heading ai-h4">{@html block.text}</h4>
      {:else if block.level === 5}
        <h5 class="ai-heading ai-h5">{@html block.text}</h5>
      {:else}
        <h6 class="ai-heading ai-h6">{@html block.text}</h6>
      {/if}
    {:else if block.type === "list"}
      {#if block.ordered}
        <ol class="ai-list ai-list-ordered" start={block.start ?? 1}>
          {#each block.items as item, itemIdx (itemIdx)}
            <li class="ai-list-item">{@html item}</li>
          {/each}
        </ol>
      {:else}
        <ul class="ai-list ai-list-unordered">
          {#each block.items as item, itemIdx (itemIdx)}
            <li class="ai-list-item">{@html item}</li>
          {/each}
        </ul>
      {/if}
    {:else if block.type === "blockquote"}
      <blockquote class="ai-blockquote">
        {@html block.text}
      </blockquote>
    {:else if block.type === "hr"}
      <hr class="ai-hr" />
    {:else if block.type === "paragraph"}
      <p class="ai-paragraph">
        {@html block.text}
      </p>
    {/if}
  {/each}
</div>

<style>
  .ai-message-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: var(--text-sm);
    line-height: 1.6;
    color: var(--text-primary);
    word-break: break-word;
  }

  .ai-paragraph {
    margin: 0;
    color: var(--text-primary);
  }

  /* Inline elements styled by chatMarkdown */
  :global(.ai-inline-code) {
    font-family: var(--font-mono);
    font-size: 0.85em;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    background: var(--surface-inset);
    border: 1px solid var(--border-subtle);
    color: var(--accent-blue);
  }

  :global(.ai-link) {
    color: var(--accent-blue);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  :global(.ai-link:hover) {
    color: var(--text-primary);
  }

  /* Reasoning / Think details */
  .ai-think-details {
    margin: 4px 0 8px 0;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-card);
    overflow: hidden;
  }

  .ai-think-summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 12px;
    background: var(--bg-secondary);
    cursor: pointer;
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-secondary);
    user-select: none;
    list-style: none;
  }

  .ai-think-summary::-webkit-details-marker {
    display: none;
  }

  .ai-think-summary:hover {
    color: var(--text-primary);
  }

  .ai-think-header-left {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--accent-purple);
  }

  .ai-think-spinner {
    display: inline-flex;
    align-items: center;
  }

  .ai-think-title {
    font-size: var(--text-xs);
  }

  .ai-think-chevron {
    color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    transition: transform var(--transition-fast);
  }

  .ai-think-details[open] .ai-think-chevron {
    transform: rotate(180deg);
  }

  .ai-think-content {
    padding: 8px 12px;
    background: var(--surface-inset);
    border-top: 1px solid var(--border-subtle);
  }

  .ai-think-pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    max-height: 280px;
    overflow-y: auto;
  }

  /* Headings */
  .ai-heading {
    margin: 8px 0 4px 0;
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.3;
  }

  .ai-h1 { font-size: 1.25rem; }
  .ai-h2 { font-size: 1.125rem; }
  .ai-h3 { font-size: 1rem; }
  .ai-h4 { font-size: 0.9rem; }
  .ai-h5 { font-size: 0.85rem; }
  .ai-h6 { font-size: 0.8rem; }

  /* Lists */
  .ai-list {
    margin: 0 0 4px 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .ai-list-item {
    line-height: 1.5;
  }

  /* Blockquote */
  .ai-blockquote {
    margin: 4px 0;
    padding: 6px 12px;
    border-left: 3px solid var(--accent-blue);
    background: var(--surface-selected);
    color: var(--text-secondary);
    font-style: italic;
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }

  /* Horizontal Rule */
  .ai-hr {
    border: none;
    border-top: 1px solid var(--border-subtle);
    margin: 8px 0;
  }

  /* GFM Container Table */
  .ai-table-container {
    max-inline-size: 100%;
    width: 100%;
    max-block-size: min(420px, 65vh);
    overflow: auto;
    margin: 6px 0;
    container-type: inline-size;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
  }

  .ai-table-container:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 2px;
  }

  .ai-table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0;
    font-size: var(--text-xs);
    text-align: left;
  }

  .ai-table thead {
    position: sticky;
    inset-block-start: 0;
    z-index: 1;
  }

  .ai-th {
    padding: 8px 10px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-weight: 600;
    border-bottom: 1px solid var(--border-primary);
  }

  .ai-td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-secondary);
  }

  .ai-tr:last-child .ai-td {
    border-bottom: none;
  }

  .ai-tr:hover .ai-td {
    background: var(--surface-selected);
    color: var(--text-primary);
  }

  @container (width < 340px) {
    .ai-table,
    .ai-table thead,
    .ai-table tbody,
    .ai-tr,
    .ai-th,
    .ai-td {
      display: block;
    }

    .ai-table thead {
      display: none;
    }

    .ai-tr {
      border-bottom: 1px solid var(--border-primary);
      padding: 4px 0;
    }

    .ai-tr:last-child {
      border-bottom: none;
    }

    .ai-td {
      border-bottom: none;
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      gap: 8px;
      text-align: right;
    }

    .ai-td[data-label]:not([data-label=""])::before {
      content: attr(data-label) ": ";
      font-weight: 600;
      color: var(--text-primary);
      text-align: left;
      flex-shrink: 0;
    }
  }

  .text-left { text-align: left; }
  .text-center { text-align: center; }
  .text-right { text-align: right; }

  /* Animations */
  @keyframes ai-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .ai-spin {
    animation: ai-spin 1.2s linear infinite;
  }

  @keyframes ai-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .ai-pulsing {
    animation: ai-pulse 1.8s ease-in-out infinite;
  }
</style>
