import { useEffect } from "react";

type HotkeyMap = Record<string, (e: KeyboardEvent) => void>;

/**
 * Zero-dependency global hotkey hook.
 * Keys use format: "mod+k" (mod = ⌘ on Mac, Ctrl otherwise), "escape", "delete".
 */
export function useHotkeys(map: HotkeyMap) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();

      for (const [combo, fn] of Object.entries(map)) {
        const parts = combo.toLowerCase().split("+");
        const needsMod = parts.includes("mod");
        const targetKey = parts[parts.length - 1];

        if (needsMod && isMod && key === targetKey) {
          e.preventDefault();
          fn(e);
          return;
        }
        if (!needsMod && key === targetKey && !isMod) {
          // Only fire non-mod shortcuts when not typing in an input
          const tag = (e.target as HTMLElement)?.tagName;
          if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
          fn(e);
          return;
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [map]);
}
