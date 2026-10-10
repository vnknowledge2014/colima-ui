<script lang="ts">
  /**
   * One dialog for all four transfers: export images, import an archive, and copy
   * a file into or out of a container.
   *
   * They share a shape — pick paths, start — so four near-identical dialogs would
   * be four places to fix the same bug.
   *
   * This collects input and starts the transfer; it does not follow it. Progress
   * and cancelling live in the notification list, because a transfer outlives the
   * dialog that started it and blocking the page behind a modal for the length of a
   * multi-gigabyte export was the problem this replaced.
   *
   * Native file pickers are used when running in the desktop shell. In browser
   * mode there is no picker, so the paths are typed; the fields stay usable rather
   * than the feature being desktop-only.
   */
  import { transferApi, type TransferMode } from "../../lib/api/transfer";
  import { startJob } from "../../store/notifications.svelte";
  import { isRunningInTauri } from "../../lib/env";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    mode: TransferMode;
    /** Image references, for export. */
    images?: string[];
    /** Container id, for either copy direction. */
    containerId?: string;
    containerLabel?: string;
    onClose: () => void;
  }

  let { mode, images = [], containerId = "", containerLabel = "", onClose }: Props = $props();

  const isTauri = isRunningInTauri();

  let destDir = $state("");
  // A dialog instance is created per open and never switches mode, so capturing
  // the initial value here is correct rather than merely convenient.
  // svelte-ignore state_referenced_locally
  let fileName = $state(mode === "export" ? "images.tar" : "");
  let tarPath = $state("");
  let hostPath = $state("");
  let containerPath = $state("");
  let overwrite = $state(false);

  let starting = $state(false);
  /**
   * A validation failure from the *start* call.
   *
   * Only synchronous rejections land here — "that name is not a .tar", "the file
   * already exists", "not enough space". Those are things to correct in the fields
   * that are still on screen, so the dialog stays open for them. Anything that goes
   * wrong once the transfer is running is reported through the notification list,
   * because by then this dialog is gone.
   */
  let failure = $state("");

  const title = $derived(
    {
      export: t("transfer.export_title", { default: "Export images" }),
      import: t("transfer.import_title", { default: "Import archive" }),
      "copy-in": t("transfer.copy_in_title", { default: "Copy file into container" }),
      "copy-out": t("transfer.copy_out_title", { default: "Copy file from container" }),
    }[mode]
  );

  /**
   * Last path segment, so a notification names the file without carrying the
   * folder it came from.
   */
  function baseName(path: string): string {
    return path.split("/").filter(Boolean).pop() ?? path;
  }

  /**
   * What the notification list shows for this transfer.
   *
   * Names only — an image reference, a path inside a container, a file name. Host
   * paths stay out: the notification store documents its entries as carrying none,
   * and `formatEntryForClipboard` tells the user they are safe to paste into an
   * issue on that basis.
   */
  const jobLabel = $derived(
    {
      export: images.join(", "),
      import: baseName(tarPath),
      "copy-in": containerPath,
      "copy-out": containerPath,
    }[mode]
  );

  /**
   * The only archive shape these operations produce or accept.
   *
   * `docker save` and `docker cp <src> -` both write an *uncompressed* TAR, so a
   * `.tar.gz` name would misdescribe the file; the backend refuses those outright.
   */
  const TAR_FILTER = { name: "TAR archive", extensions: ["tar"] };

  /**
   * Append `.tar` when the user left it off.
   *
   * The backend requires the extension, and failing after the save dialog has
   * already closed would make the user reopen it and retype the name. Compressed
   * suffixes are left alone so the backend's clearer message reaches the user
   * instead of a name silently turned into something else.
   */
  function withTarSuffix(name: string): string {
    const trimmed = name.trim();
    if (!trimmed) return trimmed;
    const lower = trimmed.toLowerCase();
    if (lower.endsWith(".tar")) return trimmed;
    if (/\.(tgz|gz|zip|bz2|xz)$/.test(lower)) return trimmed;
    return `${trimmed}.tar`;
  }

  /** Native picker for a folder or a file, when the shell provides one. */
  async function pick(kind: "folder" | "openFile" | "saveFile") {
    if (!isTauri) return;
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      if (kind === "folder") {
        const chosen = await dialog.open({ directory: true, multiple: false });
        if (typeof chosen === "string") destDir = chosen;
      } else if (kind === "openFile") {
        const chosen = await dialog.open({
          directory: false,
          multiple: false,
          // Import reads an uncompressed archive; anything else is rejected by the
          // backend anyway, so the picker should not offer it in the first place.
          filters: mode === "import" ? [TAR_FILTER] : undefined,
        });
        if (typeof chosen === "string") {
          if (mode === "import") tarPath = chosen;
          else hostPath = chosen;
        }
      } else {
        const chosen = await dialog.save({
          defaultPath: fileName || "images.tar",
          filters: [TAR_FILTER],
        });
        if (typeof chosen === "string") {
          // Split the chosen path: the backend confines the write to the folder,
          // which only works if the folder arrives separately from the name.
          const cut = chosen.lastIndexOf("/");
          destDir = cut > 0 ? chosen.slice(0, cut) : "/";
          fileName = withTarSuffix(chosen.slice(cut + 1));
        }
      }
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function start() {
    starting = true;
    failure = "";
    try {
      let started;
      if (mode === "export") {
        // Normalise here too: the name can be typed rather than picked, and in
        // browser mode there is no picker at all.
        fileName = withTarSuffix(fileName);
        started = await transferApi.saveImages(images, destDir, fileName, overwrite);
      } else if (mode === "import") {
        started = await transferApi.loadImages(tarPath);
      } else if (mode === "copy-in") {
        started = await transferApi.copyToContainer(containerId, hostPath, containerPath);
      } else {
        fileName = withTarSuffix(fileName);
        started = await transferApi.copyFromContainer(
          containerId,
          containerPath,
          destDir,
          fileName,
          overwrite
        );
      }
      // The transfer now belongs to the notification list, which survives this
      // dialog and the page it was opened from.
      startJob({
        jobId: started.jobId,
        title: jobLabel,
        // The file name, not the folder: the user chose where it goes and does not
        // need telling, and the folder is the part that identifies their machine.
        detail: fileName || undefined,
        totalEstimate: started.totalEstimate,
      });
      onClose();
    } catch (e) {
      // A rejected start means nothing was spawned — path validation, a missing
      // file, not enough disk. Those are corrected in the fields still on screen,
      // so this stays put rather than closing and making the user start over.
      failure = String(e);
    } finally {
      starting = false;
    }
  }

  const canStart = $derived.by(() => {
    if (starting) return false;
    if (mode === "export") return images.length > 0 && !!destDir && !!fileName;
    if (mode === "import") return !!tarPath;
    if (mode === "copy-in") return !!hostPath && !!containerPath;
    return !!containerPath && !!destDir && !!fileName;
  });
</script>

<div class="modal-overlay" role="presentation" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <span class="modal-title">{title}</span>
    </div>

    <div class="body">
      {#if mode === "export"}
        <p class="context">
          {images.length}
          {t("transfer.images_selected", { default: "image(s) selected" })}
        </p>
      {:else if mode !== "import"}
        <p class="context">{containerLabel || containerId}</p>
      {/if}

      {#if mode === "import"}
        <label>
          <span>{t("transfer.archive", { default: "Archive (.tar)" })}</span>
          <div class="row">
            <input bind:value={tarPath} placeholder="/path/to/images.tar" />
            {#if isTauri}
              <button class="btn btn-ghost" onclick={() => pick("openFile")}>
                {t("transfer.browse", { default: "Browse…" })}
              </button>
            {/if}
          </div>
        </label>
      {/if}

      {#if mode === "copy-in"}
        <label>
          <span>{t("transfer.host_file", { default: "File on this machine" })}</span>
          <div class="row">
            <input bind:value={hostPath} placeholder="/path/to/file" />
            {#if isTauri}
              <button class="btn btn-ghost" onclick={() => pick("openFile")}>
                {t("transfer.browse", { default: "Browse…" })}
              </button>
            {/if}
          </div>
        </label>
      {/if}

      {#if mode === "copy-in" || mode === "copy-out"}
        <label>
          <span>{t("transfer.container_path", { default: "Path inside the container" })}</span>
          <input bind:value={containerPath} placeholder="/app/data.json" />
        </label>
      {/if}

      {#if mode === "copy-out"}
        <!-- Copying out always yields an archive, including for a single file.
             Saying so here is the difference between an expected extra step and a
             result that looks wrong. -->
        <p class="hint">
          {t("transfer.copy_out_is_archive", {
            default: "Saved as a .tar archive — a folder and a single file both arrive that way. Extract it with: tar -xf <file>",
          })}
        </p>
      {/if}

      {#if mode === "export" || mode === "copy-out"}
        <label>
          <span>{t("transfer.dest_folder", { default: "Destination folder" })}</span>
          <div class="row">
            <input bind:value={destDir} placeholder="/Users/you/Downloads" />
            {#if isTauri}
              <button
                class="btn btn-ghost"
                onclick={() => pick(mode === "export" ? "saveFile" : "folder")}
               
              >
                {t("transfer.browse", { default: "Browse…" })}
              </button>
            {/if}
          </div>
        </label>
        <label>
          <span>{t("transfer.file_name", { default: "File name" })}</span>
          <input bind:value={fileName} placeholder="images.tar" />
        </label>
        <label class="inline">
          <input type="checkbox" bind:checked={overwrite} />
          <span>{t("transfer.overwrite", { default: "Replace the file if it already exists" })}</span>
        </label>
      {/if}

      {#if failure}
        <p class="failure">{failure}</p>
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn btn-ghost" onclick={onClose}>
        {t("transfer.dismiss", { default: "Cancel" })}
      </button>
      <!-- Named for what it does: the dialog closes and the transfer carries on in
           the notification list, where it keeps its progress and its cancel. -->
      <button class="btn btn-primary" onclick={start} disabled={!canStart}>
        {starting
          ? t("transfer.starting", { default: "Starting…" })
          : t("transfer.start_background", { default: "Start in background" })}
      </button>
    </div>
  </div>
</div>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    min-width: 380px;
  }

  .context {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
    word-break: break-all;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  label.inline {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  input:not([type]) {
    flex: 1;
    min-width: 0;
    background: var(--bg-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: var(--text-xs);
    padding: 6px 8px;
  }

  .hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
  }

  .failure {
    font-size: var(--text-xs);
    color: var(--color-danger, #ef4444);
    margin: 0;
    word-break: break-word;
  }





</style>
