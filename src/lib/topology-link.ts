/**
 * Cross-page navigation between the topology graph and the list pages.
 *
 * Five list pages need the same two moves — send the user to the graph focused
 * on one resource, and pick up a focus request on arrival — so the handoff lives
 * here rather than being retyped five times with five chances to forget the
 * clearing step.
 *
 * Contract matches `uiState.helpArticle`: set the request, then navigate; the
 * destination consumes it and clears it, so returning to that page later opens
 * clean instead of re-focusing something stale.
 */

import { uiState } from "../store.svelte";
import type { TopologyNodeKind } from "./api/topology";

/**
 * Jump to the Topology page with one resource selected.
 *
 * `name` is the resource's own identity (container id, network name, …); the
 * graph's node ids are that value prefixed by kind.
 */
export function viewInTopology(kind: TopologyNodeKind, name: string): void {
  uiState.focusResource = { page: "topology", id: `${kind}:${name}` };
  uiState.currentPage = "topology";
}

/**
 * Take the pending focus for `page`, if there is one, and clear it.
 *
 * Returns null when nothing was requested or the request was aimed elsewhere —
 * a focus the user abandoned mid-navigation must not fire on whatever page they
 * happen to land on next.
 */
export function consumeFocus(page: string): string | null {
  const pending = uiState.focusResource;
  if (pending?.page !== page) return null;
  uiState.focusResource = null;
  return pending.id;
}
