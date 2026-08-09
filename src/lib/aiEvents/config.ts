// @ts-nocheck
import { EventHandler } from "./types";
import { 
  colimaApi, dockerApi, volumesApi, networksApi, sysMethods, 
  composeApi, modelsApi, k8sApi, kindApi, limaApi
} from "../api";

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
      window.dispatchEvent(new CustomEvent('colima-navigate', { detail: p.page }));
      return `Navigated to ${p.page} tab.`;
    }
  },

  "ai-config-status": {
    category: "SAFE",
    description: "Get all application settings and configuration including AI provider/model",
    handler: async () => {
      if (!((window as any).__TAURI_INTERNALS__ || (window as any).isTauri)) return "[SIMULATED] Returning all settings...";
      const { invoke } = await import("@tauri-apps/api/core");
      const settings = await invoke("get_all_settings");
      return JSON.stringify(settings, null, 2);
    }
  },
  "ai-update-config": {
    category: "NORMAL",
    description: "Update application settings and configuration",
    handler: async (p) => {
      if (!((window as any).__TAURI_INTERNALS__ || (window as any).isTauri)) return "[SIMULATED] Config updated.";
      const { invoke } = await import("@tauri-apps/api/core");
      const updates = p;
      if (updates.provider) await invoke("set_setting", { key: "ai_provider", value: updates.provider });
      if (updates.model) await invoke("set_setting", { key: "ai_model", value: updates.model });
      if (updates.endpoint) await invoke("set_setting", { key: "ai_endpoint", value: updates.endpoint });
      if (updates.api_key) await invoke("set_setting", { key: "ai_api_key", value: updates.api_key });
      
      // Update other arbitrary settings
      if (updates.settings) {
        const arbitrary = typeof updates.settings === "string" ? JSON.parse(updates.settings) : updates.settings;
        for (const [key, value] of Object.entries(arbitrary)) {
          await invoke("set_setting", { key, value: String(value) });
        }
      }
      
      return `Configuration updated. Please notify the user that they may need to restart the application or refresh the UI for some changes to apply.`;
    }
  },
  "list-presets": {
    category: "SAFE",
    description: "List all saved custom presets",
    handler: async () => {
      if (!((window as any).__TAURI_INTERNALS__ || (window as any).isTauri)) return "[SIMULATED] Returning presets...";
      const { invoke } = await import("@tauri-apps/api/core");
      const presets = await invoke("get_all_presets");
      return JSON.stringify(presets, null, 2);
    }
  },
  "save-preset": {
    category: "NORMAL",
    description: "Save a custom instance preset",
    handler: async (p) => {
      if (!((window as any).__TAURI_INTERNALS__ || (window as any).isTauri)) return "[SIMULATED] Preset saved.";
      const { invoke } = await import("@tauri-apps/api/core");
      if (!p.id) return "Error: Missing preset id";
      await invoke("save_preset", { id: p.id, configJson: JSON.stringify(p) });
      return `Preset '${p.id}' saved successfully.`;
    }
  },
  "delete-preset": {
    category: "DANGEROUS",
    description: "Delete a custom instance preset",
    handler: async (p) => {
      if (!((window as any).__TAURI_INTERNALS__ || (window as any).isTauri)) return "[SIMULATED] Preset deleted.";
      const { invoke } = await import("@tauri-apps/api/core");
      if (!p.id) return "Error: Missing preset id";
      await invoke("delete_preset", { id: p.id });
      return `Preset '${p.id}' deleted successfully.`;
    }
  },

  "cli-exec": {
    category: "NORMAL",
    description: "Execute a command through the App CLI gateway",
    handler: async (p) => {
      if ((window as any).__TAURI_INTERNALS__ || (window as any).isTauri) {
        const { invoke } = await import("@tauri-apps/api/core");
        try {
          const result = await invoke("execute_shell", { command: p.command, args: p.args || [] });
          return String(result);
        } catch (e: any) {
          return `Error executing command: ${e.message || e}`;
        }
      }
      return `[SIMULATED] Executed ${p.command} ${p.args?.join(" ")}`;
    }
  }
};
