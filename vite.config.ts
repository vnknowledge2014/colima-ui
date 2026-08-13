import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { createRequire } from "node:module";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// The app's own version, so the frontend can tell whether a version-targeted
// announcement is meant for this build. Read from `package.json`, which is
// already the version `tauri.conf.json` ships.
const { version: appVersion } = createRequire(import.meta.url)("./package.json");

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte()],
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (id.includes('node_modules/svelte/')) {
            return 'vendor-svelte';
          }
          if (id.includes('node_modules/@xterm/')) {
            return 'vendor-xterm';
          }
          if (id.includes('node_modules/@tauri-apps/')) {
            return 'vendor-tauri';
          }
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
