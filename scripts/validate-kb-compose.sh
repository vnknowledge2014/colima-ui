#!/usr/bin/env bash
# Validate the Compose snippets embedded in the honeypot Knowledge Base articles.
#
# The YAML lives inside the markdown rather than in separate .yml files: the
# article is what users copy from, so a separate file would be a second copy
# free to drift from the one people actually use. This script keeps the single
# source honest by extracting every fenced yaml block and running it through
# `docker compose config`.
#
# It also enforces the one safety property those articles promise: nothing they
# publish is reachable from outside the machine. An article that told a reader
# to expose a deliberately weak SSH service on every interface would be actively
# harmful, and this check is the only thing standing between that and a release.
#
# Why it asserts on `docker compose config --format json` rather than on the raw
# text: the raw text has too many ways to say the same thing. `ports: ["2222:2222"]`
# in flow style, the long `- target:/published:` syntax, and `network_mode: host`
# all bypass a line-oriented grep while still exposing the service. Compose's own
# normalizer collapses every spelling into one shape, so the assertion is made
# against what Compose will actually do rather than against how it was written.
#
# Requires: docker (with compose v2), python3. Run from anywhere.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
kb_dir="$repo_root/src-tauri/resources/kb"

if ! docker compose version >/dev/null 2>&1; then
  echo "docker compose is not available; cannot validate" >&2
  exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is not available; cannot validate" >&2
  exit 2
fi

# Reads normalized compose JSON on stdin. Exits non-zero and explains if any
# service would be reachable from off-machine.
assert_loopback_only() {
  python3 -c '
import json, sys

doc = json.load(sys.stdin)
problems = []

for name, svc in (doc.get("services") or {}).items():
    # host networking ignores port mappings entirely and publishes everything.
    if svc.get("network_mode") == "host":
        problems.append(f"service {name}: network_mode: host publishes every port on every interface")

    for port in svc.get("ports") or []:
        # Compose normalizes to long form, but be defensive: a plain string here
        # would mean the shape changed under us, and silently skipping it is the
        # exact failure this script exists to prevent.
        if not isinstance(port, dict):
            problems.append(f"service {name}: unexpected port shape {port!r}; refusing to guess")
            continue
        host_ip = port.get("host_ip")
        published = port.get("published", "?")
        if host_ip not in ("127.0.0.1", "::1"):
            shown = host_ip if host_ip else "all interfaces"
            problems.append(f"service {name}: published port {published} binds {shown}, not 127.0.0.1")

for p in problems:
    print(p)
sys.exit(1 if problems else 0)
'
}

blocks=0
failures=0

# Print the Nth fenced ```yaml block of a file (1-indexed).
extract_yaml_block() {
  awk -v want="$2" '
    /^```yaml$/ { depth++; if (depth == want) { inblock = 1; next } }
    /^```$/     { if (inblock) { inblock = 0; exit } }
    inblock     { print }
  ' "$1"
}

for article in "$kb_dir"/*/honeypot-*.md; do
  rel="${article#"$repo_root"/}"
  total=$(grep -c '^```yaml$' "$article" || true)
  [ "$total" -eq 0 ] && continue

  for i in $(seq 1 "$total"); do
    blocks=$((blocks + 1))
    yaml=$(extract_yaml_block "$article" "$i")

    if ! normalized=$(printf '%s\n' "$yaml" | docker compose -f - config --format json 2>&1); then
      echo "FAIL  $rel block #$i — docker compose config rejected it" >&2
      printf '%s\n' "$normalized" | sed 's/^/      /' >&2
      failures=$((failures + 1))
      continue
    fi

    if ! verdict=$(printf '%s\n' "$normalized" | assert_loopback_only); then
      while IFS= read -r line; do
        [ -n "$line" ] && echo "FAIL  $rel block #$i — $line" >&2
      done <<< "$verdict"
      failures=$((failures + 1))
    fi
  done
done

if [ "$blocks" -eq 0 ]; then
  echo "No yaml blocks found under $kb_dir — nothing validated" >&2
  exit 2
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures problem(s) across $blocks compose block(s)" >&2
  exit 1
fi

echo "OK: $blocks compose block(s) valid, nothing reachable from off-machine"
