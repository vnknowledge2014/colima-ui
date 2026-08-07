import { settingsApi } from "./api";
import { setLanguage } from "./i18n.svelte";

export const appState = $state({ isSettingsLoaded: false });
export const settingsState = $state<{ [key: string]: string }>({});

export async function loadAllSettings() {
  try {
    const backendSettings = await settingsApi.getAll();
    
    // Migration logic: If backend is empty but localStorage has data, migrate it!
    const keysToMigrate = [
      "ai_provider", "ai_model", "ai_api_key", "ai_endpoint", "ai_searxng_instances",
      "ai_diag_content_mode", "ai_diag_max_page_size", "ai_diag_auto_trigger",
      "colimaui_auto_pause", "colimaui_auto_pause_mins", "colimaui_setup_complete",
      "colimaui_tour_complete", "ai_panel_width", "ColimaCustomProfiles"
    ];

    let didMigrate = false;
    for (const key of keysToMigrate) {
      if (backendSettings[key] === undefined) {
        const localVal = localStorage.getItem(key);
        if (localVal !== null) {
          backendSettings[key] = localVal;
          await settingsApi.set(key, localVal);
          didMigrate = true;
        }
      }
    }

    if (didMigrate) {
      console.log("Migrated settings from localStorage to SQLite.");
    }

    Object.assign(settingsState, backendSettings);
    
    // Set language from settings, fallback to en
    const lang = backendSettings["app.language"] || "en";
    setLanguage(lang);
    
    appState.isSettingsLoaded = true;
  } catch (e) {
    console.error("Failed to load settings from DB:", e);
  }
}

export async function setAppSetting(key: string, value: string) {
  settingsState[key] = value;
  
  if (key === "app.language") {
    setLanguage(value);
  }
  
  try {
    await settingsApi.set(key, value);
  } catch (e) {
    console.error(`Failed to save setting ${key}:`, e);
  }
}

export function getAppSetting(key: string, defaultValue: string = ""): string {
  return settingsState[key] !== undefined ? settingsState[key] : defaultValue;
}
