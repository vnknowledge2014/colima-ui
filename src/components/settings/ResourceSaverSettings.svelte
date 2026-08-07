<script lang="ts">
  import { sysMethods } from "../../lib/api";
  import { setAppSetting, getAppSetting } from "../../lib/settingsStore.svelte";

  let autoPauseEnabled = $state(getAppSetting("colimaui_auto_pause") === "true");
  let autoPauseMinutes = $state(parseInt(getAppSetting("colimaui_auto_pause_mins") || "15", 10));

  $effect(() => {
    setAppSetting("colimaui_auto_pause", String(autoPauseEnabled));
    setAppSetting("colimaui_auto_pause_mins", String(autoPauseMinutes));
    sysMethods.setResourceSaver(autoPauseEnabled, autoPauseMinutes).catch(() => {});
  });
</script>

<!-- Resource Saver Mode -->
<div class="card" style="margin-bottom: 24px;">
  <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">Resource Saver Mode</h3>
  <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 20px;">
    Automatically pause the Colima instance if there are no active CPU spikes or running containers for a set period.
  </p>
  <div style="display: flex; flex-direction: column; gap: 16px;">
    <div style="display: flex; align-items: center; justify-content: space-between;">
      <label style="font-size: var(--text-sm); font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 8px;">
        <input type="checkbox" class="checkbox" bind:checked={autoPauseEnabled} />
        <span>Enable Auto-Pause</span>
      </label>
    </div>
    
    {#if autoPauseEnabled}
    <div style="display: flex; align-items: center; gap: 12px; background: var(--bg-secondary); padding: 12px; border-radius: 6px;">
      <label for="idleThreshold" style="font-size: var(--text-sm); color: var(--text-secondary);">Idle threshold (minutes)</label>
      <input id="idleThreshold" type="number" min="1" max="1440" bind:value={autoPauseMinutes} 
             style="width: 80px; padding: 4px 8px; border: 1px solid var(--border-primary); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: var(--text-sm);" />
    </div>
    {/if}
  </div>
</div>
