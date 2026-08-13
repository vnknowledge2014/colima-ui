#!/usr/bin/env node
/**
 * Fails when a stylesheet or component reads a CSS custom property that nothing
 * defines. Such a declaration is dropped silently by the browser, so a panel
 * meant to be dark simply paints transparent and nothing reports it.
 *
 * Run: node scripts/check-css-tokens.mjs
 */
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const SOURCE_DIR = "src";
const SOURCE_FILE = /\.(svelte|css|ts)$/;

/** Properties written at runtime rather than declared in a stylesheet. */
const RUNTIME_DEFINED = [/^--col-\d+$/];

function collectFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) return collectFiles(full);
    return SOURCE_FILE.test(entry.name) ? [full] : [];
  });
}

const defined = new Set();
const used = new Map();

for (const file of collectFiles(SOURCE_DIR)) {
  const source = readFileSync(file, "utf8");
  for (const [, name] of source.matchAll(/(--[a-z0-9-]+)\s*:/g)) defined.add(name);
  for (const [, name] of source.matchAll(/var\(\s*(--[a-z0-9-]+)/g)) {
    if (!used.has(name)) used.set(name, new Set());
    used.get(name).add(file);
  }
}

const missing = [...used.keys()]
  .filter((name) => !defined.has(name) && !RUNTIME_DEFINED.some((r) => r.test(name)))
  .sort();

if (missing.length === 0) {
  console.log(`CSS tokens OK — ${defined.size} defined, ${used.size} referenced.`);
  process.exit(0);
}

console.error(`Undefined CSS custom properties (${missing.length}):`);
for (const name of missing) {
  const files = [...used.get(name)].sort();
  console.error(`  ${name} — ${files.slice(0, 4).join(", ")}${files.length > 4 ? `, +${files.length - 4} more` : ""}`);
}
console.error("\nDefine them in src/styles/tokens.css, or point the call sites at an existing token.");
process.exit(1);
