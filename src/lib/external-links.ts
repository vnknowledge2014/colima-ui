import { isRunningInTauri } from "./env";

/** Repository the bug reporter files against. */
export const REPO_URL = "https://github.com/vnknowledge2014/colima-ui";

/**
 * A prefilled "new issue" URL.
 *
 * The body carries the signature and a placeholder, not the bundle itself:
 * GitHub rejects a query string past roughly 8 KB, and a diagnostic bundle is
 * routinely larger than that. Losing the body silently would be worse than
 * asking the user to paste — which they have to consent to anyway, and which is
 * the point of showing them the contents first.
 */
export function newIssueUrl(title: string, body: string): string {
  const params = new URLSearchParams({ title, body });
  return `${REPO_URL}/issues/new?${params}`;
}

/**
 * Whether a string is a URL this app is willing to hand to the operating system.
 *
 * `https:` only. In the desktop app an external link leaves the webview entirely
 * and is resolved by the OS handler, which is outside the CSP and outside
 * anything the app can undo — `javascript:`, `file:` and a long tail of
 * registered schemes all mean something there.
 *
 * Parsed with `new URL()` rather than matched with a regex: the parser is the
 * same one that decides what the scheme actually is, so it cannot disagree with
 * whatever opens the link.
 */
export function isSafeExternalUrl(url: string): boolean {
  try {
    return new URL(url).protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * Hosts an announcement may link to.
 *
 * Kept here, next to the other outbound URLs, so extending it is one edit in one
 * file. Deliberately short: announcement content is fetched from the network, so
 * this is the only list standing between a compromised feed and a link the user
 * is invited to click.
 */
const ANNOUNCEMENT_LINK_HOSTS = ["github.com", "www.github.com"];

/**
 * Whether an announcement's `linkUrl` may be offered as a link.
 *
 * A rejected link is simply not rendered — the announcement still shows its text,
 * which is the part that carries the warning.
 */
export function isAllowedAnnouncementLink(url: string | undefined): boolean {
  if (!url || !isSafeExternalUrl(url)) return false;
  try {
    const parsed = new URL(url);
    // Port and credentials are refused rather than ignored. Neither can point at
    // another host, but `https://user:pw@github.com/` hands credentials to the
    // browser and a port says the link is not the ordinary web page it looks
    // like — a real vendor announcement needs neither.
    if (parsed.port || parsed.username || parsed.password) return false;
    return ANNOUNCEMENT_LINK_HOSTS.includes(parsed.hostname.toLowerCase());
  } catch {
    return false;
  }
}

/**
 * Open a URL in the user's default browser.
 *
 * In the desktop app the webview must hand the URL to the OS via the opener
 * plugin; in browser mode there is no plugin, so fall back to window.open.
 *
 * Anything that is not `https:` is refused here rather than at each call site.
 * Every caller today passes a compile-time constant, so the guard changes
 * nothing for them — it is here for the callers that come later, and for the one
 * that already passes a value from the network (announcement links, which are
 * filtered again by host before a link is even drawn).
 */
export async function openExternal(url: string): Promise<void> {
  if (!isSafeExternalUrl(url)) {
    console.warn("Refused to open a non-https URL");
    return;
  }
  if (isRunningInTauri()) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return;
    } catch {
      // Plugin unavailable — fall through to the browser path rather than
      // leaving the click with no visible effect.
    }
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
