import { call } from "./client";

/**
 * The vendor announcement feed, as the backend hands it over.
 *
 * Mirrors `AnnouncementFeed` in `src-tauri/src/commands/announcements.rs`. The
 * fetch itself lives there so the webview's CSP stays untouched and exactly one
 * URL is reachable; this side only reads the result.
 *
 * Unknown fields are kept out of the type on purpose — a newer feed may carry
 * more, and both sides ignore what they do not understand rather than break.
 */
export interface Announcement {
  /** Stable and human-authored; this is the identity the read-set remembers. */
  id: string;
  publishedAt: string;
  expiresAt?: string | null;
  /** `info` | `warning` | `critical`, but typed loosely: an unrecognised value
   *  from a newer feed is downgraded to `info` rather than dropped. */
  severity: string;
  /** `null` = everyone. Filtering happens here, the only side that knows whether
   *  this install is paid. */
  audience?: string | null;
  minVersion?: string | null;
  maxVersion?: string | null;
  /** Locale code → text. The app ships four languages. */
  title: Record<string, string>;
  body?: Record<string, string> | null;
  linkUrl?: string | null;
}

export interface AnnouncementFeed {
  version: number;
  announcements: Announcement[];
}

export const announcementsApi = {
  /**
   * Read the feed. Rejects when it could not be read — deliberately not an empty
   * feed, so the caller can keep showing what it already had.
   */
  fetch: () =>
    call<AnnouncementFeed>("announcements_fetch", undefined, "GET", "/api/announcements"),
};
