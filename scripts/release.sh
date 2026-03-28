#!/bin/bash
# release.sh — Automated version bump + tag + push
#
# Usage:
#   ./scripts/release.sh patch   # 0.1.0 → 0.1.1
#   ./scripts/release.sh minor   # 0.1.0 → 0.2.0
#   ./scripts/release.sh major   # 0.1.0 → 1.0.0
#   ./scripts/release.sh 1.2.3   # explicit version
#
# This script:
# 1. Validates the version bump
# 2. Updates version in package.json, Cargo.toml, tauri.conf.json
# 3. Commits the version bump
# 4. Creates a git tag
# 5. Pushes to origin (triggers GitHub Actions release workflow)

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# --- Get current version from package.json ---
CURRENT=$(node -e "console.log(require('./package.json').version)")
echo -e "${CYAN}📦 Current version: ${YELLOW}v${CURRENT}${NC}"

# --- Parse bump type ---
BUMP="${1:-}"
if [ -z "$BUMP" ]; then
  echo -e "${RED}❌ Usage: $0 <patch|minor|major|X.Y.Z>${NC}"
  exit 1
fi

# Calculate new version
IFS='.' read -r MAJOR MINOR PATCH <<< "${CURRENT%-*}"  # strip pre-release suffix
case "$BUMP" in
  patch) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  minor) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
  major) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
  *)
    # Explicit version — validate semver
    if [[ ! "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
      echo -e "${RED}❌ Invalid version: $BUMP${NC}"
      echo "   Expected: X.Y.Z or X.Y.Z-beta.1"
      exit 1
    fi
    NEW_VERSION="$BUMP"
    ;;
esac

TAG="v${NEW_VERSION}"
echo -e "${CYAN}🚀 New version: ${GREEN}v${NEW_VERSION}${NC}"

# --- Check for uncommitted changes ---
if ! git diff --quiet HEAD 2>/dev/null; then
  echo -e "${YELLOW}⚠️  You have uncommitted changes. Commit or stash them first.${NC}"
  git status --short
  exit 1
fi

# --- Check tag doesn't already exist ---
if git tag -l "$TAG" | grep -q "$TAG"; then
  echo -e "${RED}❌ Tag $TAG already exists${NC}"
  exit 1
fi

# --- Update versions ---
echo -e "${CYAN}📝 Updating package.json...${NC}"
node -e "
  const fs = require('fs');
  const pkg = JSON.parse(fs.readFileSync('package.json'));
  pkg.version = '${NEW_VERSION}';
  fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"

echo -e "${CYAN}📝 Updating src-tauri/Cargo.toml...${NC}"
sed -i.bak "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" src-tauri/Cargo.toml
rm -f src-tauri/Cargo.toml.bak

echo -e "${CYAN}📝 Updating src-tauri/tauri.conf.json...${NC}"
node -e "
  const fs = require('fs');
  const conf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json'));
  conf.version = '${NEW_VERSION}';
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(conf, null, 2) + '\n');
"

# --- Commit + tag + push ---
echo -e "${CYAN}📦 Committing version bump...${NC}"
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore(release): v${NEW_VERSION}"

echo -e "${CYAN}🏷️  Creating tag ${TAG}...${NC}"
git tag -a "$TAG" -m "Release ${TAG}"

echo -e "${CYAN}🚀 Pushing to origin...${NC}"
git push origin HEAD
git push origin "$TAG"

echo ""
echo -e "${GREEN}✅ Release ${TAG} pushed!${NC}"
echo -e "${CYAN}   GitHub Actions will now build and publish the release.${NC}"
echo -e "${CYAN}   Track progress: https://github.com/$(git remote get-url origin | sed 's/.*github.com[:/]//' | sed 's/.git$//')/actions${NC}"
