#!/usr/bin/env node
/**
 * Compare translation files against each other.
 *
 * `t()` takes a `{ default }` fallback, so a missing key renders English instead of
 * breaking the screen. That is the right runtime behaviour and a terrible review
 * signal: a locale can silently drift for months and nothing looks wrong to anyone
 * testing in English. This turns that drift into a failing check.
 *
 * English is the reference because every `t()` call site carries an English
 * `default` anyway.
 *
 *   node scripts/check-i18n-keys.mjs
 *
 * Exits non-zero when a locale is missing keys or carries keys English does not.
 */

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const LOCALES_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "locales");
const REFERENCE = "en.json";

/** Flatten to dotted paths so nesting differences show up as key differences. */
function flatten(value, prefix = "", out = new Set()) {
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) {
      flatten(child, path, out);
    } else {
      out.add(path);
    }
  }
  return out;
}

function load(file) {
  return flatten(JSON.parse(readFileSync(join(LOCALES_DIR, file), "utf8")));
}

const reference = load(REFERENCE);
const others = readdirSync(LOCALES_DIR).filter((f) => f.endsWith(".json") && f !== REFERENCE);

let failed = false;

/**
 * Keys the code asks for that English does not define.
 *
 * Comparing the locales to each other cannot see these: a key absent from *all*
 * four files looks perfectly consistent while every language renders the English
 * `default`. That is how `volumes.*` and `sidebar.colima_instances` went missing
 * from every translation without anything looking wrong.
 */
const SRC_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const CALL = /\bt\(\s*"([a-zA-Z0-9_.]+)"\s*,\s*\{\s*default:/g;

function* sourceFiles(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* sourceFiles(path);
    else if (/\.(svelte|ts)$/.test(entry.name) && !/\.test\.ts$/.test(entry.name)) yield path;
  }
}

const undefinedKeys = new Set();
for (const file of sourceFiles(SRC_DIR)) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(CALL)) {
    if (!reference.has(match[1])) undefinedKeys.add(match[1]);
  }
}

if (undefinedKeys.size > 0) {
  failed = true;
  console.error(`✗ ${REFERENCE} — used by the code, never defined:`);
  for (const key of [...undefinedKeys].sort()) console.error(`    ${key}`);
  console.error("  (these render their English `default` in every language)");
}

for (const file of others) {
  const keys = load(file);
  const missing = [...reference].filter((k) => !keys.has(k)).sort();
  // Extra keys are reported too: they are usually a rename that landed in one file,
  // which leaves the old text live in every other locale.
  const extra = [...keys].filter((k) => !reference.has(k)).sort();

  if (missing.length === 0 && extra.length === 0) {
    console.log(`✓ ${file} — ${keys.size} keys`);
    continue;
  }
  failed = true;
  console.error(`✗ ${file}`);
  for (const key of missing) console.error(`    missing: ${key}`);
  for (const key of extra) console.error(`    not in ${REFERENCE}: ${key}`);
}

if (failed) {
  console.error("\nTranslation files disagree. Add the keys, or remove the stale ones.");
  process.exit(1);
}
console.log(`\nAll ${others.length + 1} locales agree.`);
