import { call } from "./client";
import type { SystemInfo } from "./types";

// ===== System API =====

export interface PlatformInfo {
  os: "macos" | "linux" | "windows";
  arch: string;
  wsl: boolean;
  wsl_available: boolean;
  package_managers: Array<{ name: string; available: boolean; version: string }>;
}

/** Mirrors `CapabilityState` in `src-tauri/src/commands/system_capabilities.rs`. */
export type CapabilityState =
  | "missing"
  | "installed_not_running"
  | "running"
  | "unknown";

/** Mirrors `Capability` in `src-tauri/src/commands/system_capabilities.rs`. */
export interface Capability {
  id: string;
  name: string;
  state: CapabilityState;
  version?: string;
  install_hint?: string;
  doc_id?: string;
}

export interface HostSpecs {
  cpu_cores: number;
  memory_gib: number;
  disk_free_gib: number;
  disk_total_gib: number;
  arch: string;
  model: string;
}

/**
 * Mirrors `EngineResources` in `src-tauri/src/commands/engine_resources.rs`.
 * `available: false` means the engine was unreachable — callers fall back to
 * the VM config numbers instead of rendering zeros.
 */
export interface EngineResources {
  available: boolean;
  engineName: string;
  serverVersion: string;
  operatingSystem: string;
  cpuCores: number;
  cpuPercent: number;
  memoryTotalBytes: number;
  memoryUsedBytes: number;
  diskUsedBytes: number;
  diskReclaimableBytes: number;
  containersRunning: number;
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

  /**
   * Live CPU / RAM / disk of the active container engine. Works with any engine
   * (Colima, Docker Desktop, OrbStack, Rancher), unlike the VM figures read from
   * colima.yaml which only exist for Colima-managed profiles.
   */
  engineResources: () =>
    call<EngineResources>(
      "engine_resources",
      undefined,
      "GET",
      "/api/system/engine-resources",
    ),

  /**
   * One source of truth for which host tools are installed and usable.
   * Note the path: `/api/system/capabilities`, not `/api/capabilities` — the
   * latter is the static API schema published for AI agents.
   */
  getCapabilities: () =>
    call<Capability[]>(
      "get_system_capabilities",
      undefined,
      "GET",
      "/api/system/capabilities",
    ),

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
      // The Rust parameter is `threshold`; the HTTP body keeps its own name.
      "set_resource_saver", { enabled, threshold: idleMinutes }, "POST", "/api/system/resource-saver", undefined, { enabled, idle_minutes: idleMinutes }
    ),
  getRuntimeInfo: () =>
    call<string>("get_runtime_info", undefined, "GET", "/api/system/runtime"),
  // Preset snapshots
  // Argument names follow the Rust parameters, which are also the column names
  // the row lands in — `isAuto` named a field that does not exist.
  savePresetSnapshot: (presetId: string, profile: string, snapshotJson: string, isManualOverride = false) =>
    call<string>("save_preset_snapshot", { presetId, instanceProfile: profile, containersJson: snapshotJson, isManualOverride }, "POST", "/api/presets/snapshot", undefined, { preset_id: presetId, instance_profile: profile, containers_json: snapshotJson, is_manual_override: isManualOverride }),
  loadPresetSnapshot: (presetId: string, profile: string) =>
    call<{ containers_json: string }>("load_preset_snapshot", { presetId, instanceProfile: profile }, "GET", "/api/presets/snapshot", { preset_id: presetId, instance_profile: profile }),
  listAllPresetSnapshots: (profile: string) =>
    call<string>("list_all_preset_snapshots", { instanceProfile: profile }, "GET", "/api/presets/snapshots", { profile }),
};
