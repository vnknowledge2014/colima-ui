// Svelte compiler config — read by svelte-eslint-parser (eslint), the Vite
// plugin, and svelte-check. Keeps compiler-level a11y warnings from failing
// the dead-code lint gate (they are a separate concern, not enforced here).
export default {
  compilerOptions: {
    warningFilter: (warning) => !warning.code.startsWith('a11y_'),
  },
};
