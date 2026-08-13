import { call } from "./client";

// ===== Diagnostics =====
//
// Building a bundle sends nothing anywhere. The response comes back to this
// process and the user decides what happens next — copy, save, or nothing.

export interface DiagnosticSection {
  id: string;
  title: string;
  /** Already redacted by the backend, at construction. */
  content: string;
  /** Log sections start unchecked: largest, and likeliest to hold something private. */
  includedByDefault: boolean;
}

export interface DiagnosticBundle {
  sections: DiagnosticSection[];
  /** Stable across machines for the same error, so duplicates can be grouped. */
  signature: string;
  appVersion: string;
  /** Non-zero means older log lines were dropped to stay under the size cap. */
  truncatedBytes: number;
}

export const diagnosticsApi = {
  /**
   * Collect a bundle.
   *
   * `error` only feeds the signature. `containerId` adds that container's logs
   * as an opt-in section.
   */
  bundle: (error?: string, containerId?: string, logLines?: number) =>
    call<DiagnosticBundle>(
      "diagnostic_bundle",
      { error, containerId, logLines },
      "POST",
      "/api/diagnostics/bundle",
      undefined,
      { error, containerId, logLines }
    ),

  /**
   * Write the checked sections to a Markdown file.
   *
   * `destDir` and `fileName` stay separate so the write is confined to the folder
   * the user chose — the same rule the transfer commands follow.
   */
  save: (
    bundle: DiagnosticBundle,
    include: string[],
    destDir: string,
    fileName: string,
    overwrite = false
  ) =>
    call<string>(
      "save_diagnostic_bundle",
      { bundle, include, destDir, fileName, overwrite },
      "POST",
      "/api/diagnostics/save",
      undefined,
      { bundle, include, destDir, fileName, overwrite }
    ),
};

/**
 * Render the checked sections the same way the backend does.
 *
 * Duplicated deliberately, in one direction only: this is what "Copy" puts on
 * the clipboard, and routing a clipboard action through IPC to get a string the
 * page already holds would be slower and could fail. The file on disk is always
 * rendered by the backend, which redacts again on the way out.
 */
export function renderBundleMarkdown(bundle: DiagnosticBundle, include: string[]): string {
  const parts: string[] = [`## ColimaUI diagnostics (${bundle.appVersion})`, ""];
  if (bundle.signature) parts.push(`**Signature:** \`${bundle.signature}\``, "");
  if (bundle.truncatedBytes > 0) {
    parts.push(`> ${bundle.truncatedBytes} bytes of older log lines were trimmed.`, "");
  }
  for (const section of bundle.sections) {
    if (!include.includes(section.id)) continue;
    parts.push(`### ${section.title}`, "", "```", section.content, "```", "");
  }
  return parts.join("\n");
}
