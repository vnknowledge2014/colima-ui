import { call } from "./client";

// ===== Topology API =====

/** `service` is a compose service declared but never created — see `notCreated`. */
export type TopologyNodeKind =
  | "container"
  | "network"
  | "volume"
  | "project"
  | "image"
  | "service";
/**
 * `notCreated` is distinct from `stopped`: a stopped container exists and can be
 * started, while a not-created service only exists in a compose file. Collapsing
 * the two would tell the user to start something that is not there.
 */
export type TopologyNodeStatus =
  | "running"
  | "stopped"
  | "unhealthy"
  | "notCreated"
  | "none";
export type TopologyEdgeKind = "network" | "volume" | "project" | "image" | "dependsOn";

export interface TopologyNode {
  id: string;
  kind: TopologyNodeKind;
  label: string;
  status: TopologyNodeStatus;
  meta: Record<string, unknown>;
}

export interface TopologyEdge {
  from: string;
  to: string;
  kind: TopologyEdgeKind;
}

export interface TopologyGraph {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  /** Subsystems the backend could not list; the graph is incomplete, not empty. */
  warnings: string[];
}

export const topologyApi = {
  /**
   * The Docker graph for the current engine, assembled backend-side in one
   * round-trip. No instance parameter — nothing in this app targets a second
   * engine, so there is nothing to pass.
   */
  getTopology: async (): Promise<TopologyGraph> => {
    const raw = await call<TopologyGraph>("get_topology", undefined, "GET", "/api/topology");
    return { nodes: raw?.nodes ?? [], edges: raw?.edges ?? [], warnings: raw?.warnings ?? [] };
  },
};
