<script lang="ts">
  /**
   * "Then what should I use instead?" — the question a low score provokes.
   *
   * Answered from a table shipped with the app. Each row says what the swap
   * *changes*, never what to do: whether Alpine suits a service depends on
   * whether its dependencies build against musl, and this table cannot know
   * that. Advice that pretends otherwise is worse than none.
   *
   * The catalog's own date is shown. Suggestions nobody has revised for a year
   * should look their age rather than look current.
   */
  import type { CatalogSuggestions } from "../../lib/api/security";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    suggestions: CatalogSuggestions | null;
    imageRef: string;
  }

  let { suggestions, imageRef }: Props = $props();

  async function copy(image: string) {
    try {
      await navigator.clipboard.writeText(image);
    } catch {
      // A clipboard the browser refuses is not worth an error dialog; the text
      // is on screen and selectable either way.
    }
  }
</script>

<div class="alternatives">
  {#if !suggestions}
    <p class="none">{t("security.alternatives_loading", { default: "Looking…" })}</p>
  {:else if suggestions.alternatives.length === 0}
    <p class="none">
      {t("security.alternatives_none", {
        default:
          "Nothing in the catalog about {image}. That is not a verdict on it — the catalog only covers widely used base images.",
        image: imageRef,
      })}
    </p>
  {:else}
    <ul>
      {#each suggestions.alternatives as alt (alt.image)}
        <li>
          <div class="row">
            <code>{alt.image}</code>
            <button class="link" onclick={() => copy(alt.image)}>
              {t("security.copy", { default: "Copy" })}
            </button>
          </div>
          <p class="why">{alt.why}</p>
        </li>
      {/each}
    </ul>
  {/if}

  {#if suggestions}
    <p class="stamp">
      {t("security.catalog_stamp", {
        default: "Catalog {version}, last revised {date}",
        version: suggestions.catalogVersion,
        date: suggestions.updatedAt,
      })}
    </p>
  {/if}
</div>

<style>
  .alternatives {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-sm);
    word-break: break-all;
  }
  .why {
    margin: 4px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .none {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .stamp {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-blue, #3b82f6);
    cursor: pointer;
    font: inherit;
    font-size: var(--text-xs);
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
