/**
 * Vendor announcements: releases, security advisories, maintenance notices.
 *
 * A one-way, read-only channel. The backend fetches one static JSON file
 * (`src-tauri/src/commands/announcements.rs`); everything here decides which of
 * its entries this install should see, and hands them to the notification store.
 *
 * Two rules shape the rest of this file:
 *
 * **Failure is silent.** An unreachable feed is not the user's doing and not
 * something they can fix, so it produces no toast and touches nothing already on
 * screen. That is also why the backend reports failure as an error rather than as
 * an empty feed — the two must not look alike here.
 *
 * **A security advisory outranks a filter.** Every filter below can go wrong in
 * the direction of hiding something: an audience this build has no name for, a
 * version string that will not parse. Where that can happen, `critical` is let
 * through.
 * One release note too many is a nuisance; one advisory too few is the failure
 * this channel exists to prevent.
 */

import { announcementsApi, type Announcement, type AnnouncementFeed } from "./api/announcements";
import { getAppSetting, setAppSetting } from "./settingsStore.svelte";
import { getLanguage } from "./i18n.svelte";
import { pushNotification } from "../store/notifications.svelte";

/** The feed shape this build understands. A newer one is ignored, not guessed at. */
const SUPPORTED_FEED_VERSION = 1;

/**
 * Announcements change a few times a month, so polling is enough and there is no
 * push channel to a static file anyway. Once at start-up, then every six hours.
 */
const POLL_INTERVAL_MS = 6 * 60 * 60 * 1000;

/** Setting key for the off switch. The switch itself arrives in Phase 3. */
export const ANNOUNCEMENTS_ENABLED_KEY = "announcements.enabled";

/** Setting key for the set of ids already shown. Ids only — never content. */
export const ANNOUNCEMENTS_READ_KEY = "announcements.read_ids";

/**
 * How many ids to remember.
 *
 * Enough that nothing still in the feed falls out of it, small enough that the
 * set cannot grow without bound. Oldest ids are dropped first, and an
 * announcement old enough to age out here has long since left the feed.
 */
const MAX_READ_IDS = 300;

/**
 * Caps on vendor-supplied text.
 *
 * The feed is content from the network rendered in a panel the user needs for
 * other things; a runaway title or body would push the transfer they are
 * cancelling off the screen. Numbers chosen to fit a headline and a short
 * paragraph, which is all this channel is for.
 */
const MAX_TITLE_LENGTH = 200;
const MAX_BODY_LENGTH = 1000;

/** The app's own version, injected at build time from `package.json`. */
const APP_VERSION: string = __APP_VERSION__;

// ===== Version comparison =====

/**
 * Numeric release components, or `null` when the string is not a version.
 *
 * Pre-release and build metadata are cut off rather than ordered: the feed
 * targets releases, and a full semver precedence implementation would be a lot
 * of rules in service of a case that does not arise.
 */
function parseVersion(v: string): number[] | null {
  const core = v.trim().replace(/^v/, "").split(/[-+]/)[0];
  if (!core) return null;
  const parts = core.split(".");
  const out: number[] = [];
  for (const part of parts) {
    if (!/^\d+$/.test(part)) return null;
    out.push(Number(part));
  }
  return out.length ? out : null;
}

/**
 * Compare two versions numerically: negative, zero or positive as `a` is older,
 * equal or newer. `null` when either side cannot be parsed.
 *
 * Numeric per component, not lexicographic — string comparison puts `1.10.0`
 * *below* `1.9.0`, which would hide an advisory from exactly the newer builds a
 * `minVersion` was written to reach.
 */
