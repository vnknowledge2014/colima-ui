#!/usr/bin/env bash
# Installs the repo's .githooks dir (runs via the `prepare` script on install).
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$ROOT" ]; then
  exit 0
fi
git config core.hooksPath .githooks

echo "✅ git hooks installed: $(git config core.hooksPath) (pre-push lint gate active)"
