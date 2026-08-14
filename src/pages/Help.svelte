<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { helpApi, type Article, type ArticleSummary } from "../lib/api";
  import { renderMarkdown } from "../lib/markdown";
  import { getLanguage, t } from "../lib/i18n.svelte";
  import { uiState } from "../store.svelte";
  import { normalizeError, errorMessage } from "../lib/errors";
  import BundlePreview from "../components/diagnostics/BundlePreview.svelte";

  /**
   * Offline Help. Every article is compiled into the binary and seeded into
   * SQLite at launch, so this page is fully usable with no network — which is
   * the point, since the errors it explains include "the network failed".
   *
   * `uiState.helpArticle` is how the rest of the app deep-links here: an error
   * carrying a `doc_id` sets it and navigates.
   */

  /**
   * Help is where someone lands when the app misbehaved and the article did not
   * solve it, which makes it the right place to offer a bug report.
   */
  let reporting = $state(false);

  let index = $state<ArticleSummary[]>([]);
  let results = $state<ArticleSummary[] | null>(null);
  let current = $state<Article | null>(null);
  let query = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);

  const locale = $derived(getLanguage());
  const list = $derived(results ?? index);

  async function open(slug: string) {
    try {
      current = await helpApi.get(slug, locale);
      error = null;
    } catch (e) {
      error = errorMessage(normalizeError(e));
    }
  }

  async function search() {
    const q = query.trim();
    if (!q) {
      results = null;
      return;
    }
    try {
      results = await helpApi.search(q, locale);
    } catch (e) {
      error = errorMessage(normalizeError(e));
    }
  }

  // The article body arrives via {@html}, so its links can't carry Svelte
  // handlers. Delegate activation for a[data-slug] — the "Related" cross
  // references between articles.
  function delegateArticleLinks(node: HTMLElement) {
    function activate(e: Event) {
      if (e.type === "keydown") {
        const key = (e as KeyboardEvent).key;
        if (key !== "Enter" && key !== " ") return;
      }
      const target = e.target as HTMLElement | null;
      const a = target?.closest?.("a[data-slug]") as HTMLAnchorElement | null;
      if (a?.dataset.slug) {
        e.preventDefault();
        open(a.dataset.slug);
      }
    }
    node.addEventListener("click", activate);
    node.addEventListener("keydown", activate);
    return {
      destroy() {
        node.removeEventListener("click", activate);
        node.removeEventListener("keydown", activate);
      },
    };
  }

  async function load() {
    loading = true;
    try {
      index = await helpApi.list(locale);
      // Deep link wins over the default; otherwise open the first article so
      // the reading pane is never blank.
      const target = uiState.helpArticle ?? index[0]?.slug;
      uiState.helpArticle = null;
      if (target) await open(target);
      error = null;
    } catch (e) {
      error = errorMessage(normalizeError(e));
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Re-fetch when the app language changes: the article set and its bodies are
  // per-locale, so a language switch has to reload both panes.
  //
  // The effect must depend on `locale` alone. It writes `index` and `current`,
  // both of which it would otherwise read as dependencies — and `open()`
  // replaces `current` with a new object every time, so tracking it would spin
  // forever. `untrack` reads the slug without subscribing, and `lastLocale`
  // makes the run a no-op on mount, where `load()` already fetched.
  let lastLocale = "";
  $effect(() => {
    const lang = locale;
    if (loading || lang === lastLocale) return;
    const first = lastLocale === "";
    lastLocale = lang;
    if (first) return;

    const slug = untrack(() => current?.slug);
    helpApi
      .list(lang)
      .then((next) => {
        index = next;
        results = null;
        if (slug) return open(slug);
      })
      .catch(() => {
        /* keep showing the previous locale rather than emptying the page */
      });
  });

  // The handler accepts either modifier, so the table shows whichever one the
  // user's keyboard actually has rather than teaching them the wrong key.
  const mod = navigator.userAgent.includes("Mac") ? "⌘" : "Ctrl+";
  const SHORTCUTS = $derived([
    { keys: `${mod}K`, label: t("help.shortcut_ai", { default: "Toggle the AI panel" }) },
    { keys: `${mod}⇧R`, label: t("help.shortcut_refresh", { default: "Refresh data" }) },
    { keys: `${mod}1…${mod}9`, label: t("help.shortcut_pages", { default: "Jump to Dashboard through Activity" }) },
    { keys: "Esc", label: t("help.shortcut_escape", { default: "Leave the input, then close the panel" }) },
  ]);
</script>

<div class="content-header">
  <div>
    <h1 class="page-title">{t("help.title", { default: "Help" })}</h1>
    <div class="page-subtitle" style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 4px;">
      {t("help.subtitle", { default: "Troubleshooting guides. Available offline." })}
    </div>
  </div>
  <div class="content-header-actions">
    <button class="btn btn-ghost" onclick={() => (reporting = true)}>
      {t("diagnostics.title", { default: "Report a problem" })}
    </button>
  </div>
</div>

{#if reporting}
  <BundlePreview onClose={() => (reporting = false)} />
{/if}

<div class="content-body">
  {#if loading}
    <p class="help-muted">{t("common.loading", { default: "Loading…" })}</p>
  {:else}
    <div class="help-layout">
      <aside class="help-sidebar">
        <input
          class="input help-search"
          type="search"
          placeholder={t("help.search", { default: "Search articles…" })}
          bind:value={query}
          oninput={search}
        />

        {#if list.length === 0}
          <p class="help-muted">{t("help.no_results", { default: "No matching articles." })}</p>
        {:else}
          <ul class="help-index">
            {#each list as article (article.slug)}
              <li>
                <button
                  class="help-index-item {current?.slug === article.slug ? 'active' : ''}"
                  onclick={() => open(article.slug)}
                >
                  <span class="help-index-title">{article.title}</span>
                  {#if article.platform !== "all"}
                    <span class="help-platform">{article.platform}</span>
                  {/if}
                  {#if article.excerpt}
                    <span class="help-excerpt">{article.excerpt}</span>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}

        <!-- Static, and below the article index on purpose: the articles come
             from the backend and can be searched away, but a shortcut nobody
             can find is a shortcut that does not exist. -->
        <section class="help-shortcuts">
          <h2 class="help-shortcuts-title">
            {t("help.shortcuts", { default: "Keyboard shortcuts" })}
          </h2>
          <dl class="help-shortcut-list">
            {#each SHORTCUTS as s (s.keys)}
              <div class="help-shortcut">
                <dt><kbd>{s.keys}</kbd></dt>
                <dd>{s.label}</dd>
              </div>
            {/each}
          </dl>
        </section>
      </aside>

      <article class="help-article card" use:delegateArticleLinks>
        {#if error}
          <p class="help-error">{error}</p>
        {:else if current}
          <!-- Bodies ship with the binary; there is no user-supplied markdown
               reaching this renderer. -->
          {@html renderMarkdown(current.body)}
        {:else}
          <p class="help-muted">{t("help.pick", { default: "Choose an article." })}</p>
        {/if}
      </article>
    </div>
  {/if}
</div>

<style>
  .help-layout {
    display: grid;
    grid-template-columns: minmax(200px, 260px) 1fr;
    gap: 20px;
    align-items: start;
  }
  @media (max-width: 800px) {
    .help-layout {
      grid-template-columns: 1fr;
    }
  }
  .help-search {
    width: 100%;
    margin-bottom: 12px;
  }
  .help-index {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .help-index-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 6px;
    padding: 8px 10px;
    color: var(--text-secondary);
    cursor: pointer;
    font: inherit;
  }
  .help-index-item:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }
  .help-index-item.active {
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-weight: 600;
  }
  .help-index-title {
    font-size: var(--text-sm);
  }
  .help-platform {
    font-size: var(--text-xs);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .help-excerpt {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-weight: 400;
  }
  .help-article {
    padding: 24px;
    line-height: 1.7;
    overflow-x: auto;
  }
  .help-muted {
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .help-error {
    font-size: var(--text-sm);
    color: var(--accent-red);
  }
  .help-shortcuts {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--border-color);
  }
  .help-shortcuts-title {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin: 0 0 10px;
  }
  .help-shortcut-list {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .help-shortcut {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .help-shortcut dt {
    flex-shrink: 0;
  }
  .help-shortcut dd {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .help-shortcut kbd {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    background: var(--bg-card-hover);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 2px 6px;
    white-space: nowrap;
  }
</style>
