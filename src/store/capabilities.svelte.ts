/**
 * Which host tools are installed and usable.
 *
 * Every page that can be empty for two different reasons — "you have nothing
 * yet" versus "the tool isn't installed" — reads this instead of guessing from
 * an empty list. Previously only `SetupWizard.svelte` knew any of this, so the
 * rest of the app showed a blank table and left the user to work it out.
 */

import { systemApi, type Capability, type CapabilityState } from "../lib/api";
import { t } from "../lib/i18n.svelte";

export const capabilitiesState = $state({
  items: [] as Capability[],
  loading: false,
  /** Set when detection itself failed, as opposed to a tool being missing. */
  failed: false,
});

let _inflight: Promise<void> | null = null;

/**
 * Load capabilities. Concurrent callers share one request.
 *
 * The backend caches for 15s and invalidates when an instance changes state,
 * so callers can refresh freely after an action without causing a process
 * storm.
 */
export async function loadCapabilities(): Promise<void> {
  if (_inflight) return _inflight;

  capabilitiesState.loading = true;
  _inflight = (async () => {
    try {
      capabilitiesState.items = await systemApi.getCapabilities();
      capabilitiesState.failed = false;
    } catch {
      // Detection failing is not the same as a tool being absent — leave the
      // previous answer in place rather than claiming everything is missing.
      capabilitiesState.failed = true;
    } finally {
      capabilitiesState.loading = false;
      _inflight = null;
    }
  })();

  return _inflight;
}

export function capability(id: string): Capability | undefined {
  return capabilitiesState.items.find((c) => c.id === id);
}

export function capabilityState(id: string): CapabilityState {
  return capability(id)?.state ?? "unknown";
}

/** True when the tool can be used right now. */
export function isUsable(id: string): boolean {
  return capabilityState(id) === "running";
}

/**
 * Why a page is empty, so it can say something more useful than "no items".
 *
 * Returns `undefined` when the tool is fine and the list is genuinely empty.
 */
export function blockingCapability(...ids: string[]): Capability | undefined {
  for (const id of ids) {
    const cap = capability(id);
    if (cap && cap.state !== "running") return cap;
  }
  return undefined;
}

export interface CapabilityNotice {
  title: string;
  text: string;
}

/**
 * What a page should say when a missing or stopped tool is the reason it has
 * nothing to show.
 *
 * A plain function rather than a component on purpose: the app already has two
 * onboarding surfaces (`SetupWizard` and `GettingStartedTour`), and each page
 * has its own `.empty-state` markup. Pages pass these strings into the markup
 * they already have.
 */
export function capabilityNotice(cap: Capability): CapabilityNotice {
  const name = cap.name;

  if (cap.state === "missing") {
    return {
      title: t("capabilities.missing.title", { name, default: `${name} is not installed` }),
      text: cap.install_hint
        ? t("capabilities.missing.text_with_hint", {
            hint: cap.install_hint,
            default: `Install it to use this page: ${cap.install_hint}`,
          })
        : t("capabilities.missing.text", { default: "Install it to use this page." }),
    };
  }

  if (cap.state === "installed_not_running") {
    return {
      title: t("capabilities.not_running.title", { name, default: `${name} is not running` }),
      text: t("capabilities.not_running.text", {
        default: "Start a Colima instance from the Instances page, then come back.",
      }),
    };
  }

  return {
    title: t("capabilities.unknown.title", { name, default: `Could not check ${name}` }),
    text: t("capabilities.unknown.text", {
      default: "Detection failed. Check that the tool is on your PATH.",
    }),
  };
}
