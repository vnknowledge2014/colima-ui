export function isRunningInTauri() {
  if (typeof window === 'undefined') return false;
  const w = window as Window & { __TAURI_INTERNALS__?: unknown; isTauri?: unknown; __TAURI_IPC__?: unknown };
  return !!(w.__TAURI_INTERNALS__) ||
         !!(w.isTauri) ||
         !!(w.__TAURI_IPC__) ||
         navigator.userAgent.includes('Tauri') ||
         navigator.userAgent.includes('tauri');
}
