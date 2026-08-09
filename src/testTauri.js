export const isTauri = () => navigator.userAgent.includes('Tauri') || !!window.__TAURI_INTERNALS__ || !!window.__TAURI_IPC__ || !!window.isTauri || !!window.__TAURI__;
