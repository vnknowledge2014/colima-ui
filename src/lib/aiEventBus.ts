import { EventHandler, ActionCategory } from "./aiEvents/types";
import { colimaRegistry } from "./aiEvents/colima";
import { dockerRegistry } from "./aiEvents/docker";
import { volumesRegistry } from "./aiEvents/volumes";
import { systemRegistry } from "./aiEvents/system";
import { composeRegistry } from "./aiEvents/compose";
import { k8sRegistry } from "./aiEvents/k8s";
import { limaRegistry } from "./aiEvents/lima";
import { configRegistry } from "./aiEvents/config";

export type { ActionCategory, EventHandler };

export const registry: Record<string, EventHandler> = {
  ...colimaRegistry,
  ...dockerRegistry,
  ...volumesRegistry,
  ...systemRegistry,
  ...composeRegistry,
  ...k8sRegistry,
  ...limaRegistry,
  ...configRegistry,
};

export function getCategory(eventName: string): ActionCategory | null {
  return registry[eventName]?.category || null;
}

export async function executeEvent(eventName: string, payload: any): Promise<string> {
  const handlerInfo = registry[eventName];
  if (!handlerInfo) {
    throw new Error(`Unknown event: ${eventName}`);
  }
  return await handlerInfo.handler(payload);
}

export function listEvents(): { name: string; category: ActionCategory; description: string }[] {
  return Object.entries(registry).map(([name, { category, description }]) => ({
    name, category, description
  }));
}
