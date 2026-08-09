export function isRunningInTauri() {
  if (typeof window === 'undefined') return false;
  return !!(window as any).__TAURI_INTERNALS__ || 
         !!(window as any).isTauri || 
         !!(window as any).__TAURI_IPC__ ||
         navigator.userAgent.includes('Tauri') ||
         navigator.userAgent.includes('tauri');
}
