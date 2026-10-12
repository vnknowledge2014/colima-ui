/**
 * Teardown contract for the data poller.
 *
 * Every bug this file guards against had the same shape: work scheduled before
 * a teardown ran anyway afterwards, against a context that no longer existed.
 * They are covered here because the module owns process-wide resources — an
 * interval, a retry timeout and an EventSource — and nothing else can observe
 * whether they were released.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const listContainers = vi.fn();
const listImages = vi.fn();
const listVolumes = vi.fn();
const listNetworks = vi.fn();

/** Instances handed out by the module under test, so a test can assert on them. */
const eventSources: FakeEventSource[] = [];

class FakeEventSource {
  url: string;
  closed = false;
  listeners = new Map<string, (e: MessageEvent) => void>();
  onerror: ((e: unknown) => void) | null = null;
  constructor(url: string) {
    this.url = url;
    eventSources.push(this);
  }
  addEventListener(type: string, fn: (e: MessageEvent) => void) {
    this.listeners.set(type, fn);
  }
  close() {
    this.closed = true;
  }
}

vi.mock("./api", () => ({
  colimaApi: { listInstances: vi.fn().mockResolvedValue([]) },
  dockerApi: {
    listContainers: (...a: unknown[]) => listContainers(...a),
    listImages: (...a: unknown[]) => listImages(...a),
  },
  volumesApi: { listVolumes: (...a: unknown[]) => listVolumes(...a) },
  networksApi: { listNetworks: (...a: unknown[]) => listNetworks(...a) },
  sysMethods: {
    checkSystem: vi.fn().mockResolvedValue({}),
    setResourceSaver: vi.fn().mockResolvedValue(undefined),
  },
  getApiToken: vi.fn().mockResolvedValue("t"),
}));
vi.mock("./api/client", () => ({ resolveApiBase: vi.fn().mockResolvedValue("http://x") }));
vi.mock("../store/capabilities.svelte", () => ({ loadCapabilities: vi.fn() }));
vi.mock("./normalizers", () => ({ normalizeContainer: (c: unknown) => c, normalizeImage: (i: unknown) => i }));
vi.mock("./settingsStore.svelte", () => ({ getAppSetting: () => "" }));
// Browser mode: the SSE branch is the one that owns an EventSource.
vi.mock("./env", () => ({ isRunningInTauri: () => false }));
vi.mock("../store.svelte", () => ({
  dockerState: { containers: [], images: [], loading: true },
  resourceState: { volumes: [], networks: [], volumesLoading: true, networksLoading: true },
  dashboardState: { colimaInstances: [], systemInfo: {}, instancesLoaded: false },
  uiState: { currentPage: "dashboard" },
  isEventCooldownActive: () => false,
}));
vi.mock("./visibleInterval", () => ({ setVisibleInterval: () => () => {} }));

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  eventSources.length = 0;
  vi.stubGlobal("EventSource", FakeEventSource as unknown as typeof EventSource);
  listContainers.mockResolvedValue([]);
  listImages.mockResolvedValue([]);
  listVolumes.mockResolvedValue([]);
  listNetworks.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("teardown", () => {
  it("stops refetching once torn down", async () => {
    const { startDataPoller, refetchAllResources } = await import("./dataPoller");
    const stop = startDataPoller();
    await vi.runOnlyPendingTimersAsync();
    const before = listContainers.mock.calls.length;

    stop();
    await refetchAllResources();

    // A call that lands after teardown must not reach the network at all;
    // it used to keep polling a context that was already gone.
    expect(listContainers.mock.calls.length).toBe(before);
  });

  it("closes the EventSource so the connection is not leaked", async () => {
    const { startDataPoller } = await import("./dataPoller");
    const stop = startDataPoller();
    await vi.runOnlyPendingTimersAsync();
    expect(eventSources.length).toBeGreaterThan(0);

    stop();

    // Chrome allows six concurrent connections per origin; a leak here made
    // every later request to the API block, including the polling fallback.
    expect(eventSources.every((es) => es.closed)).toBe(true);
  });

  it("clears the retry timeout a failed refetch scheduled", async () => {
    listVolumes.mockRejectedValue(new Error("down"));
    const { startDataPoller } = await import("./dataPoller");
    const stop = startDataPoller();
    await vi.runOnlyPendingTimersAsync();

    // The failure must actually have armed a retry, or this proves nothing.
    expect(vi.getTimerCount()).toBeGreaterThan(0);

    stop();

    // The timer itself is asserted on, not its network effect: the guard at the
    // top of refetchAllResources would swallow the effect and let an orphaned
    // timeout — the original bug — go unnoticed.
    expect(vi.getTimerCount()).toBe(0);
  });

  it("a restart after teardown polls again instead of inheriting disposed", async () => {
    const { startDataPoller } = await import("./dataPoller");
    startDataPoller()();
    const after = listContainers.mock.calls.length;

    const stop2 = startDataPoller();
    await vi.runOnlyPendingTimersAsync();

    // HMR and a re-mounted root both take this path.
    expect(listContainers.mock.calls.length).toBeGreaterThan(after);
    stop2();
  });
});
