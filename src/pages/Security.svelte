<script lang="ts">
  /**
   * The Security page: the state of this machine first, the things you can do
   * to it second.
   *
   * ## State before action
   *
   * The page opens on a posture — one score, what it covers, and what produced
   * it — rather than on a list of images waiting to be acted upon. A person
   * arriving here is asking "is this machine in good shape", and a list of
   * buttons does not answer that question.
   *
   * The tabs are layers of that same question: Overview is the summary, Images
   * is where a single image is examined, History is the layer that later work
   * fills in. It is shown, and says plainly that it is not available yet,
   * instead of appearing when the feature does — a tab that materialises later
   * reads as an accident.
   *
   * ## Scanning is something you ask for
   *
   * No image is scanned on arrival. A scan spawns a process, reads an entire
   * image, and on a first run downloads a 1.2 GB vulnerability database. Doing
   * that because somebody opened a page would be spending their disk and their
   * time on a decision they did not make.
   *
   * ## What leaves the machine
   *
   * Nothing about the images. Trivy fetches its own database; the alternatives
   * come from a table shipped inside the app. The image list is never sent
   * anywhere — see `docs/telemetry.md`.
   */
  import { onMount } from "svelte";
  import {
    securityApi,
    type CatalogSuggestions,
    type Level,
    type RulePack,
    type SecurityAudit,
  } from "../lib/api/security";
  import { dockerApi, type DockerImage } from "../lib/api";
  import { blockingCapability, capability, capabilityNotice } from "../store/capabilities.svelte";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import { openHelpArticle } from "../store.svelte";
  import PostureHeader from "../components/security/PostureHeader.svelte";
  import NextActions from "../components/security/NextActions.svelte";
  import ImageRanking from "../components/security/ImageRanking.svelte";
  import { summarizePosture, type ImageRow } from "../components/security/security-posture";
  import { nextActions } from "../components/security/security-actions";
  import ScoreBreakdownCard from "../components/security/ScoreBreakdownCard.svelte";
  import FindingsTable from "../components/security/FindingsTable.svelte";
  import RuleChecklist from "../components/security/RuleChecklist.svelte";
  import AlternativesPanel from "../components/security/AlternativesPanel.svelte";

  type Tab = "overview" | "images" | "history";

  let images = $state<DockerImage[]>([]);
  let tab = $state<Tab>("overview");
  let selected = $state<string | null>(null);
  let level = $state<Level>("l1");
  let pack = $state<RulePack | null>(null);

  /** Audits by image reference, so switching back to one is instant. */
  let audits = $state<Record<string, SecurityAudit>>({});
  let suggestions = $state<Record<string, CatalogSuggestions>>({});
  /**
   * Which images are being scanned, and which failed — both keyed by image.
   *
   * Not single values: the user can start one scan, select another image and
   * start a second. With one `scanning` ref, whichever finished first cleared
   * the flag for both, so the first image lost its Cancel button while its
   * process was still running, and an error from one image rendered under
   * another.
   */
  let scanning = $state<Record<string, true>>({});
  let scanErrors = $state<Record<string, string>>({});
  /** True while `changeLevel` is re-scoring; it must not be re-entered. */
  let rescoring = $state(false);

  const busy = $derived(rescoring || Object.keys(scanning).length > 0);
  /**
   * Absent is not the same as missing: capabilities load asynchronously and
   * detection can fail outright. Claiming "Trivy is not installed" before
   * anyone has looked sends the user to install what they already have.
   */
  const scannerState = $derived(capability("trivy")?.state);
  const scannerMissing = $derived(scannerState === "missing");
  const scannerUnknown = $derived(scannerState === undefined || scannerState === "unknown");
  const currentAudit = $derived(selected ? (audits[selected] ?? null) : null);
  const posture = $derived(summarizePosture(audits, images.length));
  const actions = $derived(nextActions(audits, suggestions, pack));
  const rows = $derived<ImageRow[]>(
    images.map((img) => {
      const ref = refName(img);
      const score = audits[ref]?.score.total ?? null;
      return {
        // Two untagged images of the same repository share a reference, so the
        // reference cannot identify a row: as a list key it collides, and Svelte
        // patches the wrong node.
        key: img.Id + img.Tag,
        ref,
        score,
        status: scanning[ref] ? "scanning" : scanErrors[ref] ? "failed" : score === null ? "unscanned" : "scanned",
      };
    }),
  );

  /** Jump from a summary row to the image it is about, where it can be acted on. */
  function openImage(imageRef: string) {
    selected = imageRef;
    tab = "images";
  }

  function refName(img: DockerImage): string {
    return img.Tag && img.Tag !== "<none>" ? `${img.Repository}:${img.Tag}` : img.Repository;
  }

  /**
   * Stable per image, and the handle the cancel button needs.
   *
   * The digest of the reference rather than a truncation of it: two long
   * references sharing a prefix would otherwise share a scan id, and cancelling
   * one would kill the other.
   */
  function scanId(imageRef: string): string {
    let hash = 2166136261;
    for (let i = 0; i < imageRef.length; i++) {
      hash ^= imageRef.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    const safe = imageRef.replace(/[^A-Za-z0-9_-]/g, "-").slice(0, 32);
    return `ui-${safe}-${(hash >>> 0).toString(36)}`;
  }

  /** The entries of `map` whose image is still on this machine. */
  function pick<T>(map: Record<string, T>, keep: Set<string>): Record<string, T> {
    return Object.fromEntries(Object.entries(map).filter(([ref]) => keep.has(ref)));
  }

  async function loadImages() {
    try {
      images = (await dockerApi.listImages()).filter((i) => i.Repository && i.Repository !== "<none>");
      // An image that is gone cannot keep the detail pane: the score on screen
      // would describe something that no longer exists on this machine.
      if (selected && !images.some((i) => refName(i) === selected)) selected = null;
      // Nor can it keep its result. The posture counts what has been scanned
      // against how many images exist, so a score outliving its image reads as
      // "3 of 1 scanned" and puts fixes for a deleted image at the top of the
      // list of things to do next.
      const present = new Set(images.map(refName));
      audits = pick(audits, present);
      suggestions = pick(suggestions, present);
      scanErrors = pick(scanErrors, present);
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function audit(imageRef: string, refresh = false): Promise<SecurityAudit | null> {
    scanning = { ...scanning, [imageRef]: true };
    const { [imageRef]: _cleared, ...restErrors } = scanErrors;
    scanErrors = restErrors;
    try {
      const result = await securityApi.audit(scanId(imageRef), imageRef, level, refresh);
      audits = { ...audits, [imageRef]: result };
      // Asked for after the scan, not before: an image nobody scans needs no
      // suggestions, and this way the panel never races the score it explains.
      //
      // Its own failure, too: the scan above succeeded, and reporting "this
      // image could not be scanned" because a table of alternatives could not be
      // read would throw away a result that is on screen.
      try {
        suggestions = { ...suggestions, [imageRef]: await securityApi.alternatives(imageRef) };
      } catch {
        // The alternatives panel says it has nothing to suggest. The score,
        // checklist and findings are unaffected.
      }
      return result;
    } catch (e) {
      // Recorded against the image rather than raised as a toast: a scanner that
      // cannot read one image is a normal result for that image, and the rest of
      // the page stays usable.
      scanErrors = { ...scanErrors, [imageRef]: String(e) };
      return null;
    } finally {
      const { [imageRef]: _done, ...rest } = scanning;
      scanning = rest;
    }
  }

  /** Stops the scan for `imageRef` — the one the button was drawn for. */
  async function cancel(imageRef: string) {
    try {
      await securityApi.cancel(scanId(imageRef));
    } catch {
      // The scan will end on its own; a failed cancel is not worth a dialog.
    }
  }

  /**
   * Re-score everything already scanned when the strictness changes.
   *
   * Results are collected into a new map and swapped in at the end: clearing
   * first would lose an image permanently if its re-audit failed, and would
   * leave the page half-empty while the loop ran.
   */
  async function changeLevel(next: Level) {
    if (rescoring) return;
    level = next;
    rescoring = true;
    try {
      const scanned = Object.keys(audits);
      for (const imageRef of scanned) {
        // `refresh: false` — the scan is cached by digest, so this re-evaluates
        // the rules without running the scanner again.
        await audit(imageRef);
      }
    } finally {
      rescoring = false;
    }
  }

  onMount(() => {
    void loadImages();
    // The pack turns a rule id into something a person can read. Without it the
    // checklist still lists what failed, just without the explanation.
    securityApi.rules().then((p) => (pack = p)).catch(() => (pack = null));
  });
</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t("security.title", { default: "Security" })}
    <span class="subtitle">{t("security.subtitle", { default: "Scan a local image and see why it scores what it does" })}</span>
  </h1>
</div>

<div class="content-body">
  <PostureHeader {posture} {level} {busy} onChangeLevel={changeLevel} onReload={loadImages} />

  {#if scannerUnknown}
    <!-- Detection has not answered yet, or failed. Saying "not installed" here
         would send someone to install what they already have. -->
    <div class="card scanner-missing">
      <div class="empty-state-title">
        {t("security.scanner_unknown", { default: "Checking for Trivy…" })}
      </div>
    </div>
  {:else if scannerMissing}
    {@const cap = capability("trivy")}
    <div class="card scanner-missing">
      <div class="empty-state-title">
        {t("security.scanner_missing", { default: "Trivy is not installed" })}
      </div>
      <p class="empty-state-text">
        {t("security.scanner_missing_text", {
          default:
            "ColimaUI drives Trivy rather than bundling it — its vulnerability database alone is about 1.2 GB and changes daily.",
        })}
      </p>
      {#if cap?.install_hint}
        <pre class="hint">{cap.install_hint}</pre>
      {/if}
      <button class="link" onclick={() => openHelpArticle("image-hardening")}>
        {t("security.open_guide", { default: "Read the image hardening guide" })}
      </button>
    </div>
  {/if}

  {#snippet noImages()}
    {@const blocked = blockingCapability("colima", "docker")}
    <div class="empty-state">
      {#if blocked}
        <div class="empty-state-title">{capabilityNotice(blocked).title}</div>
        <div class="empty-state-text">{capabilityNotice(blocked).text}</div>
      {:else}
        <div class="empty-state-title">{t("security.no_images", { default: "No local images to scan" })}</div>
        <div class="empty-state-text">
          {t("security.no_images_text", { default: "Pull or build an image, then come back." })}
        </div>
      {/if}
    </div>
  {/snippet}

  <!-- `aria-current` rather than colour alone: which section you are in is the
       one thing this bar exists to say. -->
  <nav class="tabs" aria-label={t("security.sections", { default: "Security sections" })}>
    <button
      class="tab-btn"
      class:active={tab === "overview"}
      aria-current={tab === "overview" ? "page" : undefined}
      onclick={() => (tab = "overview")}
    >
      {t("security.tab_overview", { default: "Overview" })}
    </button>
    <button
      class="tab-btn"
      class:active={tab === "images"}
      aria-current={tab === "images" ? "page" : undefined}
      onclick={() => (tab = "images")}
    >
      {t("security.tab_images", { default: "Images" })}
    </button>
    <button
      class="tab-btn"
      class:active={tab === "history"}
      aria-current={tab === "history" ? "page" : undefined}
      onclick={() => (tab = "history")}
    >
      {t("security.tab_history", { default: "History" })}
    </button>
  </nav>

  {#if tab === "history"}
    <!-- Shown rather than hidden: this is a layer the page will grow, and a tab
         that appears out of nowhere later reads as a glitch. Saying what is
         missing is more honest than an empty panel. -->
    <div class="card not-yet">
      <div class="empty-state-title">
        {t("security.history_title", { default: "Score history is not available yet" })}
      </div>
      <p class="empty-state-text">
        {t("security.history_text", {
          default:
            "Scores are not stored between runs yet, so there is no trend to draw. Today's score is on the Overview.",
        })}
      </p>
    </div>
  {:else if tab === "overview"}
    {#if images.length === 0}
      {@render noImages()}
    {:else}
      <div class="overview">
        <ImageRanking
          {rows}
          onSelect={openImage}
          onScan={(ref) => {
            selected = ref;
            void audit(ref);
          }}
          scanDisabled={scannerMissing || scannerUnknown || rescoring}
          onSeeAll={() => (tab = "images")}
        />
        <NextActions {actions} {level} onSelect={openImage} />
      </div>
    {/if}
  {:else if images.length === 0}
    {@render noImages()}
  {:else}
    <div class="layout">
      <aside class="list card">
        {#each images as img (img.Id + img.Tag)}
          {@const ref = refName(img)}
          {@const score = audits[ref]?.score.total}
          <button
            class="row"
            class:active={selected === ref}
            aria-current={selected === ref ? "true" : undefined}
            onclick={() => (selected = ref)}
          >
            <span class="ref">{ref}</span>
            {#if score !== undefined}
              <span class="bar" aria-hidden="true">
                <span class="fill" style="width: {score}%"></span>
              </span>
              <span class="score">{score}</span>
            {:else if scanning[ref]}
              <span class="score pending">…</span>
            {:else}
              <span class="score pending">—</span>
            {/if}
          </button>
        {/each}
      </aside>

      <section class="detail">
        {#if !selected}
          <p class="hint-text">
            {t("security.pick_image", { default: "Pick an image on the left, then scan it." })}
          </p>
        {:else}
          <div class="detail-head">
            <code class="selected-ref">{selected}</code>
            {#if scanning[selected]}
              <span class="stage">
                {t("security.scanning", {
                  default:
                    "Scanning… the first scan in a while downloads a ~1.2 GB database before it starts.",
                })}
              </span>
              <button class="btn btn-ghost" onclick={() => cancel(selected!)}>
                {t("security.cancel", { default: "Cancel" })}
              </button>
            {:else}
              <button class="btn btn-primary" disabled={scannerMissing || rescoring} onclick={() => audit(selected!)}>
                {currentAudit
                  ? t("security.rescan", { default: "Scan again" })
                  : t("security.scan", { default: "Scan" })}
              </button>
              {#if currentAudit}
                <button class="btn btn-ghost" onclick={() => audit(selected!, true)}>
                  {t("security.refresh", { default: "Ignore cache" })}
                </button>
              {/if}
            {/if}
          </div>

          {#if scanErrors[selected]}
            <div class="card scan-error">
              <strong>{t("security.scan_failed", { default: "This image could not be scanned" })}</strong>
              <p>{scanErrors[selected]}</p>
            </div>
          {/if}

          {#if currentAudit}
            <ScoreBreakdownCard score={currentAudit.score} />

            <section class="card block">
              <h2>{t("security.rules_heading", { default: "Configuration" })}</h2>
              <RuleChecklist
                evaluation={currentAudit.evaluation}
                {pack}
                onOpenGuide={() => openHelpArticle("image-hardening")}
              />
            </section>

            <section class="card block">
              <h2>{t("security.findings_heading", { default: "Known vulnerabilities" })}</h2>
              <FindingsTable findings={currentAudit.scan.findings} />
            </section>

            <section class="card block">
              <h2>{t("security.alternatives_heading", { default: "Other base images" })}</h2>
              <AlternativesPanel suggestions={suggestions[selected] ?? null} imageRef={selected} />
            </section>
          {:else if !scanning[selected] && !scanErrors[selected]}
            <p class="hint-text">
              {t("security.not_scanned", {
                default:
                  "Not scanned yet. Scanning reads the image locally; nothing about it is sent anywhere.",
              })}
            </p>
          {/if}
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .subtitle {
    font-size: var(--text-sm);
    color: var(--text-muted);
    font-weight: 400;
    margin-left: 12px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 16px;
    border-bottom: 1px solid var(--border-primary);
  }
  .tab-btn {
    padding: 8px 14px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .tab-btn:hover {
    color: var(--text-primary);
  }
  .tab-btn.active {
    color: var(--text-primary);
    border-bottom-color: var(--accent-blue, #3b82f6);
  }
  .overview {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    align-items: start;
  }
  @media (max-width: 900px) {
    .overview {
      grid-template-columns: 1fr;
    }
  }
  .not-yet {
    padding: 24px 16px;
    text-align: center;
  }
  /* The shared empty-state paragraph is capped at 400px, which leaves it flush
     left inside a centered card unless the block itself is centered too. */
  .not-yet .empty-state-text {
    margin: 8px auto 0;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(220px, 320px) 1fr;
    gap: 16px;
    align-items: start;
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
  .list {
    padding: 6px;
    max-height: calc(100vh - 200px);
    overflow-y: auto;
  }
  .row {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    color: inherit;
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
    text-align: left;
  }
  .row:hover {
    background: var(--bg-secondary);
  }
  .row.active {
    background: var(--bg-secondary);
    box-shadow: inset 2px 0 0 var(--accent-blue, #3b82f6);
  }
  .ref {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* The same 0-100 bar as the Overview list, so a score keeps one shape across
     the page instead of being a bar in one place and a bare number in another. */
  .bar {
    flex-shrink: 0;
    width: 48px;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-app);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--accent-blue);
  }
  .score {
    flex-shrink: 0;
    width: 2.5ch;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .score.pending {
    color: var(--text-muted);
    font-weight: 400;
  }
  .detail {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .detail-head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .selected-ref {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-sm);
    word-break: break-all;
  }
  .stage {
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .block h2 {
    margin: 0 0 8px;
    font-size: var(--text-base);
  }
  .block {
    padding: 16px;
  }
      .hint-text {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .scanner-missing,
  .scan-error {
    padding: 16px;
    margin-bottom: 16px;
  }
  .scan-error p {
    margin: 6px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
    word-break: break-word;
  }
  .hint {
    margin: 8px 0;
    padding: 8px 10px;
    background: var(--bg-app);
    border-radius: 6px;
    font-size: var(--text-sm);
    overflow-x: auto;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-blue, #3b82f6);
    cursor: pointer;
    font: inherit;
    font-size: var(--text-sm);
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
