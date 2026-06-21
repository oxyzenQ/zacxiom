#!/usr/bin/env bash
# version-to.sh — Advanced single-source version bumping for zacxiom
# Usage: ./version-to.sh vX.Y.Z [--dry-run]
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
  shift
fi

if [ $# -ne 1 ]; then
  echo "Usage: $0 [--dry-run] <version>"
  echo "Example: $0 v2.0.0"
  echo "         $0 --dry-run v2.0.0"
  exit 1
fi

NEW="${1#v}"
NEW_TAG="v${NEW}"
CUR="$(grep -E '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')"
CUR_TAG="v${CUR}"

if [ "${NEW}" == "${CUR}" ]; then
  echo "Already at version ${NEW_TAG}"
  exit 0
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  zacxiom version-to"
echo "  ${CUR_TAG} → ${NEW_TAG}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

update_file() {
  local file="$1"
  local sed_pattern="$2"
  if [ ! -f "$file" ]; then return; fi

  local before after
  before=$(grep -n "${CUR}" "$file" 2>/dev/null || true)
  if [ -z "$before" ]; then return; fi

  if $DRY_RUN; then
    echo "  [DRY-RUN] Would update: $file"
    echo "$before" | while read line; do echo "    L$line"; done
    return
  fi

  echo "  Updating: $file"
  sed -i "${sed_pattern}" "$file"
  after=$(grep -n "${NEW}" "$file" 2>/dev/null || true)
  echo "$after" | while read line; do echo "    → L$line"; done
}

# 1. Cargo.toml — primary source
update_file "Cargo.toml" "s/^version = \"${CUR}\"/version = \"${NEW}\"/"

# 2. README.md — version badge / version section
update_file "README.md" "s/${CUR_TAG}/${NEW_TAG}/g"

# 3. ARCHITECTURE.md — any version references
update_file "ARCHITECTURE.md" "s/v${CUR}/v${NEW}/g"

# 4. RULES.md — version references
update_file "RULES.md" "s/v${CUR}/v${NEW}/g"

echo ""
echo "Version bumped: ${CUR_TAG} → ${NEW_TAG}"
echo ""
echo "Next steps:"
echo "  ./build.sh check-all"
echo "  git add -A && git commit -m 'chore: bump to ${NEW_TAG}'"
echo "  git tag ${NEW_TAG}"
echo "  git push origin main --tags"
