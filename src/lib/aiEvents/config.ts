import { isRunningInTauri } from "../env";
import { runSandboxed } from "./shell";
import { EventHandler } from "./types";
import { modelsApi, settingsApi } from "../api";
import { call } from "../api/client";
import { getActiveProvider } from "../aiProviderConfig";
import { setAppSetting } from "../settingsStore.svelte";
import { uiState, VALID_PAGES, isValidPage } from "../../store.svelte";

export const configRegistry: Record<string, EventHandler> = {
  "model-list": {
    category: "SAFE",
    description: "List locally pulled AI models",
    handler: async (p) => JSON.stringify(await modelsApi.listModels(p.profile || "default"), null, 2)
  },
  "model-pull": {
    category: "NORMAL",
    description: "Pull an AI model",
    handler: async (p) => {
      await modelsApi.pullModel(p.profile || "default", p.name);
      return `Model '${p.name}' pulled.`;
    }
  },
  "model-delete": {
    category: "DANGEROUS",
    description: "Delete an AI model",
    handler: async (p) => {
      await modelsApi.deleteModel(p.profile || "default", p.name);
      return `Model '${p.name}' deleted.`;
    }
  },
  "model-serve": {
    category: "NORMAL",
    description: "Serve an AI model on a port",
    handler: async (p) => {
      await modelsApi.serveModel(p.profile || "default", p.name, p.port);
      return `Model '${p.name}' serving on port ${p.port}.`;
    }
  },

  "navigate": {
    category: "SAFE",
    description: "Navigate to a specific page in the application",
    handler: async (p) => {
      const targetPage = typeof p?.page === "string" ? p.page.trim().toLowerCase() : "";
      if (!isValidPage(targetPage)) {
        return `Error: Invalid page '${p?.page ?? ""}'. Valid pages: ${VALID_PAGES.join(", ")}`;
      }
      uiState.currentPage = targetPage;
      return `Navigated to ${targetPage} tab.`;
    }
  },

  "ai-config-status": {
    category: "SAFE",
    description: "Get all application settings and configuration including AI provider/model",
    handler: async () => {
      const settings = await settingsApi.getAll();
      const redacted: Record<string, string> = {};
      for (const [key, val] of Object.entries(settings ?? {})) {
        if (/api_key|token|secret|password/i.test(key)) {
          redacted[key] = val ? "[REDACTED]" : "";
        } else {
          redacted[key] = val;
        }
      }
      return JSON.stringify(redacted, null, 2);
    }
  },
  "ai-update-config": {
    category: "NORMAL",
    description: "Update application settings and configuration",
    handler: async (p) => {
      const updates = p ?? {};
      if (updates.provider) await setAppSetting("ai_provider", String(updates.provider));
      // Written to the active mirror and to the provider's own slot, so the
      // settings form does not overwrite these the next time it loads.
      const provider = updates.provider || getActiveProvider();
      const scoped: [string, string, string | undefined][] = [
        ["ai_model", `ai_model.${provider}`, updates.model],
        ["ai_endpoint", `ai_endpoint.${provider}`, updates.endpoint],
        ["ai_api_key", `ai_api_key.${provider}`, updates.api_key],
      ];
      for (const [activeKey, providerKey, value] of scoped) {
        if (!value) continue;
        await setAppSetting(activeKey, value);
        await setAppSetting(providerKey, value);
      }

      // Update other arbitrary settings
      if (updates.settings) {
        const arbitrary = typeof updates.settings === "string"
          ? (JSON.parse(updates.settings) as Record<string, unknown>)
          : (updates.settings as Record<string, unknown>);
        for (const [key, value] of Object.entries(arbitrary)) {
          await setAppSetting(key, String(value));
        }
      }

      return `Configuration updated. Please notify the user that they may need to restart the application or refresh the UI for some changes to apply.`;
    }
  },
  "list-presets": {
    category: "SAFE",
    description: "List all saved custom presets",
    handler: async () => {
      const presets = await call<Record<string, string>>("get_all_presets", undefined, "GET", "/api/presets");
      return JSON.stringify(presets, null, 2);
    }
  },
  "save-preset": {
    category: "NORMAL",
    description: "Save a custom instance preset",
    handler: async (p) => {
      if (!p?.id) return "Error: Missing preset id";
      const configJson = JSON.stringify(p);
      await call<string>(
        "save_preset",
        { id: p.id, configJson },
        "POST",
        "/api/presets",
        undefined,
        { id: p.id, config_json: configJson, configJson }
      );
      return `Preset '${p.id}' saved successfully.`;
    }
  },
  "delete-preset": {
    category: "DANGEROUS",
    description: "Delete a custom instance preset",
    handler: async (p) => {
      if (!p?.id) return "Error: Missing preset id";
      await call<string>(
        "delete_preset",
        { id: p.id },
        "POST",
        "/api/presets/delete",
        undefined,
        { id: p.id }
      );
      return `Preset '${p.id}' deleted successfully.`;
    }
  },

  "cli-exec": {
    category: "NORMAL",
    description: "Execute a command through the App CLI gateway",
    handler: async (p) => {
      if (isRunningInTauri()) {
        try {
          return await runSandboxed(p.command, p.args || []);
        } catch (e) {
          // Includes the sandbox's own refusals — a `banned:` reason for
          // command chaining, or an argument the executor cannot represent.
          return `Error executing command: ${e instanceof Error ? e.message : String(e)}`;
        }
      }
      return `[SIMULATED] Executed ${p.command} ${p.args?.join(" ")}`;
    }
  }
};
