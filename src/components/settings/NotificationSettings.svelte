<script lang="ts">
  /**
   * The one thing worth configuring about notifications: whether they leave the
   * app.
   *
   * The in-app centre is not optional — it is where a background transfer is
   * cancelled from, so switching it off would strand transfers. This toggle only
   * covers the operating system's notification centre, which is the part that
   * shows up while ColimaUI is not on screen.
   */
  import { isRunningInTauri } from "../../lib/env";
  import { getAppSetting, setAppSetting } from "../../lib/settingsStore.svelte";
  import { OS_NOTIFY_SETTING } from "../../lib/osNotify";
  import { ANNOUNCEMENTS_ENABLED_KEY } from "../../lib/announcements";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "./SettingsSection.svelte";

  const inApp = isRunningInTauri();

  /**
   * Read through to the settings store rather than snapshotted at mount.
   *
   * Settings load asynchronously, so a component mounted first would show the
   * default "on" for a stored "off" — and the next click would then write back a
   * value the user was never shown.
   */
  let enabled = $derived(getAppSetting(OS_NOTIFY_SETTING, "true") !== "false");

  /** Same read-through reasoning as `enabled` above. */
  let announcements = $derived(
    getAppSetting(ANNOUNCEMENTS_ENABLED_KEY, "true") !== "false"
  );

  function toggle(e: Event) {
    // Written immediately rather than behind a Save: there is one value and its
    // effect is instant, so a pending state would be the only thing to get wrong.
    const checked = (e.currentTarget as HTMLInputElement).checked;
    void setAppSetting(OS_NOTIFY_SETTING, checked ? "true" : "false");
  }

  function toggleAnnouncements(e: Event) {
    const checked = (e.currentTarget as HTMLInputElement).checked;
    void setAppSetting(ANNOUNCEMENTS_ENABLED_KEY, checked ? "true" : "false");
  }
</script>

<SettingsSection
  title={t("notifications.settings_title", { default: "Notifications" })}
  icon="Bell"
  description={t("notifications.settings_description", {
    default:
      "Transfers keep running in the background. ColimaUI can tell your operating system when one finishes, so you find out without switching back.",
  })}
>
  {#if inApp}
    <label
      style="font-size: var(--text-sm); font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 8px;"
    >
      <input type="checkbox" class="checkbox" checked={enabled} onchange={toggle} />
      <span>
        {t("notifications.os_enabled", {
          default: "Notify me when a transfer finishes",
        })}
      </span>
    </label>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 12px;">
      {t("notifications.os_hint", {
        default:
          "Only while the ColimaUI window is in the background, and only that a transfer ended — never its file paths or error output.",
      })}
    </p>
  {:else}
    <p style="font-size: var(--text-sm); color: var(--text-secondary);">
      {t("notifications.os_desktop_only", {
        default:
          "System notifications need the desktop app. In the browser, the bell in the sidebar shows the same thing.",
      })}
    </p>
  {/if}

  <!-- Not behind `inApp`: announcements are fetched by the backend either way,
       so the switch has to exist in browser mode too. -->
  <hr
    style="border: none; border-top: 1px solid var(--border-primary); margin: 16px 0;"
  />
  <label
    style="font-size: var(--text-sm); font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 8px;"
  >
    <input
      type="checkbox"
      class="checkbox"
      checked={announcements}
      onchange={toggleAnnouncements}
    />
    <span>
      {t("notifications.announcements_enabled", {
        default: "Show release notes and security advisories",
      })}
    </span>
  </label>
  <p style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 12px;">
    {t("notifications.announcements_hint", {
      default:
        "ColimaUI checks a file on GitHub every few hours. Nothing about you is sent — but the request itself shows GitHub your IP address and the time. Switching this off stops the request entirely.",
    })}
  </p>
</SettingsSection>
