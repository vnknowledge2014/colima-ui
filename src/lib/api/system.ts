import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== System API =====

export interface PlatformInfo {
  os: "macos" | "linux" | "windows";
  arch: string;
  wsl: boolean;
  wsl_available: boolean;
  package_managers: Array<{ name: string; available: boolean; version: string }>;
}

export interface HostSpecs {
  cpu_cores: number;
  memory_gib: number;
  disk_free_gib: number;
  disk_total_gib: number;
  arch: string;
  model: string;
}

export const systemApi = {
  checkSystem: () =>
    call<SystemInfo>("check_system", undefined, "GET", "/api/system/check"),
  getColimaVersion: () =>
    call<string>("get_colima_version", undefined, "GET", "/api/system/version"),
  systemPrune: (all = false) =>
    call<string>("system_prune", { all }, "POST", "/api/system/prune", { all: String(all) }),
  systemDf: () =>
    call<string>("system_df", undefined, "GET", "/api/system/df"),

  // Host hardware detection
  hostSpecs: () =>
    call<HostSpecs>("host_specs", undefined, "GET", "/api/system/host-specs"),

  // Setup Wizard APIs
  getPlatform: () =>
    call<PlatformInfo>("get_platform", undefined, "GET", "/api/system/platform"),
  installDep: (name: "colima" | "docker" | "lima", method = "brew") =>
    call<{ success: boolean; output: string }>(
      "install_dependency", { name, method }, "POST", "/api/system/install", undefined, { name, method }
    ),
  checkHomebrew: () =>
    call<{ installed: boolean; version: string }>(
      "check_homebrew", undefined, "GET", "/api/system/homebrew"
    ),
  configureAutostart: (enable: boolean) =>
    call<string>(
      "configure_autostart", { enable }, "POST", "/api/system/autostart", undefined, { enable }
    ),
  getAutostartStatus: () =>
    call<{ enabled: boolean }>(
      "get_autostart_status", undefined, "GET", "/api/system/autostart"
    ),
  checkTool: (name: string) =>
    call<{ installed: boolean; version: string }>(
      "check_tool", { name }, "GET", "/api/system/check-tool", { name }
    ),
};


// ===== System Methods (convenience facade) =====

export const sysMethods = {
  checkSystem: () =>
    call<SystemInfo>("check_system", undefined, "GET", "/api/system/check"),
  systemDf: () =>
    call<string>("system_df", undefined, "GET", "/api/docker/df"),
  systemPrune: (all = true) =>
    call<string>("system_prune", { all }, "POST", "/api/docker/prune"),
  hostSpecs: () =>
    call<HostSpecs>("host_specs", undefined, "GET", "/api/system/host-specs"),
  checkTool: (name: string) =>
    call<{ installed: boolean; version: string }>(
      "check_tool", { name }, "GET", "/api/system/check-tool", { name }
    ),
  getPlatform: () =>
    call<PlatformInfo>("get_platform", undefined, "GET", "/api/system/platform"),
  checkHomebrew: () =>
    call<{ installed: boolean; version: string }>(
      "check_homebrew", undefined, "GET", "/api/system/homebrew"
    ),
  installDep: (name: "colima" | "docker" | "lima", method = "brew") =>
    call<{ success: boolean; output: string }>(
      "install_dependency", { name, method }, "POST", "/api/system/install", undefined, { name, method }
    ),
  configureAutostart: (enable: boolean) =>
    call<string>(
      "configure_autostart", { enable }, "POST", "/api/system/autostart", undefined, { enable }
    ),
  setResourceSaver: (enabled: boolean, idleMinutes: number) =>
    call<string>(
      "set_resource_saver", { enabled, idle_minutes: idleMinutes }, "POST", "/api/system/resource-saver", undefined, { enabled, idle_minutes: idleMinutes }
    ),
  getRuntimeInfo: () =>
    call<string>("get_runtime_info", undefined, "GET", "/api/system/runtime"),
  // Preset snapshots
  savePresetSnapshot: (presetId: string, profile: string, snapshotJson: string, isAuto = false) =>
    call<string>("save_preset_snapshot", { preset_id: presetId, profile, snapshot_json: snapshotJson, is_auto: isAuto }, "POST", "/api/presets/snapshot", undefined, { preset_id: presetId, profile, snapshot_json: snapshotJson, is_auto: isAuto }),
  loadPresetSnapshot: (presetId: string, profile: string) =>
    call<string>("load_preset_snapshot", { preset_id: presetId, profile }, "GET", "/api/presets/snapshot", { preset_id: presetId, profile }),
  listAllPresetSnapshots: (profile: string) =>
    call<string>("list_all_preset_snapshots", { profile }, "GET", "/api/presets/snapshots", { profile }),
};
