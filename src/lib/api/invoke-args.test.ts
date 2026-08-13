import { describe, it, expect } from "vitest";

/**
 * Tauri derives each command's expected argument names from its Rust parameters
 * and camelCases them. A snake_case key in the `invoke` payload is not ignored —
 * the call is rejected before the command runs:
 *
 *   invalid args `apiKey` for command `ai_list_models`:
 *   command ai_list_models missing required key apiKey
 *
 * Nothing catches that at build time: the payload is a plain object, so the
 * mistake only surfaces when a user clicks the button in the packaged app. Every
 * such call was broken from the day it was written. This test reads the source
 * of every api module and fails on the pattern.
 *
 * The HTTP body — the last argument of `call()` — stays snake_case to match
 * serde on the axum handlers, and is deliberately not checked here.
 */

// Read through Vite rather than node:fs: this suite is type-checked against the
// browser tsconfig, which has no node types.
const modules = import.meta.glob("./*.ts", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** `call<T>("cmd", {` — the opening of the second argument. */
const CALL_WITH_ARGS = /call<[^>]*>\(\s*"([a-z0-9_]+)"\s*,\s*\{/g;

/**
 * Read the object that starts at `open`, tracking brace depth.
 *
 * Only depth-1 keys are argument names. A nested object is a struct parameter
 * (`{ request: {...} }`) whose fields are named by serde and are correctly
 * snake_case, so descending into it would report false positives.
 */
function topLevelKeys(source: string, open: number): string[] {
  const keys: string[] = [];
  let depth = 0;
  let token = "";

  for (let i = open; i < source.length; i++) {
    const char = source[i];

    if (char === "{" || char === "[" || char === "(") {
      // A key is whatever was being accumulated when its value began.
      if (char === "{" && depth >= 1) token = "";
      depth++;
      continue;
    }
    if (char === "}" || char === "]" || char === ")") {
      depth--;
      if (depth === 0) {
        const trailing = token.trim();
        if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(trailing)) keys.push(trailing);
        break;
      }
      continue;
    }

    if (depth !== 1) continue;

    if (char === ":") {
      const key = token.trim();
      if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) keys.push(key);
      // Skip to the end of this value; the next key starts after the comma.
      token = "";
      let valueDepth = 0;
      for (i++; i < source.length; i++) {
        const c = source[i];
        if (c === "{" || c === "[" || c === "(") valueDepth++;
        else if (c === "}" || c === "]" || c === ")") {
          if (valueDepth === 0) {
            i--;
            break;
          }
          valueDepth--;
        } else if (c === "," && valueDepth === 0) break;
      }
      continue;
    }

    if (char === ",") {
      // Shorthand property: `{ provider, endpoint }`.
      const shorthand = token.trim();
      if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(shorthand)) keys.push(shorthand);
      token = "";
      continue;
    }

    token += char;
  }

  return [...new Set(keys)];
}

describe("Tauri invoke arguments", () => {
  it("are camelCase in every api module", () => {
    const sources = Object.entries(modules).filter(([path]) => !path.includes(".test."));
    expect(sources.length).toBeGreaterThan(5);

    const offenders: string[] = [];
    for (const [path, source] of sources) {
      for (const match of source.matchAll(CALL_WITH_ARGS)) {
        const command = match[1];
        // The match ends on the `{`, which is where the object begins.
        const open = match.index + match[0].length - 1;
        for (const key of topLevelKeys(source, open)) {
          if (key.includes("_")) offenders.push(`${path}: ${command} → ${key}`);
        }
      }
    }

    expect(offenders).toEqual([]);
  });
});
