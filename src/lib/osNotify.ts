/**
 * Notifications from the operating system, for when the app is not on screen.
 *
 * A long export is exactly the thing you start and then switch away from, which is
 * when the in-app badge is useless — nobody is looking at it.
 *
 * Three deliberate restrictions:
 *
 *  - **Only when the window is unfocused.** Telling someone about a transfer they
 *    are watching finish is noise.
 *  - **Only terminal outcomes.** Progress belongs in the panel.
 *  - **Never the runtime's error text.** `redact` masks the account segment of a
 *    home directory and known secret shapes, but not arbitrary absolute paths, so
 *    `Cannot create /Volumes/backup-2024/img.tar` survives it intact. In the app
 *    that reaches the person who chose that path; an OS notification is copied into
 *    a system-wide centre with its own history and, on macOS, can be mirrored to
 *    other devices. The message here says which transfer and how it ended, nothing
 *    more.
 */

import { isRunningInTauri } from "./env";
import { getAppSetting } from "./settingsStore.svelte";

/** Settings key for the user-facing toggle. Default on. */
export const OS_NOTIFY_SETTING = "notifications_os_enabled";

/**
 * Permission is asked for once per session, and only when there is something to
 * show. Asking at startup is a prompt with no context attached to it, and asking
 * again after a refusal is nagging.
 */
let permissionAsked = false;
let permissionGranted = false;
/**
 * The prompt in flight, shared by every caller that arrives while it is open.
 *
 * Two transfers finishing together used to mean the second one saw
 * `permissionAsked` already true and returned — losing its notification even
 * though the grant it was waiting on succeeded a moment later.
 */
let permissionPrompt: Promise<boolean> | null = null;

export function osNotifyEnabled(): boolean {
  return getAppSetting(OS_NOTIFY_SETTING, "true") !== "false";
}

/**
 * Show an OS notification, if this is the desktop app, the window is in the
 * background, and the user has not turned it off.
 *
 * Never throws: this is a courtesy on top of a transfer that already succeeded or
 * failed on its own terms, and a permissions quirk must not become a second error.
 */
export async function osNotify(title: string, body?: string): Promise<void> {
  if (!isRunningInTauri()) return;
  if (!osNotifyEnabled()) return;
  // `hasFocus` rather than a Tauri window query: this is about whether the user is
  // looking at the page, which is exactly what the document knows.
  if (typeof document !== "undefined" && document.hasFocus()) return;

  try {
    const mod = await import("@tauri-apps/plugin-notification");

    if (!permissionGranted) {
      // Re-asked every time until it is true, so permission granted in system
      // settings while the app runs is picked up. The reverse — a permission
      // revoked after being granted — stays cached until restart; the OS drops
      // those notifications anyway, so nothing is shown that should not be.
      permissionGranted = await mod.isPermissionGranted();
    }
    if (!permissionGranted) {
      if (permissionPrompt) {
        // Someone else is already asking. Wait for their answer rather than
        // opening a second system dialog or giving up.
        permissionGranted = await permissionPrompt;
      } else if (permissionAsked) {
        // Asked and refused earlier this session. Asking again is nagging.
        return;
      } else {
        permissionAsked = true;
        permissionPrompt = mod
          .requestPermission()
          .then((r) => r === "granted")
          .finally(() => {
            permissionPrompt = null;
          });
        permissionGranted = await permissionPrompt;
      }
      if (!permissionGranted) return;
    }

    mod.sendNotification({ title, body });
  } catch {
    // Plugin missing, permission revoked mid-flight, or a platform that has no
    // notification centre at all. The in-app entry is already there.
  }
}

/** Test seam: forget what was asked and granted. */
export function _resetOsNotifyForTest(): void {
  permissionAsked = false;
  permissionGranted = false;
  permissionPrompt = null;
}
