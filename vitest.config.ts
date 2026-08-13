import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte({ compilerOptions: { dev: false } })],
  // Mirrors the `define` in `vite.config.ts`, which this config replaces rather
  // than extends. Any build-time constant the app reads has to exist here too.
  define: { __APP_VERSION__: JSON.stringify('0.0.0-test') },
  // Without the browser condition, `import ... from 'svelte'` resolves to the
  // server build, whose `mount()` throws — so component tests cannot render.
  resolve: { conditions: ['browser'] },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/setupTests.ts'],
    include: ['src/**/*.{test,spec}.{js,ts,svelte}'],
    globals: true,
  },
})
