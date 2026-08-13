<script lang="ts">
  /**
   * The switch for scheduled rescans — and deliberately **not** inside a
   * `ProGate`.
   *
   * A stop control placed behind the gate of the feature it stops is a trap: a
   * user whose subscription lapses would see a background job running and have
   * no way to reach the switch that ends it. So this renders for everyone. The
   * backend enforces the same asymmetry — turning it on needs entitlement,
   * turning it off never does.
   *
   * `enabled` and `running` are shown separately on purpose. After a lapse the
   * preference is still on while the thread has stopped, and a switch that
   * hides that difference is lying about what the machine is doing.
   */
  import { securityApi, type WatchState } from "../../lib/api";
  import { onEntitlementChange } from "../../lib/pro.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  let watch = $state<WatchState | null>(null);
  let loaded = $state(false);

  async function load() {
    try {
      watch = await securityApi.watchState();
    } catch {
      watch = null;
    }
    loaded = true;
  }

  $effect(() => {
    if (!loaded) void load();
  });

  // Entitlement changing is exactly when `running` stops matching `enabled`.
  $effect(() => onEntitlementChange(() => void load()));

  async function toggle() {
    if (!watch) return;
    try {
      watch = await securityApi.watchSetEnabled(!watch.enabled);
    } catch (e) {
      globalToast("error", String(e));
      // The refusal came from the backend, so re-read rather than assume.
      await load();
    }
  }

  async function setInterval(hours: number) {
    try {
      watch = await securityApi.watchSetInterval(hours);
    } catch (e) {
      globalToast("error", String(e));
    }
  }
</script>

{#if watch}
  <div style="display: flex; align-items: center; gap: 10px; flex-wrap: wrap;">
    <label style="display: flex; align-items: center; gap: 6px; font-size: var(--text-sm);">
      <input type="checkbox" checked={watch.enabled} onchange={toggle} />
      {t("security.watch.label", { default: "Rescan running images on a schedule" })}
    </label>

    {#if watch.enabled}
      <label style="display: flex; align-items: center; gap: 6px; font-size: var(--text-xs); color: var(--text-secondary);">
        {t("security.watch.every", { default: "every" })}
        <input
          class="input"
          style="width: 70px;"
          type="number"
          min="1"
          max="168"
          value={watch.intervalHours}
          onchange={(e) => setInterval(Number((e.currentTarget as HTMLInputElement).value))}
        />
        {t("security.watch.hours", { default: "hours" })}
      </label>
    {/if}
  </div>

  {#if watch.enabled && !watch.running}
    <!-- The honest case: the preference says on, nothing is running. Almost
         always a lapsed subscription. -->
    <p class="hint-text" style="font-size: var(--text-xs); color: var(--accent-yellow);">
      {t("security.watch.not_running", {
        default: "Switched on, but not running — scheduled rescans need an active subscription. The switch above still turns them off.",
      })}
    </p>
  {:else}
    <p class="hint-text" style="font-size: var(--text-xs);">
      {t("security.watch.cost", {
        default: "Off by default: a rescan runs a scanner over every running image and may download a vulnerability database.",
      })}
    </p>
  {/if}
{/if}
