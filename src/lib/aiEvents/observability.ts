import { EventHandler } from "./types";
import { topologyApi } from "../api/topology";
import { activityApi, type ActivityKind } from "../api/activity";
import { alertsApi } from "../api/metrics";
import { selfHealApi } from "../api/self-heal";

export const observabilityRegistry: Record<string, EventHandler> = {
  "topology-get": {
    category: "SAFE",
    description: "Get the live Docker topology graph (containers, networks, volumes, projects, images, edges)",
    handler: async () => JSON.stringify(await topologyApi.getTopology(), null, 2)
  },
  "activity-feed": {
    category: "SAFE",
    description: "Get the unified activity timeline across lifecycle, security scans, alerts, self-heal, and compose fixes",
    handler: async (p) => {
      const fromRaw = p?.fromMs ?? p?.from_ms;
      const toRaw = p?.toMs ?? p?.to_ms;
      const limitRaw = p?.limit;
      const feed = await activityApi.feed({
        kind: typeof p?.kind === "string" ? (p.kind as ActivityKind) : undefined,
        target: typeof p?.target === "string" ? p.target : undefined,
        fromMs: fromRaw !== undefined ? Number(fromRaw) : undefined,
        toMs: toRaw !== undefined ? Number(toRaw) : undefined,
        limit: limitRaw !== undefined ? Number(limitRaw) : undefined,
      });
      return JSON.stringify(feed, null, 2);
    }
  },
  "alerts-rules": {
    category: "SAFE",
    description: "List configured metric alert rules",
    handler: async () => JSON.stringify(await alertsApi.listRules(), null, 2)
  },
  "alerts-events": {
    category: "SAFE",
    description: "List recent metric alert firing events",
    handler: async (p) => {
      const limit = p?.limit !== undefined ? Number(p.limit) : 50;
      return JSON.stringify(await alertsApi.recentEvents(limit), null, 2);
    }
  },
  "self-heal-rules": {
    category: "SAFE",
    description: "List configured self-healing trigger and action rules",
    handler: async () => JSON.stringify(await selfHealApi.listRules(), null, 2)
  },
  "self-heal-log": {
    category: "SAFE",
    description: "List recent self-healing execution and suggestion log entries",
    handler: async (p) => {
      const limit = p?.limit !== undefined ? Number(p.limit) : 50;
      return JSON.stringify(await selfHealApi.recentLog(limit), null, 2);
    }
  }
};
