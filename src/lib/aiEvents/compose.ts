import { runSandboxed } from "./shell";
import { EventHandler } from "./types";
import { composeApi } from "../api";

export const composeRegistry: Record<string, EventHandler> = {
  "compose-list": {
    category: "SAFE",
    description: "List docker compose projects",
    handler: async () => JSON.stringify(await composeApi.list(), null, 2)
  },
  "compose-build": {
    category: "NORMAL",
    description: "Build or rebuild services in a compose project",
    handler: async (p) => {
      try {
        const args = ["compose"];
        if (p?.file) args.push("-f", String(p.file));
        if (p?.dir) args.push("--project-directory", String(p.dir));
        args.push("build");
        return await runSandboxed("docker", args);
      } catch (e) {
        return `Error building compose project: ${e instanceof Error ? e.message : String(e)}`;
      }
    }
  },
  "compose-pull": {
    category: "NORMAL",
    description: "Pull images associated with a compose project",
    handler: async (p) => {
      try {
        const args = ["compose"];
        if (p?.file) args.push("-f", String(p.file));
        if (p?.dir) args.push("--project-directory", String(p.dir));
        args.push("pull");
        return await runSandboxed("docker", args);
      } catch (e) {
        return `Error pulling compose images: ${e instanceof Error ? e.message : String(e)}`;
      }
    }
  },
  "compose-up": {
    category: "NORMAL",
    description: "Docker compose up",
    handler: async (p) => {
      const dir = p?.projectDir || p?.project_dir || p?.dir || p?.file || "";
      const detach = p?.detach !== false;
      await composeApi.up(dir, detach);
      return `Compose project in '${dir}' started.`;
    }
  },
  "compose-start": {
    category: "NORMAL",
    description: "Start existing compose containers",
    handler: async (p) => {
      const project = p?.projectName || p?.project_name || p?.name || p?.dir;
      await composeApi.start(project);
      return `Compose project in '${project}' started.`;
    }
  },
  "compose-stop": {
    category: "NORMAL",
    description: "Stop running compose containers without removing them",
    handler: async (p) => {
      const project = p?.projectName || p?.project_name || p?.name || p?.dir;
      await composeApi.stop(project);
      return `Compose project in '${project}' stopped.`;
    }
  },
  "compose-down": {
    category: "NORMAL",
    description: "Stop and remove compose containers",
    handler: async (p) => {
      await composeApi.down(p.dir || p.projectName); // use projectName as dir was removed from api
      return `Compose project in '${p.dir || p.projectName}' stopped.`;
    }
  },
  "compose-restart": {
    category: "NORMAL",
    description: "Restart compose containers",
    handler: async (p) => {
      await composeApi.restart(p.dir || p.projectName);
      return `Compose project in '${p.dir || p.projectName}' restarted.`;
    }
  },
  "compose-ps": {
    category: "SAFE",
    description: "List containers belonging to a compose project",
    handler: async (p) => {
      const project = p?.projectName || p?.project_name || p?.dir || p?.name;
      return await composeApi.ps(project);
    }
  },
  "compose-logs": {
    category: "SAFE",
    description: "Get logs from compose containers",
    handler: async (p) => {
      const logs = await composeApi.logs(p.dir || p.projectName, p.lines || 100);
      return logs;
    }
  },
  "compose-diagnose": {
    category: "SAFE",
    description: "Diagnose a Compose file for validation errors, KB solutions, and triage",
    handler: async (p) => {
      const filePath = p?.filePath || p?.file_path || p?.path || p?.file;
      return JSON.stringify(await composeApi.diagnose(filePath), null, 2);
    }
  },
  "compose-autofix-history": {
    category: "SAFE",
    description: "List applied compose autofix records",
    handler: async () => JSON.stringify(await composeApi.autofixHistory(), null, 2)
  },
  "compose-autofix-propose": {
    category: "NORMAL",
    description: "Propose autofix patches for a Compose file",
    handler: async (p) => {
      const filePath = p?.filePath || p?.file_path || p?.path || p?.file;
      return JSON.stringify(await composeApi.autofixPropose(filePath), null, 2);
    }
  },
  "compose-autofix-apply": {
    category: "NORMAL",
    description: "Apply an autofix patch to a Compose file",
    handler: async (p) => {
      const filePath = String(p?.filePath || p?.file_path || p?.path || p?.file || "");
      const patchObj = p?.patch && typeof p.patch === "object" ? p.patch : p;
      const newContent = String(patchObj?.newContent ?? patchObj?.new_content ?? patchObj?.content ?? "");
      const explanation = String(patchObj?.explanation ?? "Applied by agent");
      const rawRemovals = patchObj?.declaredRemovals ?? patchObj?.declared_removals;
      const declaredRemovals = Array.isArray(rawRemovals) ? rawRemovals.map(String) : [];
      const rec = await composeApi.autofixApply(filePath, {
        id: String(patchObj?.id ?? "agent"),
        unified_diff: String(patchObj?.unified_diff ?? ""),
        new_content: newContent,
        explanation,
        source: "llm",
        confidence: 1,
        comments_preserved: true,
        declared_removals: declaredRemovals,
        category: String(patchObj?.category ?? ""),
      });
      return JSON.stringify(rec, null, 2);
    }
  },
  "compose-autofix-undo": {
    category: "NORMAL",
    description: "Undo a previously applied compose autofix",
    handler: async (p) => {
      const fixId = p?.fixId || p?.fix_id || p?.id;
      return JSON.stringify(await composeApi.autofixUndo(fixId), null, 2);
    }
  }
};
