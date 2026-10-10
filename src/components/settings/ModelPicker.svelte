<script lang="ts">
  import { tick } from "svelte";
  import Icon from "../Icon.svelte";

  /**
   * Model field: a text box that is also a filtered list.
   *
   * Replaces `<input list>` + `<datalist>`, whose dropdown is drawn by the
   * browser and takes no styling — it arrived as an OS-coloured list in the
   * middle of a dark panel. A proxy can also serve hundreds of models, which a
   * datalist gives no way to narrow.
   *
   * Typing stays free-form: a model the listing endpoint does not report can
   * still be entered by hand.
   */
  let {
    value = $bindable(),
    models = [],
    fetching = false,
    onRefresh,
  }: {
    value: string;
    models: string[];
    fetching?: boolean;
    onRefresh: () => void;
  } = $props();

  /** Kept in step with `max-height` on `.model-picker-list`. */
  const LIST_MAX_HEIGHT = 240;

  let open = $state(false);
  let dropUp = $state(false);
  let query = $state("");
  let highlighted = $state(0);
  let inputEl = $state<HTMLInputElement>();
  let listEl = $state<HTMLUListElement>();

  /** While the list is open the field shows what is being typed to filter. */
  const filtered = $derived(
    query.trim()
      ? models.filter((m) => m.toLowerCase().includes(query.trim().toLowerCase()))
      : models,
  );

  function openList() {
    if (open) return;
    open = true;
    query = "";
    highlighted = Math.max(0, models.indexOf(value));
    chooseDirection();
  }

  /**
   * Open upward when the space below cannot hold the list.
   *
   * The settings body scrolls, so a picker near its bottom would otherwise have
   * its options cut off by the scroll container — the same way the card used to
   * cut them off.
   */
  function chooseDirection() {
    const rect = inputEl?.getBoundingClientRect();
    if (!rect) return;
    const needed = LIST_MAX_HEIGHT + 4;
    const below = window.innerHeight - rect.bottom;
    dropUp = below < needed && rect.top > below;
  }

  function closeList() {
    open = false;
    query = "";
  }

  function choose(model: string) {
    value = model;
    closeList();
    // Safe to return the caret to the field: opening is bound to click and to
    // the keyboard, not to focus, so this cannot reopen what was just closed.
    inputEl?.focus();
  }

  function onInput(event: Event) {
    const text = (event.currentTarget as HTMLInputElement).value;
    // The typed text is both the value and the filter: committing as we go means
    // a half-typed name is still saved if the user clicks away.
    value = text;
    query = text;
    // Not `openList()`: that clears the query, which is the text just typed.
    if (!open) {
      open = true;
      chooseDirection();
    }
    highlighted = 0;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        openList();
        return;
      }
      const step = event.key === "ArrowDown" ? 1 : -1;
      const count = filtered.length;
      if (count === 0) return;
      highlighted = (highlighted + step + count) % count;
      scrollHighlightedIntoView();
      return;
    }

    if (event.key === "Enter" && open && filtered[highlighted]) {
      event.preventDefault();
      choose(filtered[highlighted]);
      return;
    }

    if (event.key === "Escape" && open) {
      event.preventDefault();
      // Escape closes the list without discarding what was typed.
      closeList();
    }
  }

  async function scrollHighlightedIntoView() {
    // `tick`, not a microtask: the row carrying the attribute only exists once
    // Svelte has flushed the state change.
    await tick();
    listEl?.querySelector<HTMLElement>("[data-highlighted='true']")?.scrollIntoView({
      block: "nearest",
    });
  }
</script>

<svelte:window onclick={(e) => {
  // Anything outside the field dismisses the list, including a click on another
  // settings card — an orphaned dropdown floating over the page reads as a bug.
  if (open && !(e.target as HTMLElement)?.closest?.(".model-picker")) closeList();
}} />

<div class="model-picker">
  <div class="model-picker-field">
    <input
      bind:this={inputEl}
      id="aiModel"
      type="text"
      class="input"
      placeholder="Type or select…"
      autocomplete="off"
      role="combobox"
      aria-expanded={open}
      aria-controls="model-picker-list"
      value={value}
      oninput={onInput}
      onclick={openList}
      onkeydown={onKeydown}
    />
    <button
      type="button"
      class="model-picker-toggle"
      aria-label={open ? "Hide models" : "Show models"}
      onclick={() => (open ? closeList() : (inputEl?.focus(), openList()))}
    >
      <!-- The icon set has no chevron; the group headers in Containers use this
           same glyph for the same purpose. -->
      <span class="model-picker-caret" class:open>▾</span>
    </button>

    {#if open}
      <ul class="model-picker-list" class:up={dropUp} id="model-picker-list" role="listbox" bind:this={listEl}>
        {#if fetching}
          <li class="model-picker-empty"><span class="spinner"></span> Fetching models…</li>
        {:else if models.length === 0}
          <li class="model-picker-empty">No models loaded — use Refresh</li>
        {:else if filtered.length === 0}
          <li class="model-picker-empty">Nothing matches “{query}”</li>
        {:else}
          {#each filtered as model, i (model)}
            <li>
              <button
                type="button"
                class="model-picker-option"
                class:selected={model === value}
                data-highlighted={i === highlighted}
                role="option"
                aria-selected={model === value}
                onmouseenter={() => (highlighted = i)}
                onclick={() => choose(model)}
              >
                <span class="model-picker-name">{model}</span>
                {#if model === value}
                  <Icon name="Check" size={12} color="var(--accent-green)" />
                {/if}
              </button>
            </li>
          {/each}
        {/if}
      </ul>
    {/if}
  </div>

  <button class="btn btn-ghost" onclick={onRefresh} disabled={fetching}>
    {#if fetching}
      <span class="spinner"></span> Fetching…
    {:else}
      <Icon name="Refresh" size={14} /> Refresh
    {/if}
  </button>
</div>

<style>
  .model-picker {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .model-picker-field {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  .model-picker-field .input {
    width: 100%;
    /* Room for the chevron sitting inside the field. */
    padding-right: 30px;
  }

  .model-picker-toggle {
    position: absolute;
    top: 50%;
    right: 6px;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    padding: 2px;
    color: var(--text-muted);
    background: none;
    border: none;
    cursor: pointer;
  }

  .model-picker-toggle:hover {
    color: var(--text-primary);
  }

  .model-picker-caret {
    display: inline-block;
    font-size: 11px;
    line-height: 1;
    transition: transform var(--transition-fast);
  }

  .model-picker-caret.open {
    transform: rotate(180deg);
  }

  .model-picker-list {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    /* Mirrors LIST_MAX_HEIGHT, which decides which way the list opens. */
    max-height: 240px;
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
  }

  .model-picker-list.up {
    top: auto;
    bottom: calc(100% + 4px);
  }

  .model-picker-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    font-size: var(--text-sm);
    font-family: var(--font-mono);
    color: var(--text-primary);
    text-align: left;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  /* Driven by the keyboard cursor as well as the pointer, so arrow keys and the
     mouse cannot disagree about which row is active. */
  .model-picker-option[data-highlighted="true"] {
    background: var(--bg-card-hover);
  }

  .model-picker-option.selected {
    color: var(--accent-green);
  }

  .model-picker-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-picker-empty {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .model-picker-empty .spinner,
  .btn .spinner {
    width: 12px;
    height: 12px;
    border-width: 1.5px;
  }
</style>
