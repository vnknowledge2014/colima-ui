# AGENTS.md — Quality Gates & Lint Rules

> Every agent working in this repo MUST follow these rules. They are enforced
> automatically by the pre-push hook and CI. Violating them blocks the push.

## Non-negotiable: run the gates before commit/push

| Gate | Command | Catches |
|---|---|---|
| ESLint | `pnpm lint` | dead code, unused vars/imports, error-prone patterns, `any` |
| TypeScript | `pnpm typecheck` | type errors, unused imports/locals |
| Svelte | `pnpm check` | Svelte 5 runes/template errors |
| Rust clippy | `pnpm lint:rust` | dead code, unused imports, clippy warnings |

- Run ALL four after any change. Fix every error/warning — never silence one
  to "get past the gate".
- The pre-push hook runs these automatically. `git push --no-verify` and
  `SKIP_LINT_GATES=1` are emergency-only escapes and require explicit user approval.
- CI mirrors the same gates, so a skipped local gate fails the PR.

## TypeScript / Svelte rules (enforced in `eslint.config.js`)

1. **No unused code.** No unused variables, parameters, imports, private class
   members, or unused expressions. Prefix intentionally-ignored params/vars
   with `_` (e.g. `_event`, `_unused`). Remove the import — don't comment it out.
2. **No `any`.** `@typescript-eslint/no-explicit-any` is an error. Use a real
   type, `unknown` + narrowing, or a generic. Only if a site is genuinely
   impossible to type may you add `// eslint-disable-next-line
   @typescript-eslint/no-explicit-any -- <reason>` with a justification.
3. **No unreachable/dead code.** `no-unreachable`, `no-unreachable-loop`,
   `no-constant-condition`, `no-constant-binary-expression`, `no-self-compare`.
4. **No no-op code.** Empty blocks (`no-empty`), useless returns/escapes.
5. **No error-prone patterns.** Throwing non-`Error` literals, promises with a
   return in the executor, template literals in non-template strings
   (`no-template-curly-in-string`), constructor returns.
6. **Svelte.** Components must compile (`svelte/valid-compile`); no unused
   `<!-- svelte-ignore -->` comments.

## Rust rules (enforced in `src-tauri/Cargo.toml` + `pnpm lint:rust`)

1. **No dead code.** `dead_code`, `unused`, `unused_imports`,
   `unused_variables`, `unused_mut`, `unused_assignments`, `unused_must_use`
   are warnings → errors under `-D warnings`. Remove the code or mark it
   `#[allow(dead_code)]` with a reason when it is genuinely kept for later.
2. **No unreachable code/patterns.**
3. **Clippy `all` + targeted lints** (`unused_async`, `needless_return`,
   `redundant_closure`, `useless_conversion`, `let_and_return`,
   `redundant_pattern_matching`, `map_unwrap_or`, `collapsible_if`,
   `needless_borrow`) are warnings → errors under `-D warnings`. Prefer the
   idiomatic form clippy suggests.
4. Do not use `#[allow(...)]`/`#[expect(...)]` as a way to bypass the gate.
   `cargo clippy --all-targets --all-features -- -D warnings` must pass.

## Test files

Test files are linted too. Dead test helpers, unused fixtures, or `any` in
tests fail the same gates.
