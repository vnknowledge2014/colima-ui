import { call } from "./client";

// ===== Lima API =====

export interface LimaInstance {
  name: string;
  status: string;
  arch: string;
  cpus: string;
  memory: string;
  disk: string;
  dir: string;
}

export const limaApi = {
  list: async (): Promise<LimaInstance[]> => {
    const raw = await call<unknown>("lima_list", undefined, "GET", "/api/lima");
    if (!raw) return [];
    const toInstance = (v: Record<string, unknown>): LimaInstance => ({
      name: String(v.name || ""),
      status: String(v.status || "Unknown"),
      arch: String(v.arch || ""),
      cpus: String(v.cpus || 0),
      memory: v.memory ? (typeof v.memory === 'number' ? formatLimaBytes(v.memory) : String(v.memory)) : "0",
      disk: v.disk ? (typeof v.disk === 'number' ? formatLimaBytes(v.disk) : String(v.disk)) : "0",
      dir: String(v.dir || ""),
    });
    // Tauri IPC may return parsed array directly
    if (Array.isArray(raw)) return raw.map((item) => toInstance(item as Record<string, unknown>));
    if (typeof raw !== 'string') return [];
    if (!raw.trim()) return [];
    try {
      return raw.split("\n").filter((l: string) => l.trim()).map((l: string) => {
        const v = JSON.parse(l) as Record<string, unknown>;
        return toInstance(v);
      });
    } catch { return []; }
  },
  start: (name: string) =>
    call<string>("lima_start", { name }, "POST", "/api/lima/start", undefined, { name }),
  stop: (name: string) =>
    call<string>("lima_stop", { name }, "POST", "/api/lima/stop", undefined, { name }),
  delete: (name: string, force = false) =>
    call<string>("lima_delete", { name, force }, "POST", "/api/lima/delete", undefined, { name, force }),
  info: () =>
    call<string>("lima_info", { name: "" }, "GET", "/api/lima/info"),
  shell: (name: string, command: string) =>
    call<string>("lima_shell", { name, command }, "POST", "/api/lima/shell", undefined, { name, command }),
  templates: () =>
    call<string>("lima_templates", undefined, "GET", "/api/lima/templates"),
  // Every field is required over IPC — `lima_create` takes plain `u32`/`String`,
  // and an omitted (or `undefined`) key is dropped during serialization, which
  // Tauri then rejects as a missing argument. Fill the gaps here so callers can
  // keep passing a partial config. An empty template means "no template", which
  // is what the Rust side already checks for.
  create: (config: { name: string; cpus?: number; memory?: number; disk?: number; template?: string }) => {
    const args = {
      name: config.name,
      cpus: config.cpus ?? 2,
      memory: config.memory ?? 2,
      disk: config.disk ?? 60,
      template: config.template ?? "",
    };
    return call<string>("lima_create", args, "POST", "/api/lima/create", undefined, args);
  },
};

function formatLimaBytes(bytes: number): string {
  if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
  if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
  return `${bytes} B`;
}
