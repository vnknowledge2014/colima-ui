import { describe, it, expect, beforeEach, vi } from "vitest";

/**
 * The behaviours worth pinning down are the ones that fail silently and in the
 * wrong direction: an advisory hidden by a filter, a version compared as text, a
 * network blip clearing what the user was reading, and news that comes back
 * every time the app starts.
 */

vi.mock("./api/announcements", () => ({
  announcementsApi: { fetch: vi.fn() },
}));

// The read-set is written through the settings API, which has no server here.
// Stubbed so a persistence failure does not look like part of the test output.
vi.mock("./api/settings", () => ({
  settingsApi: {
    getAll: vi.fn(async () => ({})),
    set: vi.fn(async () => undefined),
  },
}));

import { announcementsApi, type Announcement, type AnnouncementFeed } from "./api/announcements";
import {
  compareVersions,
  pickText,
  isVisible,
  normalizeSeverity,
  selectAnnouncements,
  pollAnnouncements,
  readIds,
  ANNOUNCEMENTS_ENABLED_KEY,
  ANNOUNCEMENTS_READ_KEY,
} from "./announcements";
import { settingsState } from "./settingsStore.svelte";
import {
  notificationState,
  pushNotification,
  _resetNotificationsForTest,
} from "../store/notifications.svelte";

const fetchMock = vi.mocked(announcementsApi.fetch);

function announcement(over: Partial<Announcement> = {}): Announcement {
  return {
    id: "a1",
    publishedAt: "2026-08-01T00:00:00Z",
    severity: "info",
    title: { en: "Release 0.2.0" },
    ...over,
  };
}

function feed(...items: Announcement[]): AnnouncementFeed {
  return { version: 1, announcements: items };
}

const ctx = { now: Date.parse("2026-08-12T00:00:00Z"), appVersion: "1.10.0" };

beforeEach(() => {
  fetchMock.mockReset();
  _resetNotificationsForTest();
  for (const key of Object.keys(settingsState)) delete settingsState[key];
});

describe("compareVersions", () => {
  it("compares numerically, not as text", () => {
    // The bug this exists to prevent: "1.10.0" < "1.9.0" as strings, which would
    // hide a `minVersion: "1.9.0"` advisory from every 1.10 build.
    expect(compareVersions("1.10.0", "1.9.0")).toBeGreaterThan(0);
    expect(compareVersions("0.1.10", "0.1.9")).toBeGreaterThan(0);
    expect(compareVersions("1.2.3", "1.2.3")).toBe(0);
  });

  it("treats missing components as zero", () => {
    expect(compareVersions("1.2", "1.2.0")).toBe(0);
    expect(compareVersions("1.2", "1.2.1")).toBeLessThan(0);
  });

  it("has no opinion on strings that are not versions", () => {
    expect(compareVersions("nightly", "1.0.0")).toBeNull();
  });
});

describe("visibility", () => {
  it("shows content addressed to everyone, and to this build's own audience", () => {
    expect(isVisible(announcement({ audience: null }), ctx)).toBe(true);
    expect(isVisible(announcement({ audience: "free" }), ctx)).toBe(true);
  });

  it("hides an audience this build is not part of", () => {
    expect(isVisible(announcement({ audience: "pro" }), ctx)).toBe(false);
  });

  it("shows a critical advisory addressed to another audience", () => {
    // An audience this build has no name for must not be able to hide a
    // security advisory.
    const advisory = announcement({ audience: "pro", severity: "critical" });
    expect(isVisible(advisory, ctx)).toBe(true);
  });

  it("shows a critical advisory outside its version range", () => {
    const advisory = announcement({ severity: "critical", minVersion: "9.0.0" });
    expect(isVisible(advisory, ctx)).toBe(true);
  });

  it("applies version bounds to ordinary entries", () => {
    expect(isVisible(announcement({ minVersion: "1.9.0" }), ctx)).toBe(true);
    expect(isVisible(announcement({ minVersion: "2.0.0" }), ctx)).toBe(false);
    expect(isVisible(announcement({ maxVersion: "1.9.0" }), ctx)).toBe(false);
  });

  it("ignores a bound it cannot parse rather than silencing the entry", () => {
    expect(isVisible(announcement({ minVersion: "not-a-version" }), ctx)).toBe(true);
  });

  it("hides expired entries, including critical ones", () => {
    const expired = { expiresAt: "2026-08-01T00:00:00Z" };
    expect(isVisible(announcement(expired), ctx)).toBe(false);
    expect(isVisible(announcement({ ...expired, severity: "critical" }), ctx)).toBe(false);
  });

  it("downgrades an unrecognised severity instead of dropping the entry", () => {
    expect(normalizeSeverity("apocalyptic")).toBe("info");
    expect(isVisible(announcement({ severity: "apocalyptic" }), ctx)).toBe(true);
  });
});

