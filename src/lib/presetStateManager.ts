import { dockerApi, sysMethods } from "../lib/api";
import { getAppSetting, setAppSetting } from "./settingsStore.svelte";

export const TIER_ORDER: Record<string, number> = {
  minimal: 1,
  development: 2,
  standard: 3,
  power: 4,
  kubernetes: 5
};

export const PRESET_LABELS: Record<string, string> = {
  minimal: "Minimal",
  development: "Dev",
  standard: "Standard",
  power: "Power",
  kubernetes: "K8s",
  custom: "Custom",
};

export const PRESET_COLORS: Record<string, string> = {
  minimal: "#3FB950",
  development: "#58A6FF",
  standard: "#A78BFA",
  power: "#D29922",
  kubernetes: "#A78BFA",
  custom: "#8B949E",
};

export const BUILT_IN_PRESETS = [
  { id: 'minimal', label: 'Minimal', cpus: 2, memory: 2, disk: 20 },
  { id: 'development', label: 'Development', cpus: 4, memory: 8, disk: 60 },
  { id: 'standard', label: 'Standard', cpus: 8, memory: 16, disk: 100 },
  { id: 'power', label: 'Power', cpus: 12, memory: 32, disk: 200 },
  { id: 'kubernetes', label: 'Kubernetes', cpus: 6, memory: 16, disk: 80 }
];

// We store the current preset for each instance in SQLite via settingsStore
export function setCurrentPresetForInstance(profile: string, presetId: string) {
  setAppSetting(`colima_instance_preset_${profile}`, presetId);
}

export function getCurrentPresetForInstance(profile: string): string {
  return getAppSetting(`colima_instance_preset_${profile}`, "custom");
}

export async function handleInstanceStarted(profile: string, newPresetId: string) {
  try {
    const oldPresetId = getCurrentPresetForInstance(profile);
    
    // If the preset hasn't changed, just return (no scale-aware logic needed)
    if (oldPresetId === newPresetId && oldPresetId !== "custom") {
        return;
    }

    console.log(`[PresetState] Switching ${profile} from ${oldPresetId} to ${newPresetId}`);
    
    // Get current running containers (snapshot_current)
    const currentContainers = await dockerApi.listContainers(true);
    
    // Load the target snapshot for the new preset
    const newSnapshot = await sysMethods.loadPresetSnapshot(newPresetId, profile);
    const newSnapshotContainers = newSnapshot ? JSON.parse(newSnapshot.containers_json) : [];

    const oldTier = TIER_ORDER[oldPresetId] || 0;
    const newTier = TIER_ORDER[newPresetId] || 0;

    if (newTier > oldTier && oldTier !== 0) {
      // SCALE UP: Keep current, and restore missing ones from new snapshot
      console.log("[PresetState] Scaling UP: Restoring higher-tier containers...");
      for (const c of newSnapshotContainers) {
        if (c.State === 'running') {
           const current = currentContainers.find((x: any) => x.Names === c.Names);
           if (current && current.State !== 'running') {
              console.log(`[PresetState] Starting ${c.Names}...`);
              await dockerApi.startContainer(current.Id).catch(console.error);
           }
        }
      }
    } else if (newTier < oldTier && newTier !== 0) {
      // SCALE DOWN: Stop containers that belong to the higher tier but NOT in the new snapshot
      console.log("[PresetState] Scaling DOWN: Stopping higher-tier containers...");
      for (const current of currentContainers) {
        if (current.State === 'running') {
          const inNew = newSnapshotContainers.find((x: any) => x.Names === current.Names && x.State === 'running');
          if (!inNew) {
             console.log(`[PresetState] Stopping ${current.Names}...`);
             await dockerApi.stopContainer(current.Id).catch(console.error);
          }
        }
      }
      
      // Also ensure any container from the lower tier snapshot is running
      for (const c of newSnapshotContainers) {
         if (c.State === 'running') {
            const current = currentContainers.find((x: any) => x.Names === c.Names);
            if (current && current.State !== 'running') {
               console.log(`[PresetState] Starting ${c.Names}...`);
               await dockerApi.startContainer(current.Id).catch(console.error);
            }
         }
      }
    }

    // Update the known preset for this instance
    setCurrentPresetForInstance(profile, newPresetId);
    
    // Save the fresh state after adjustments
    await saveCurrentState(newPresetId, profile);

  } catch (err) {
    console.error("[PresetState] Failed to apply scale-aware logic", err);
  }
}

export async function saveCurrentState(presetId: string, profile: string) {
  try {
    const currentContainers = await dockerApi.listContainers(true);
    await sysMethods.savePresetSnapshot(presetId, profile, JSON.stringify(currentContainers), false);
    console.log(`[PresetState] Saved snapshot for ${profile} at preset ${presetId}`);
  } catch (err) {
    console.error("[PresetState] Failed to save snapshot", err);
  }
}

export async function handleInstanceStopping(profile: string) {
  const currentPreset = getCurrentPresetForInstance(profile);
  if (currentPreset) {
     await saveCurrentState(currentPreset, profile);
  }
}

/**
 * Build a mapping of containerName → presetId by scanning all stored snapshots.
 * Each container is attributed to the LOWEST-tier preset that ever recorded it as running.
 * This provides a "this container belongs to Minimal" or "this container was added at Power" indicator.
 */
export async function getContainerPresetMap(instanceProfile: string): Promise<Map<string, string>> {
  const map = new Map<string, string>();
  try {
    const snapshots = await sysMethods.listAllPresetSnapshots(instanceProfile);
    if (!snapshots || !Array.isArray(snapshots)) return map;

    // Sort by tier (lowest first) so lowest-tier attribution wins
    const sorted = [...snapshots].sort((a, b) => {
      return (TIER_ORDER[a.preset_id] || 99) - (TIER_ORDER[b.preset_id] || 99);
    });

    for (const snap of sorted) {
      try {
        const containers = JSON.parse(snap.containers_json);
        for (const c of containers) {
          const name = c.Names || c.names || "";
          if (name && !map.has(name)) {
            map.set(name, snap.preset_id);
          }
        }
      } catch { /* skip malformed JSON */ }
    }
  } catch (err) {
    console.error("[PresetState] Failed to build container preset map", err);
  }
  return map;
}