export function compareVersions(a: string, b: string): number | null {
  const left = parseVersion(a);
  const right = parseVersion(b);
  if (!left || !right) return null;
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    const diff = (left[i] ?? 0) - (right[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

// ===== Localisation =====

/**
 * The best available text for a locale.
 *
 * Falls back to the language without its region, then English, then whatever is
 * there. Showing text in the wrong language beats showing nothing: the feed is
 * translated by hand and a new entry may be English-only for a while.
 */
export function pickText(
  texts: Record<string, string> | null | undefined,
  lang: string,
  maxLength = MAX_BODY_LENGTH
): string | undefined {
  if (!texts) return undefined;
  const base = lang.split("-")[0];
  const candidate =
    texts[lang] ?? texts[base] ?? texts.en ?? Object.values(texts)[0];
  const trimmed = candidate?.trim();
  if (!trimmed) return undefined;
  // Truncated rather than rejected: the beginning of an over-long advisory is
  // still the warning. The cap exists so one entry cannot take over a panel the
  // user needs for cancelling transfers.
  return trimmed.length > maxLength ? `${trimmed.slice(0, maxLength)}…` : trimmed;
}

// ===== Filtering =====

export interface VisibilityContext {
  /** Milliseconds since the epoch, for `expiresAt`. */
  now: number;
  appVersion: string;
}

export type Severity = "info" | "warning" | "critical";

/** Anything unrecognised is `info`: a newer vocabulary must not raise alarm. */
export function normalizeSeverity(severity: string): Severity {
  return severity === "critical" || severity === "warning" ? severity : "info";
}

function matchesAudience(audience: string | null | undefined): boolean {
  if (!audience) return true;
  if (audience === "free") return true;
  // Anything else — including an audience this build does not know about.
  // Showing it would put content in front of people it was not written for,
  // which is the failure that matters here; the other direction is handled by
  // the `critical` bypass.
  return false;
}

function matchesVersion(a: Announcement, appVersion: string): boolean {
  if (a.minVersion) {
    const cmp = compareVersions(appVersion, a.minVersion);
    // Unparseable bound: no opinion. Treating it as a mismatch would let one
    // typo in the feed silence an entry for everyone.
    if (cmp !== null && cmp < 0) return false;
  }
  if (a.maxVersion) {
    const cmp = compareVersions(appVersion, a.maxVersion);
    if (cmp !== null && cmp > 0) return false;
  }
  return true;
}

/** Expiry is the one filter `critical` does not override — see `isVisible`. */
function isExpired(a: Announcement, now: number): boolean {
  if (!a.expiresAt) return false;
  const at = Date.parse(a.expiresAt);
  // An unparseable date means "no expiry stated", not "expired".
  return Number.isFinite(at) && at <= now;
}

/**
 * Whether this install should be shown this announcement.
 *
 * `critical` skips the audience and version filters. Both depend on state that
 * can be read wrongly — an audience this build has no name for, a version bound
 * that does not parse — and a security advisory must not be hidden by either.
 *
 * Expiry is not skipped: it is stated by the same author as the severity, so a
 * withdrawn advisory stays withdrawn.
 */
export function isVisible(a: Announcement, ctx: VisibilityContext): boolean {
  if (isExpired(a, ctx.now)) return false;
  if (normalizeSeverity(a.severity) === "critical") return true;
  return matchesAudience(a.audience) && matchesVersion(a, ctx.appVersion);
}

/** What actually goes on screen: visible, and not already shown once. */
export function selectAnnouncements(
  feed: AnnouncementFeed,
  ctx: VisibilityContext,
  readIds: ReadonlySet<string>
): Announcement[] {
  if (feed.version !== SUPPORTED_FEED_VERSION) return [];
  // Also deduplicated within the batch: announcements are never collapsed by the
  // store, so a feed that repeats an id would put the same news on screen twice.
  const seen = new Set(readIds);
  const picked: Announcement[] = [];
  for (const a of feed.announcements) {
    if (!a.id || seen.has(a.id) || !isVisible(a, ctx)) continue;
    seen.add(a.id);
    picked.push(a);
  }
  return picked;
}

// ===== Read set =====

export function readIds(): Set<string> {
  const raw = getAppSetting(ANNOUNCEMENTS_READ_KEY, "");
  if (!raw) return new Set();
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return new Set();
    return new Set(parsed.filter((v): v is string => typeof v === "string"));
  } catch {
    // Unreadable set: start over rather than refuse to show anything. The cost
    // is one repeat of what is currently in the feed.
    return new Set();
  }
}

/**
 * Remember that these ids have been shown.
 *
 * Newest last, and the oldest fall off the front once the cap is met.
 */
export async function markShown(ids: string[]): Promise<void> {
  if (!ids.length) return;
  const kept = [...readIds()].filter((id) => !ids.includes(id));
  kept.push(...ids);
  const trimmed = kept.slice(Math.max(0, kept.length - MAX_READ_IDS));
  await setAppSetting(ANNOUNCEMENTS_READ_KEY, JSON.stringify(trimmed));
}

// ===== Polling =====

/**
 * Off means no request, not a filtered result — the fetch itself discloses an IP,
 * a time and an app version to the host, so the only honest switch is one that
 * stops it happening. Default on; the UI for it lands in Phase 3.
 */
export function announcementsEnabled(): boolean {
  return getAppSetting(ANNOUNCEMENTS_ENABLED_KEY, "true") !== "false";
}

/**
 * At most one poll at a time.
 *
 * The start-up poll and an interval tick can otherwise overlap — in browser mode
 * the fetch has no timeout at all — and both would read the set of shown ids
 * before either had written to it, putting the same announcement on screen twice.
 */
let inFlight: Promise<void> | null = null;

/**
 * Fetch once and show whatever is new.
 *
 * Never throws. A failed fetch leaves the list exactly as it was.
 */
export function pollAnnouncements(): Promise<void> {
  if (inFlight) return inFlight;
  inFlight = poll().finally(() => {
    inFlight = null;
  });
  return inFlight;
}

async function poll(): Promise<void> {
  if (!announcementsEnabled()) return;

  let feed: AnnouncementFeed;
  try {
    feed = await announcementsApi.fetch();
  } catch (e) {
    // Debug level on purpose: an offline laptop is not an event worth logging at
    // a level anyone reads, and it is certainly not worth a toast.
    console.debug("Announcements unavailable:", e);
    return;
  }

  const lang = getLanguage();
  const fresh = selectAnnouncements(
    feed,
    { now: Date.now(), appVersion: APP_VERSION },
    readIds()
  );

  const shown: string[] = [];
  for (const a of fresh) {
    const title = pickText(a.title, lang, MAX_TITLE_LENGTH);
    // No title in any language is a broken entry, not a blank notification.
    if (!title) continue;
    pushNotification({
      kind: "announcement",
      // Nothing is pending and nothing failed; this is news that has arrived.
      status: "success",
      title,
      detail: pickText(a.body, lang),
      announcement: {
        announcementId: a.id,
        severity: normalizeSeverity(a.severity),
        linkUrl: a.linkUrl ?? undefined,
      },
    });
    shown.push(a.id);
  }

  await markShown(shown);
}

/**
 * Poll at start-up and every six hours. Returns a teardown.
 *
 * Called from `App.svelte` rather than a component, because nothing about this
 * belongs to a screen the user can navigate away from.
 */
export function startAnnouncements(): () => void {
  void pollAnnouncements();
  const timer = setInterval(() => void pollAnnouncements(), POLL_INTERVAL_MS);
  return () => clearInterval(timer);
}
