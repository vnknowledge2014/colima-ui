import { describe, it, expect } from "vitest";
import {
  groupContainersByProject,
  serviceOf,
  STANDALONE_GROUP,
  COMPOSE_PROJECT_LABEL,
  COMPOSE_SERVICE_LABEL,
} from "./composeGrouping";
import type { DockerContainer } from "./api";

function container(
  id: string,
  state: string,
  labels?: Record<string, string>,
): DockerContainer {
  return {
    Id: id,
    Names: id,
    Image: "nginx",
    Status: state,
    State: state,
    Ports: "",
    CreatedAt: "0",
    Size: "0",
    Command: "",
    Labels: labels,
  } as DockerContainer;
}

const inProject = (id: string, project: string, service = "web", state = "running") =>
  container(id, state, {
    [COMPOSE_PROJECT_LABEL]: project,
    [COMPOSE_SERVICE_LABEL]: service,
  });

describe("groupContainersByProject", () => {
  it("groups by compose project", () => {
    const groups = groupContainersByProject([
      inProject("a", "shop"),
      inProject("b", "shop", "db"),
      inProject("c", "blog"),
    ]);
    expect(groups.map((g) => g.project)).toEqual(["blog", "shop"]);
    expect(groups.find((g) => g.project === "shop")?.total).toBe(2);
  });

  it("puts unlabelled containers in the standalone group, last", () => {
    const groups = groupContainersByProject([
      container("loose", "running"),
      inProject("a", "shop"),
    ]);
    expect(groups[groups.length - 1].id).toBe(STANDALONE_GROUP);
    expect(groups[groups.length - 1].project).toBe("");
  });

  it("treats an empty or whitespace project label as standalone", () => {
    // A label present but blank must not create a group with no name.
    const groups = groupContainersByProject([
      container("a", "running", { [COMPOSE_PROJECT_LABEL]: "" }),
      container("b", "running", { [COMPOSE_PROJECT_LABEL]: "   " }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].id).toBe(STANDALONE_GROUP);
    expect(groups[0].total).toBe(2);
  });

  it("counts running separately from total", () => {
    const groups = groupContainersByProject([
      inProject("a", "shop", "web", "running"),
      inProject("b", "shop", "db", "exited"),
    ]);
    expect(groups[0].total).toBe(2);
    expect(groups[0].running).toBe(1);
  });

  it("matches state case-insensitively", () => {
    // Docker reports "running"; some paths title-case it.
    const groups = groupContainersByProject([inProject("a", "shop", "web", "Running")]);
    expect(groups[0].running).toBe(1);
  });

  it("handles a project named the same as a service", () => {
    const groups = groupContainersByProject([inProject("a", "web", "web")]);
    expect(groups[0].project).toBe("web");
    expect(groups[0].total).toBe(1);
  });

  it("handles containers with no Labels field at all", () => {
    const bare = { Id: "x", Names: "x", State: "running" } as DockerContainer;
    const groups = groupContainersByProject([bare]);
    expect(groups[0].id).toBe(STANDALONE_GROUP);
  });

  it("returns an empty array for no containers", () => {
    expect(groupContainersByProject([])).toEqual([]);
  });

  it("keeps unusual project names intact rather than sanitising them", () => {
    const groups = groupContainersByProject([inProject("a", "my_project.v2-beta")]);
    expect(groups[0].project).toBe("my_project.v2-beta");
  });
});

describe("serviceOf", () => {
  it("returns the compose service name", () => {
    expect(serviceOf(inProject("a", "shop", "api"))).toBe("api");
  });

  it("returns an empty string when unlabelled", () => {
    expect(serviceOf(container("a", "running"))).toBe("");
  });
});
