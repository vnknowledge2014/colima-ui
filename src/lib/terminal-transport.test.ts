import { describe, it, expect } from "vitest";
import { exitHint, sessionLabel } from "./terminal-transport";

describe("terminal-transport — sessionLabel", () => {
  it("formats colima default and named profiles", () => {
    expect(sessionLabel({ kind: "colima", profile: "default" })).toBe("colima");
    expect(sessionLabel({ kind: "colima", profile: "dev" })).toBe("dev");
  });

  it("formats lima instances", () => {
    expect(sessionLabel({ kind: "lima", instance: "ubuntu" })).toBe("🐧 ubuntu");
  });

  it("formats k8sExec with and without explicit container", () => {
    expect(
      sessionLabel({ kind: "k8sExec", namespace: "default", pod: "api-1", container: "" }),
    ).toBe("api-1");
    expect(
      sessionLabel({ kind: "k8sExec", namespace: "default", pod: "api-1", container: "envoy" }),
    ).toBe("api-1/envoy");
  });

  it("formats dockerExec using label when present and container id as fallback", () => {
    expect(
      sessionLabel({ kind: "dockerExec", container: "a1b2c3d4e5f6", label: "web-redis" }),
    ).toBe("🐳 web-redis");
    expect(
      sessionLabel({ kind: "dockerExec", container: "a1b2c3d4e5f6", label: "" }),
    ).toBe("🐳 a1b2c3d4e5f6");
    expect(
      sessionLabel({ kind: "dockerExec", container: "a1b2c3d4e5f6" }),
    ).toBe("🐳 a1b2c3d4e5f6");
  });
});

describe("terminal-transport — exitHint", () => {
  it("explains distroless / FROM scratch exit codes 126 and 127 for dockerExec", () => {
    const hint126 = exitHint({ kind: "dockerExec", container: "c1", label: "app" }, 126);
    const hint127 = exitHint({ kind: "dockerExec", container: "c1", label: "app" }, 127);

    expect(hint126).toContain("/bin/sh");
    expect(hint126).toContain("distroless");
    expect(hint126).toContain("Logs");
    expect(hint127).toBe(hint126);
  });

  it("returns null for normal dockerExec exit codes", () => {
    expect(exitHint({ kind: "dockerExec", container: "c1", label: "app" }, 0)).toBeNull();
    expect(exitHint({ kind: "dockerExec", container: "c1", label: "app" }, 1)).toBeNull();
    expect(exitHint({ kind: "dockerExec", container: "c1", label: "app" }, 130)).toBeNull();
  });

  it("explains exit code 127 for k8sExec and returns null otherwise", () => {
    const k8s127 = exitHint(
      { kind: "k8sExec", namespace: "kube-system", pod: "coredns-1", container: "coredns" },
      127,
    );
    expect(k8s127).toContain("kubectl debug -it -n kube-system coredns-1");
    expect(k8s127).toContain("--target=coredns");
    expect(
      exitHint(
        { kind: "k8sExec", namespace: "kube-system", pod: "coredns-1", container: "coredns" },
        0,
      ),
    ).toBeNull();
  });

  it("returns null for colima and lima sessions even on exit code 127", () => {
    expect(exitHint({ kind: "colima", profile: "default" }, 127)).toBeNull();
    expect(exitHint({ kind: "lima", instance: "ubuntu" }, 127)).toBeNull();
  });
});
