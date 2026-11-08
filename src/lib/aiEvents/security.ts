import { EventHandler } from "./types";
import {
  securityApi,
  type Level,
  type SbomFormat,
  type SecurityAudit,
} from "../api/security";
import { falcoApi } from "../api/falco";
import { detonationApi } from "../api/detonation";

let scanSeq = 1;

function defaultScanId(p: Record<string, unknown> | undefined): string {
  const explicit = p?.scanId ?? p?.scan_id;
  if (typeof explicit === "string" && explicit.trim()) {
    return explicit.trim();
  }
  return `agent-scan-${Date.now()}-${scanSeq++}`;
}

function parseSecurityLevel(p: Record<string, unknown> | undefined): Level {
  const raw = String(p?.level ?? p?.strictness ?? "l1").trim().toLowerCase();
  if (raw === "l3" || raw === "3") return "l3";
  if (raw === "l2" || raw === "2") return "l2";
  return "l1";
}

function parseSbomFormat(p: Record<string, unknown> | undefined): SbomFormat {
  const raw = String(p?.format ?? "cyclone_dx").trim().toLowerCase();
  if (raw === "spdx" || raw === "spdx-json" || raw === "spdx_json") {
    return "spdx";
  }
  return "cyclone_dx";
}

export const securityRegistry: Record<string, EventHandler> = {
  "security-history": {
    category: "SAFE",
    description: "Get historical security scan score runs for an image digest",
    handler: async (p) => {
      const digest = String(p?.imageDigest ?? p?.image_digest ?? p?.digest ?? "");
      return JSON.stringify(await securityApi.history(digest), null, 2);
    }
  },
  "security-rules": {
    category: "SAFE",
    description: "Get the security posture rule pack (hardening, provenance, freshness rules)",
    handler: async () => JSON.stringify(await securityApi.rules(), null, 2)
  },
  "security-alternatives": {
    category: "SAFE",
    description: "Suggest hardened base image alternatives for an image reference",
    handler: async (p) => {
      const imageRef = String(p?.imageRef ?? p?.image_ref ?? p?.image ?? p?.name ?? "");
      return JSON.stringify(await securityApi.alternatives(imageRef), null, 2);
    }
  },
  "falco-state": {
    category: "SAFE",
    description: "Get Falco runtime security detector status and rule count",
    handler: async () => JSON.stringify(await falcoApi.state(), null, 2)
  },
  "falco-events": {
    category: "SAFE",
    description: "List recent Falco runtime security events",
    handler: async (p) => {
      const limit = p?.limit !== undefined ? Number(p.limit) : 200;
      return JSON.stringify(await falcoApi.events(limit), null, 2);
    }
  },
  "detonation-list": {
    category: "SAFE",
    description: "List isolated runtime behavior observation sessions",
    handler: async () => JSON.stringify(await detonationApi.list(), null, 2)
  },
  "security-scan": {
    category: "NORMAL",
    description: "Scan a container image for vulnerabilities with Trivy",
    handler: async (p) => {
      const scanId = defaultScanId(p);
      const imageRef = String(p?.imageRef ?? p?.image_ref ?? p?.image ?? "");
      const refresh = Boolean(p?.refresh);
      return JSON.stringify(await securityApi.scan(scanId, imageRef, refresh), null, 2);
    }
  },
  "security-sbom": {
    category: "NORMAL",
    description: "Export an image SBOM (CycloneDX or SPDX) to a destination directory",
    handler: async (p) => {
      const imageRef = String(p?.imageRef ?? p?.image_ref ?? p?.image ?? "");
      const destDir = String(p?.destDir ?? p?.dest_dir ?? p?.dir ?? "");
      const fileName = String(p?.fileName ?? p?.file_name ?? p?.file ?? "sbom.json");
      const format = parseSbomFormat(p);
      const overwrite = Boolean(p?.overwrite);
      return await securityApi.exportSbom(imageRef, destDir, fileName, format, overwrite);
    }
  },
  "security-audit": {
    category: "NORMAL",
    description: "Scan an image, evaluate hardening/provenance/freshness rules, and compute posture score",
    handler: async (p) => {
      const scanId = defaultScanId(p);
      const imageRef = String(p?.imageRef ?? p?.image_ref ?? p?.image ?? "");
      const level = parseSecurityLevel(p);
      const refresh = Boolean(p?.refresh);
      return JSON.stringify(await securityApi.audit(scanId, imageRef, level, refresh), null, 2);
    }
  },
  "security-triage": {
    category: "NORMAL",
    description: "Prioritize failing security posture rules by estimated score impact",
    handler: async (p) => {
      const level = parseSecurityLevel(p);
      let audit: SecurityAudit;
      if (p?.audit && typeof p.audit === "object") {
        audit = p.audit as SecurityAudit;
      } else {
        const scanId = defaultScanId(p);
        const imageRef = String(p?.imageRef ?? p?.image_ref ?? p?.image ?? "");
        const refresh = Boolean(p?.refresh);
        audit = await securityApi.audit(scanId, imageRef, level, refresh);
      }
      return JSON.stringify(await securityApi.triage(audit, level), null, 2);
    }
  },
  "detonation-start": {
    category: "DANGEROUS",
    description: "Observe an image's runtime behavior in a disposable isolated Colima instance",
    handler: async (p) => {
      const image = String(p?.image ?? p?.imageRef ?? p?.image_ref ?? "");
      const timeoutRaw = p?.timeoutSecs ?? p?.timeout_secs;
      const cmdRaw = p?.cmdOverride ?? p?.cmd_override;
      return await detonationApi.start({
        image,
        ...(timeoutRaw !== undefined ? { timeoutSecs: Number(timeoutRaw) } : {}),
        ...(typeof cmdRaw === "string" ? { cmdOverride: cmdRaw } : {}),
      });
    }
  },
  "detonation-cancel": {
    category: "NORMAL",
    description: "Cancel a running isolated behavior observation session",
    handler: async (p) => {
      const sessionId = String(p?.sessionId ?? p?.session_id ?? p?.id ?? "");
      const cancelled = await detonationApi.cancel(sessionId);
      return JSON.stringify({ cancelled, sessionId });
    }
  }
};
