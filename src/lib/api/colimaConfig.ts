import { call } from "./client";

/**
 * Client for colima.yaml editing and the offline Help articles.
 *
 * Mirrors `src-tauri/src/commands/colima_config.rs` and
 * `src-tauri/src/commands/kb_articles.rs`. Both transports are wired: the
 * config page and the Help page are the same components in Tauri and browser
 * mode, so a missing HTTP path would read as a feature that only half works.
 */

/** Dotted paths from `MANAGED_FIELDS` in `colima_config.rs`. */
export type ManagedField =
  | "cpu"
  | "memory"
  | "disk"
  | "runtime"
  | "vmType"
  | "mountType"
  | "network.dns"
  | "network.address"
  | "kubernetes.enabled";

export const MANAGED_FIELDS: ManagedField[] = [
  "cpu",
  "memory",
  "disk",
  "runtime",
  "vmType",
  "mountType",
  "network.dns",
  "network.address",
  "kubernetes.enabled",
];

export const RUNTIMES = ["docker", "containerd", "incus"] as const;
export const VM_TYPES = ["qemu", "vz"] as const;
export const MOUNT_TYPES = ["sshfs", "9p", "virtiofs"] as const;

/** Only fields the user actually changed are sent; absent means "leave alone". */
export interface ConfigChanges {
  cpu?: number;
  memory?: number;
  disk?: number;
  runtime?: string;
  vmType?: string;
  mountType?: string;
  dns?: string[];
  networkAddress?: boolean;
  kubernetes?: boolean;
}

export type IssueSeverity = "error" | "warning";

export interface ValidationIssue {
  field: string;
  severity: IssueSeverity;
  /** Stable key; the UI translates it and falls back to `message`. */
  code: string;
  message: string;
  /**
   * Interpolation values for the translated string — `{cpu}`, `{host}`, etc.
   * Rust sends these so a translation keeps the host-specific numbers that
   * make the message worth reading.
   */
  params: Record<string, string>;
}

export interface FieldChange {
  field: string;
  from: string | null;
  to: string | null;
  requiresRestart: boolean;
}

export interface ConfigSnapshot {
  profile: string;
  /** Managed fields only, keyed by dotted path. */
  values: Record<string, unknown>;
  /** Echo back on apply so a concurrent `colima start` is detected. */
  mtime: number;
}

export interface ApplyResult {
  changes: FieldChange[];
  issues: ValidationIssue[];
  backupPath: string | null;
  mtime: number;
}

export interface ArticleSummary {
  slug: string;
  locale: string;
  title: string;
  platform: string;
  excerpt?: string;
}

export interface Article extends ArticleSummary {
  body: string;
}

export const colimaConfigApi = {
  get: (profile: string) =>
    call<ConfigSnapshot>(
      "get_colima_config",
      { profile },
      "GET",
      "/api/instances/config",
      { profile }
    ),

  /** Diff + validation without writing anything. */
  preview: (profile: string, changes: ConfigChanges) =>
    call<ApplyResult>(
      "preview_colima_config",
      { profile, changes },
      "POST",
      "/api/instances/config/preview",
      undefined,
      { profile, changes }
    ),

  /**
   * Validate and write. Returns without a `backupPath` when the write was
   * refused — check `issues` for severity `error`.
   */
  apply: (profile: string, changes: ConfigChanges, expectedMtime: number) =>
    call<ApplyResult>(
      "apply_colima_config",
      { profile, changes, expectedMtime },
      "POST",
      "/api/instances/config/apply",
      undefined,
      { profile, changes, expectedMtime }
    ),
};

export const helpApi = {
  list: (locale: string) =>
    call<ArticleSummary[]>(
      "kb_list_articles",
      { locale },
      "GET",
      "/api/kb/articles",
      { locale }
    ),

  get: (slug: string, locale: string) =>
    call<Article>(
      "kb_get_article",
      { slug, locale },
      "GET",
      "/api/kb/articles/get",
      { slug, locale }
    ),

  search: (query: string, locale: string) =>
    call<ArticleSummary[]>(
      "kb_search_articles",
      { query, locale },
      "GET",
      "/api/kb/articles/search",
      { q: query, locale }
    ),
};