describe("selectAnnouncements", () => {
  it("skips ids already shown", () => {
    const items = [announcement({ id: "old" }), announcement({ id: "new" })];
    const picked = selectAnnouncements(feed(...items), ctx, new Set(["old"]));
    expect(picked.map((a) => a.id)).toEqual(["new"]);
  });

  it("ignores a feed version it does not understand", () => {
    const future: AnnouncementFeed = { version: 99, announcements: [announcement()] };
    expect(selectAnnouncements(future, ctx, new Set())).toEqual([]);
  });
});

describe("pickText", () => {
  it("prefers the exact locale, then the base language, then English", () => {
    const texts = { en: "Hello", vi: "Xin chào" };
    expect(pickText(texts, "vi")).toBe("Xin chào");
    expect(pickText(texts, "vi-VN")).toBe("Xin chào");
    expect(pickText(texts, "ja")).toBe("Hello");
  });

  it("truncates text that would take over the panel", () => {
    const long = "x".repeat(5000);
    const shown = pickText({ en: long }, "en");
    expect(shown!.length).toBeLessThanOrEqual(1001);
    expect(shown!.endsWith("…")).toBe(true);
  });

  it("treats blank text as absent", () => {
    expect(pickText({ en: "   " }, "en")).toBeUndefined();
    expect(pickText(undefined, "en")).toBeUndefined();
  });
});

describe("pollAnnouncements", () => {
  it("pushes new announcements and remembers their ids", async () => {
    fetchMock.mockResolvedValue(feed(announcement({ id: "rel-0.2.0" })));

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].kind).toBe("announcement");
    expect(notificationState.entries[0].title).toBe("Release 0.2.0");
    expect(readIds().has("rel-0.2.0")).toBe(true);
  });

  it("does not show the same announcement twice", async () => {
    settingsState[ANNOUNCEMENTS_READ_KEY] = JSON.stringify(["rel-0.2.0"]);
    fetchMock.mockResolvedValue(feed(announcement({ id: "rel-0.2.0" })));

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(0);
  });

  it("keeps two announcements apart instead of collapsing them", async () => {
    fetchMock.mockResolvedValue(
      feed(
        announcement({ id: "one", title: { en: "Same headline" } }),
        announcement({ id: "two", title: { en: "Same headline" } })
      )
    );

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(2);
    expect(notificationState.entries.every((e) => e.count === 1)).toBe(true);
  });

  it("leaves existing notifications alone when the feed cannot be read", async () => {
    pushNotification({ title: "A local thing that happened" });
    fetchMock.mockRejectedValue(new Error("offline"));

    await expect(pollAnnouncements()).resolves.toBeUndefined();

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].title).toBe("A local thing that happened");
  });

  it("makes no request at all when switched off", async () => {
    settingsState[ANNOUNCEMENTS_ENABLED_KEY] = "false";

    await pollAnnouncements();

    // Off has to mean no request: the fetch itself is what discloses an IP and a
    // timestamp, so filtering the result afterwards would not be off at all.
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("polls by default, with no setting written", async () => {
    fetchMock.mockResolvedValue(feed());

    await pollAnnouncements();

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("runs one poll at a time", async () => {
    fetchMock.mockResolvedValue(feed(announcement({ id: "once" })));

    await Promise.all([pollAnnouncements(), pollAnnouncements()]);

    // Both callers share the one request, so the same news cannot be pushed by
    // two overlapping polls that each read the id set before either wrote it.
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(notificationState.entries).toHaveLength(1);
  });

  it("keeps the remembered id set bounded", async () => {
    const existing = Array.from({ length: 300 }, (_, i) => `old-${i}`);
    settingsState[ANNOUNCEMENTS_READ_KEY] = JSON.stringify(existing);
    fetchMock.mockResolvedValue(feed(announcement({ id: "newest" })));

    await pollAnnouncements();

    const ids = readIds();
    expect(ids.size).toBe(300);
    expect(ids.has("newest")).toBe(true);
    // The oldest falls out first, and anything that old has long left the feed.
    expect(ids.has("old-0")).toBe(false);
  });

  it("shows an id from the feed only once, even if the feed repeats it", async () => {
    fetchMock.mockResolvedValue(
      feed(announcement({ id: "dup" }), announcement({ id: "dup" }))
    );

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(1);
  });

  it("hides an audience this build does not know", async () => {
    fetchMock.mockResolvedValue(feed(announcement({ audience: "enterprise" })));

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(0);
  });

  it("skips an entry with no usable title", async () => {
    fetchMock.mockResolvedValue(feed(announcement({ id: "blank", title: {} })));

    await pollAnnouncements();

    expect(notificationState.entries).toHaveLength(0);
  });
});
