import { describe, it, expect, beforeEach } from "vitest";
import {
  buildLiveUiContextSnapshot,
  getStarterPrompts,
} from "../lib/aiSuggestions";
import { setLanguage, getLanguage, t } from "../lib/i18n.svelte";
import type { ColimaInstance, DockerContainer } from "../lib/api/types";

describe("Empirical Challenger Adversarial Stress Suite", () => {
  beforeEach(() => {
    setLanguage("en");
  });

  // =========================================================================
  // 1. Live UI Context Snapshot Budgeting Stress Tests
  // =========================================================================
  describe("1. Live UI Context Snapshot Budgeting", () => {
    it("never exceeds 900 characters with 100 containers", () => {
      const containers: DockerContainer[] = [];
      for (let i = 0; i < 100; i++) {
        containers.push({
          Id: `container-${i}-${"x".repeat(30)}`,
          Names: [`/production-worker-service-${i}`],
          Image: `ghcr.io/org/repo/heavy-image-${i}:v1.2.3@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef`,
          State: i % 2 === 0 ? "running" : "exited",
          Status: i % 2 === 0 ? "Up 3 hours (healthy)" : "Exited (137) 5 minutes ago",
        } as unknown as DockerContainer);
      }

      const snapshot = buildLiveUiContextSnapshot({
        currentPage: "containers",
        language: "en",
        containers,
      });

      expect(typeof snapshot).toBe("string");
      expect(snapshot.length).toBeLessThanOrEqual(900);
      expect(snapshot).toContain("[LIVE_UI_CONTEXT]");
      expect(snapshot).toContain("Ground Truth Rule:");
    });

    it("never exceeds 900 characters with container names of 500 characters", () => {
      const containers: DockerContainer[] = [];
      for (let i = 0; i < 20; i++) {
        containers.push({
          Id: `cid-${i}-${"y".repeat(64)}`,
          Names: [`/${"very-long-container-name-".repeat(20)}-${i}`], // ~500 chars
          Image: `registry.enterprise.internal/deeply/nested/namespace/subsystem/service-image-${i}:latest`,
          State: "running",
          Status: "Up 2 weeks (unhealthy)",
        } as unknown as DockerContainer);
      }

      const snapshot = buildLiveUiContextSnapshot({
        currentPage: "containers",
        language: "en",
        containers,
      });

      expect(snapshot.length).toBeLessThanOrEqual(900);
      // Ensure name was safely truncated with ellipsis
      expect(snapshot).toContain("...");
    });

    it("handles missing/undefined properties, empty objects, and corrupt arrays robustly", () => {
      const malformedContainers = [
        {},
        null,
        undefined,
        { Id: undefined, Names: undefined, State: undefined, Status: undefined },
        { Id: null, Names: [], State: null, Status: null, Image: null },
        { Names: [""] },
        { Names: ["/"] },
        { Names: 12345 }, // invalid type
        { State: { complex: "object" } }, // invalid type
        { Image: ["not", "a", "string"] }, // invalid type
        { Id: "short-id", Status: "running" },
      ] as unknown as DockerContainer[];

      const snapshot = buildLiveUiContextSnapshot({
        currentPage: undefined,
        language: undefined,
        containers: malformedContainers,
        instances: undefined,
        k8sStatus: undefined,
        activeContext: undefined,
      });

      expect(snapshot.length).toBeLessThanOrEqual(900);
      expect(snapshot).toContain("[LIVE_UI_CONTEXT]");
      expect(snapshot).toContain("Current Page: dashboard");
      expect(snapshot).toContain("Language: en");
    });

    it("fuzzing: 500 randomized extreme scenarios never breach 900 character ceiling", () => {
      const states = ["running", "exited", "dead", "paused", "restarting", "", undefined, "up", "unknown"];
      const statuses = ["Up 10 hours", "Exited (0)", "Exited (1)", "CrashLoop", "", null, undefined];

      for (let run = 0; run < 500; run++) {
        const containerCount = Math.floor(Math.random() * 80);
        const containers: DockerContainer[] = [];

        for (let j = 0; j < containerCount; j++) {
          const nameLen = Math.floor(Math.random() * 600);
          const imgLen = Math.floor(Math.random() * 300);
          containers.push({
            Id: "id-" + "x".repeat(Math.floor(Math.random() * 80)),
            Names: [nameLen > 0 ? "c_" + "a".repeat(nameLen) : ""],
            Image: imgLen > 0 ? "img_" + "b".repeat(imgLen) : undefined,
            State: states[Math.floor(Math.random() * states.length)],
            Status: statuses[Math.floor(Math.random() * statuses.length)],
          } as unknown as DockerContainer);
        }

        const vmCount = Math.floor(Math.random() * 30);
        const instances: ColimaInstance[] = [];
        for (let k = 0; k < vmCount; k++) {
          instances.push({
            name: "vm-" + "v".repeat(Math.floor(Math.random() * 200)),
            status: Math.random() > 0.5 ? "Running" : "Stopped",
            cpus: Math.floor(Math.random() * 64),
            memory: Math.floor(Math.random() * 128),
            disk: 100,
            runtime: "docker",
            arch: "aarch64",
            address: "127.0.0.1",
            kubernetes: false,
          });
        }

        const snapshot = buildLiveUiContextSnapshot({
          currentPage: "page-" + "p".repeat(Math.floor(Math.random() * 100)),
          language: "lang-" + "l".repeat(Math.floor(Math.random() * 50)),
          containers,
          instances,
          k8sStatus: Math.random() > 0.5 ? { connected: true, pods: 999999, namespaces: 88888 } : undefined,
          activeContext: "ctx-" + "c".repeat(Math.floor(Math.random() * 200)),
        });

        expect(snapshot.length).toBeLessThanOrEqual(900);
      }
    });

    it("verifies max 6 containers are detailed in the snapshot even if 1,000 exist", () => {
      const containers: DockerContainer[] = [];
      for (let i = 0; i < 1000; i++) {
        containers.push({
          Id: `cid-${i}`,
          Names: [`/worker-${i}`],
          State: "running",
          Status: "Up 2 hours",
          Image: "alpine:latest",
        } as unknown as DockerContainer);
      }

      const snapshot = buildLiveUiContextSnapshot({ containers });
      const containerDetailMatches = snapshot.match(/- worker-\d+/g) || [];
      expect(containerDetailMatches.length).toBeLessThanOrEqual(6);
      expect(snapshot.length).toBeLessThanOrEqual(900);
    });
  });

  // =========================================================================
  // 2. i18n & Language Isolation Stress Tests
  // =========================================================================
  describe("2. i18n & Language Isolation", () => {
    const supportedLanguages = ["vi", "ja", "zh", "en"] as const;

    it("rapidly switches languages ('vi' -> 'ja' -> 'zh' -> 'en') across 100 cycles without state leakage", () => {
      const testKey = "ai.starter.containers_prune_title";
      // Expected translations for each language
      const expectedTranslations: Record<string, string> = {
        en: "Clean stopped containers",
        vi: "Dọn dẹp container đã dừng",
        ja: "停止したコンテナをクリーンアップ",
        zh: "清理已停止的容器",
      };

      for (let cycle = 0; cycle < 100; cycle++) {
        for (const lang of supportedLanguages) {
          setLanguage(lang);
          expect(getLanguage()).toBe(lang);

          const translation = t(testKey);
          expect(translation).toBe(expectedTranslations[lang]);
          // Verify no cross-language leakage
          for (const otherLang of supportedLanguages) {
            if (otherLang !== lang) {
              expect(translation).not.toBe(expectedTranslations[otherLang]);
            }
          }
        }
      }
    });

    it("prevents parameter interpolation leak between consecutive calls in different languages", () => {
      const templateKey = "ai.starter.warning_instance_title";

      // 1. Call Vietnamese with instance "alpha"
      setLanguage("vi");
      const viResult = t(templateKey, { default: "Investigate {name}", name: "alpha" });
      expect(viResult).toContain("alpha");

      // 2. Switch to Japanese with instance "beta"
      setLanguage("ja");
      const jaResult = t(templateKey, { default: "Investigate {name}", name: "beta" });
      expect(jaResult).toContain("beta");
      expect(jaResult).not.toContain("alpha");

      // 3. Switch to Chinese with instance "gamma"
      setLanguage("zh");
      const zhResult = t(templateKey, { default: "Investigate {name}", name: "gamma" });
      expect(zhResult).toContain("gamma");
      expect(zhResult).not.toContain("beta");
      expect(zhResult).not.toContain("alpha");

      // 4. Switch to English with instance "delta"
      setLanguage("en");
      const enResult = t(templateKey, { default: "Investigate {name}", name: "delta" });
      expect(enResult).toContain("delta");
      expect(enResult).not.toContain("gamma");
    });

    it("starter prompts and fallback suggestions update synchronously upon language switch", () => {
      const context = {
        currentPage: "dashboard",
        instances: [{ name: "prod-vm", status: "Stopped", cpus: 2, memory: 4, disk: 50, runtime: "docker", arch: "aarch64", address: "127.0.0.1", kubernetes: false }],
      };

      // English
      setLanguage("en");
      const enStarters = getStarterPrompts(context);
      expect(enStarters[0].title).toBe("Investigate stopped instance prod-vm");

      // Vietnamese
      setLanguage("vi");
      const viStarters = getStarterPrompts(context);
      expect(viStarters[0].title).toContain("prod-vm");
      expect(viStarters[0].title).not.toBe("Investigate stopped instance prod-vm");

      // Japanese
      setLanguage("ja");
      const jaStarters = getStarterPrompts(context);
      expect(jaStarters[0].title).toContain("prod-vm");
      expect(jaStarters[0].title).not.toBe(enStarters[0].title);
      expect(jaStarters[0].title).not.toBe(viStarters[0].title);

      // Chinese
      setLanguage("zh");
      const zhStarters = getStarterPrompts(context);
      expect(zhStarters[0].title).toContain("prod-vm");
      expect(zhStarters[0].title).not.toBe(enStarters[0].title);
      expect(zhStarters[0].title).not.toBe(viStarters[0].title);
      expect(zhStarters[0].title).not.toBe(jaStarters[0].title);
    });

    it("falls back cleanly to default or key if language is unknown, without throwing", () => {
      setLanguage("invalid_lang_code");
      expect(getLanguage()).toBe("en"); // fell back to en

      const res = t("non.existent.key.adversarial", { default: "Safe Fallback Value" });
      expect(res).toBe("Safe Fallback Value");
    });
  });
});
