<script lang="ts">
  import { setAppSetting, getAppSetting } from "../../lib/settingsStore.svelte";
  import { isTauri } from "../../lib/api/client";

  // Absent means "not configured yet", and the Rust side defaults to showing
  // the tray — so only an explicit "false" turns it off.
  let showTray = $state(getAppSetting("colimaui_show_tray") !== "false");
  let touched = $state(false);

  $effect(() => {
    // Skip the first run so merely opening Settings does not write the value.
    if (!touched) return;
    setAppSetting("colimaui_show_tray", String(showTray));
  });
</script>

{#if isTauri()}
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">Menu Bar</h3>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 20px;">
      Show instance status in the menu bar and start or stop instances without opening the window.
    </p>
    <label style="font-size: var(--text-sm); font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 8px;">
      <input
        type="checkbox"
        class="checkbox"
        bind:checked={showTray}
        onchange={() => touched = true}
      />
      <span>Show menu bar icon</span>
    </label>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 12px;">
      Takes effect the next time ColimaUI starts.
    </p>
  </div>
{/if}
