#!/usr/bin/env bash
# Installs the repo's .githooks dir (runs via the `prepare` script on install).
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
git config core.hooksPath .githooks

echo "✅ git hooks installed: $(git config core.hooksPath) (pre-push lint gate active)"
