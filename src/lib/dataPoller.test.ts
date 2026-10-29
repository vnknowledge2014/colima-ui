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

describe("disconnect error handling and backoff", () => {
  it("clears dockerState.containers and triggers refreshManual when listContainers fails", async () => {
    const { colimaApi } = await import("./api");
    const { dockerState } = await import("../store.svelte");
    const { refetchAllResources } = await import("./dataPoller");

    // Populate with existing container
    dockerState.containers = [{ Id: "c-active", Names: "web" } as unknown as (typeof dockerState.containers)[number]];
    expect(dockerState.containers.length).toBe(1);


    vi.mocked(colimaApi.listInstances).mockClear();
    listContainers.mockRejectedValue(new Error("Cannot connect to Docker daemon"));

    await refetchAllResources();

    // Verify containers are cleared on connection failure
    expect(dockerState.containers).toEqual([]);
    // Verify refreshManual was called (which invokes colimaApi.listInstances)
    expect(colimaApi.listInstances).toHaveBeenCalled();
  });

  it("applies 5s retry intervals initially, then 15s backoff once retry count reaches MAX_REFETCH_RETRIES (10)", async () => {
    listContainers.mockRejectedValue(new Error("daemon offline"));
    const { startDataPoller } = await import("./dataPoller");

    const stop = startDataPoller();
    // Flush initial microtasks (the initial refetchAllResources() promise)
    await Promise.resolve();
    await Promise.resolve();


    // Initial refetchAllResources() was attempt 0, which failed and set _refetchRetryCount = 1 (delay 5000ms).
    // Retries 1 through 8 all use 5000ms delay:
    for (let retry = 1; retry <= 8; retry++) {
      const callsBefore = listContainers.mock.calls.length;
      await vi.advanceTimersByTimeAsync(4000);
      expect(listContainers.mock.calls.length).toBe(callsBefore);
      await vi.advanceTimersByTimeAsync(1000);
      expect(listContainers.mock.calls.length).toBe(callsBefore + 1);
    }

    // Now attempt 8 finished and incremented _refetchRetryCount to 9.
    // Since 9 < 10, retry 9 still uses 5000ms delay:
    const callsBefore9 = listContainers.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(listContainers.mock.calls.length).toBe(callsBefore9 + 1);

    // Attempt 9 ran and failed. It incremented _refetchRetryCount to 10 (MAX_REFETCH_RETRIES).
    // Because 10 < 10 is FALSE, the next scheduled retry MUST use 15000ms (15s backoff).
    const callsAtMax = listContainers.mock.calls.length;

    // After 5000ms (old interval), no call should happen
    await vi.advanceTimersByTimeAsync(5000);
    expect(listContainers.mock.calls.length).toBe(callsAtMax);

    // After 10000ms (total 10s), still no call
    await vi.advanceTimersByTimeAsync(5000);
    expect(listContainers.mock.calls.length).toBe(callsAtMax);

    // After 14900ms, still no call
    await vi.advanceTimersByTimeAsync(4900);
    expect(listContainers.mock.calls.length).toBe(callsAtMax);

    // Advancing the final 100ms (total 15000ms = 15s) triggers the 10th retry!
    await vi.advanceTimersByTimeAsync(100);
    expect(listContainers.mock.calls.length).toBe(callsAtMax + 1);

    // Subsequent failure (retry 11) also uses 15000ms backoff
    const callsAt11 = listContainers.mock.calls.length;
    await vi.advanceTimersByTimeAsync(14000);
    expect(listContainers.mock.calls.length).toBe(callsAt11);
    await vi.advanceTimersByTimeAsync(1000);
    expect(listContainers.mock.calls.length).toBe(callsAt11 + 1);

    stop();
  });

  it("resets backoff interval to 5s after successful reconnection", async () => {
    listContainers.mockRejectedValue(new Error("daemon offline"));
    const { startDataPoller } = await import("./dataPoller");
    const { dockerState } = await import("../store.svelte");

    const stop = startDataPoller();
    await Promise.resolve();
    await Promise.resolve();

    // Fast-forward to 10 failures (10th failure enters 15s backoff)
    // 9 retries at 5s = 45s
    await vi.advanceTimersByTimeAsync(45000);

    // Daemon recovers!
    const recoveredContainer = { Id: "c-rec", Names: "recovered", State: "running" };
    listContainers.mockResolvedValue([recoveredContainer]);

    // Advance 15s to trigger the backoff retry
    await vi.advanceTimersByTimeAsync(15000);
    await Promise.resolve();
    await Promise.resolve();

    // State populated, retry count reset
    expect(dockerState.containers).toEqual([recoveredContainer]);

    // Now if it fails again in the future:
    listContainers.mockRejectedValue(new Error("transient glitch"));
    // Trigger manual refetch to simulate a new failure
    const { refetchAllResources } = await import("./dataPoller");
    await refetchAllResources();

    // Verify it uses the fresh 5000ms delay, NOT 15000ms backoff
    const calls = listContainers.mock.calls.length;
    await vi.advanceTimersByTimeAsync(4000);
    expect(listContainers.mock.calls.length).toBe(calls);
    await vi.advanceTimersByTimeAsync(1000);
    expect(listContainers.mock.calls.length).toBe(calls + 1);

    stop();
  });
});


