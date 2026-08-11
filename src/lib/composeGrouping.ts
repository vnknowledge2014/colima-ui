/**
 * Group containers by the Compose project they belong to.
 *
 * Docker Compose stamps every container it creates with
 * `com.docker.compose.project`. Containers started by hand have no such label
 * and land in a single "Standalone" group rather than being hidden.
 *
 * Pure functions, no UI: the grouping rules are the part worth testing, and
 * they should not need a component mounted to exercise.
 */

import type { DockerContainer } from "./api";

export const COMPOSE_PROJECT_LABEL = "com.docker.compose.project";
export const COMPOSE_SERVICE_LABEL = "com.docker.compose.service";

/** Group id for containers that are not part of any Compose project. */
export const STANDALONE_GROUP = "__standalone__";

export interface ContainerGroup {
  /** `STANDALONE_GROUP` or the Compose project name. */
  id: string;
  /** Project name, or empty for the standalone group. */
  project: string;
  containers: DockerContainer[];
  total: number;
  running: number;
}

function projectOf(container: DockerContainer): string {
  const labels = container.Labels;
  if (!labels) return "";
  const value = labels[COMPOSE_PROJECT_LABEL];
  return typeof value === "string" ? value.trim() : "";
}

/** The Compose service name, used as the display name inside a project group. */
export function serviceOf(container: DockerContainer): string {
  const value = container.Labels?.[COMPOSE_SERVICE_LABEL];
  return typeof value === "string" ? value.trim() : "";
}

function isRunning(container: DockerContainer): boolean {
  return (container.State || "").toLowerCase() === "running";
}

/**
 * Group containers by project.
 *
 * Projects come first in alphabetical order, with standalone containers last —
 * they are the ones the user is least likely to be looking for, and putting a
 * variable-size group in the middle makes the list jump around.
 */
export function groupContainersByProject(containers: DockerContainer[]): ContainerGroup[] {
  const byProject = new Map<string, DockerContainer[]>();

  for (const container of containers) {
    const project = projectOf(container);
    const key = project || STANDALONE_GROUP;
    const bucket = byProject.get(key);
    if (bucket) {
      bucket.push(container);
    } else {
      byProject.set(key, [container]);
    }
  }

  const groups: ContainerGroup[] = [];
  for (const [key, items] of byProject) {
    groups.push({
      id: key,
      project: key === STANDALONE_GROUP ? "" : key,
      containers: items,
      total: items.length,
      running: items.filter(isRunning).length,
    });
  }

  return groups.sort((a, b) => {
    if (a.id === STANDALONE_GROUP) return 1;
    if (b.id === STANDALONE_GROUP) return -1;
    return a.project.localeCompare(b.project);
  });
}
