#!/bin/bash
# release.sh — Prepare a SemVer bump (minor/major/pre-release) for ColimaUI
#
# NOTE: Every push/merge to `main` that passes CI automatically increments the
# patch version, tags `main`, and publishes a GitHub Release via
# `.github/workflows/release.yml` (`workflow_run`). You do NOT need to run this
# script for routine patch releases!
#
# Use this script when you want to bump `minor` / `major` on a PR branch or cut
# an explicit pre-release tag (`-beta.N` / `-rc.N`):
#   ./scripts/release.sh minor          # bump version files on current branch
#   ./scripts/release.sh major          # bump version files on current branch
#   ./scripts/release.sh 0.3.0-rc.1     # bump + optional --tag push on main
#   ./scripts/release.sh 0.3.0 --tag    # bump, push commit, wait for CI, push tag

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

CURRENT=$(node -e "console.log(require('./package.json').version)")
echo -e "${CYAN}📦 Current package.json version: ${YELLOW}v${CURRENT}${NC}"

BUMP="${1:-}"
PUSH_TAG="${2:-}"
if [ -z "$BUMP" ]; then
  echo -e "${RED}❌ Usage: $0 <patch|minor|major|X.Y.Z[-rc.N|-beta.N]> [--tag]${NC}"
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "${CURRENT%-*}"
case "$BUMP" in
  patch) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  minor) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
  major) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
  *)
    if [[ ! "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
      echo -e "${RED}❌ Invalid version: $BUMP${NC}"
      echo "   Expected: X.Y.Z, X.Y.Z-beta.1, or X.Y.Z-rc.1"
      exit 1
    fi
    NEW_VERSION="$BUMP"
    ;;
esac

TAG="v${NEW_VERSION}"
echo -e "${CYAN}🚀 Target version: ${GREEN}${TAG}${NC}"

if ! git diff --quiet HEAD 2>/dev/null; then
  echo -e "${YELLOW}⚠️  You have uncommitted changes. Commit or stash them first.${NC}"
  git status --short
  exit 1
fi

if git tag -l "$TAG" | grep -Fxq "$TAG"; then
  echo -e "${RED}❌ Tag $TAG already exists${NC}"
  exit 1
fi

echo -e "${CYAN}📝 Updating package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json...${NC}"
NEW_VERSION="$NEW_VERSION" node -e '
  const fs = require("node:fs");
  const ver = process.env.NEW_VERSION;
  for (const file of ["package.json", "src-tauri/tauri.conf.json"]) {
    const json = JSON.parse(fs.readFileSync(file, "utf8"));
    json.version = ver;
    fs.writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
  }
'
sed -i.bak "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" src-tauri/Cargo.toml
rm -f src-tauri/Cargo.toml.bak

git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore(release): v${NEW_VERSION}"

if [[ "$PUSH_TAG" == "--tag" ]]; then
  echo -e "${CYAN}🚀 Pushing commit to origin...${NC}"
  git push origin HEAD
  echo -e "${CYAN}🏷️  Creating and pushing tag ${TAG}...${NC}"
  git tag -a "$TAG" -m "Release ${TAG}"
  git push origin "$TAG"
  echo -e "${GREEN}✅ Tag ${TAG} pushed — release.yml will wait for CI and publish ${TAG}.${NC}"
else
  echo ""
  echo -e "${GREEN}✅ Version bumped to v${NEW_VERSION} on branch $(git branch --show-current).${NC}"
  echo -e "${CYAN}   Open/merge a PR to main — once CI passes on main, GitHub Actions will automatically tag and publish ${TAG}.${NC}"
fi
