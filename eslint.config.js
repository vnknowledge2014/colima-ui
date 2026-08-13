// @ts-check
/**
 * ESLint flat config — enforced by the pre-push hook and CI.
 *
 * Focus: dead code, unused variables/imports, unreachable code, and
 * error-prone patterns. Everything here runs with `--max-warnings=0`,
 * so a single warning fails the push.
 *
 * Agents MUST run `pnpm lint` before committing. See AGENTS.md.
 */
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';

export default tseslint.config(
  // ------------------------------------------------------------------
  // Ignore everything that is generated, vendored, or not first-party.
  // ------------------------------------------------------------------
  {
    ignores: [
      '**/node_modules/**',
      '**/dist/**',
      '**/src-tauri/**',
      '**/target/**',
      'public/**',
      'docs/**',
      'plans/**',
      'supabase/**',
      'node-compile-cache/**',
      'tsx-501/**',
      '.sisyphus/**',
      '.agents/**',
      '.claude/**',
      'external_skill/**',
      'coverage/**',
    ],
  },

  // ------------------------------------------------------------------
  // Baseline: recommended JS + recommended (non-type-checked) TS.
  // ------------------------------------------------------------------
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],

  // ------------------------------------------------------------------
  // Shared language options for all first-party files.
  // ------------------------------------------------------------------
  {
    files: ['**/*.{js,mjs,cjs,ts,svelte}'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
      parserOptions: {
        extraFileExtensions: ['.svelte'],
        parser: tseslint.parser,
      },
    },
  },

  // ------------------------------------------------------------------
  // Dead-code + error-prone rules (the point of this config).
  // ------------------------------------------------------------------
  {
    files: ['**/*.{js,mjs,cjs,ts,svelte}'],
    rules: {
      // --- Unused code -------------------------------------------------
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          args: 'after-used',
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
          destructuredArrayIgnorePattern: '^_',
          ignoreRestSiblings: true,
        },
      ],
      '@typescript-eslint/no-unused-expressions': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      'no-unused-private-class-members': 'error',

      // --- Unreachable / dead code ------------------------------------
      'no-unreachable': 'error',
      'no-unreachable-loop': 'error',
      'no-constant-condition': ['error', { checkLoops: 'allExceptWhileTrue' }],
      'no-constant-binary-expression': 'error',
      'no-self-compare': 'error',

      // --- Empty / no-op code -----------------------------------------
      'no-empty': ['error', { allowEmptyCatch: false }],
      'no-useless-return': 'error',
      'no-useless-escape': 'error',

      // --- Error-prone patterns ----------------------------------------
      'no-constructor-return': 'error',
      'no-promise-executor-return': 'error',
      'no-throw-literal': 'error',
      'no-template-curly-in-string': 'error',

      // --- Svelte ------------------------------------------------------
      // Compile ERRORS always fail; compiler WARNINGS here are all a11y
      // (see svelte.config.js warningFilter) — a separate concern, ignored.
      'svelte/valid-compile': ['error', { ignoreWarnings: true }],
      'svelte/no-unused-svelte-ignore': 'error',
      // `{@html}` renders static internal icon constants (Icons.*, KIND_ICON)
      // and explicitly-rendered markdown — not raw user HTML. XSS-policy rule,
      // orthogonal to the dead-code gate, so it stays off.
      'svelte/no-at-html-tags': 'off',
      // Preference-level reactivity hints (SvelteDate etc.) — not dead-code.
      'svelte/prefer-svelte-reactivity': 'off',
    },
  },

  // ------------------------------------------------------------------
  // Svelte-specific tuning.
  // ------------------------------------------------------------------
  {
    files: ['**/*.svelte'],
    rules: {
      // Accessibility rules are a separate concern from dead-code
      // enforcement. Keep them from failing the lint gate.
      'svelte/a11y-click-events-have-key-events': 'off',
      'svelte/a11y-no-static-element-interactions': 'off',
      'svelte/a11y-no-noninteractive-element-interactions': 'off',
      'svelte/a11y-no-onchange': 'off',
      'svelte/a11y-mouse-events-have-key-events': 'off',
      'svelte/a11y-aria-attributes': 'off',
      'svelte/a11y-role-has-required-aria-props': 'off',
      'svelte/a11y-role-supports-aria-props': 'off',
      'svelte/a11y-no-redundant-roles': 'off',
      'svelte/a11y-img-redundant-alt': 'off',
      'svelte/a11y-label-has-associated-control': 'off',
      'svelte/a11y-media-has-caption': 'off',
      'svelte/a11y-no-distracting-elements': 'off',
      'svelte/a11y-no-noninteractive-tabindex': 'off',
      'svelte/a11y-incorrect-aria-attribute-type': 'off',
      'svelte/a11y-autofocus': 'off',
      'svelte/a11y-misplaced-role': 'off',
      'svelte/a11y-missing-attribute': 'off',
      'svelte/a11y-missing-content': 'off',
      'svelte/a11y-no-abstract-role': 'off',
      'svelte/a11y-no-aria-hidden-on-focusable': 'off',
      'svelte/a11y-no-interactive-element-to-noninteractive-role': 'off',
      'svelte/a11y-no-noninteractive-element-to-interactive-role': 'off',
      'svelte/a11y-positive-tabindex': 'off',
    },
  },
);
