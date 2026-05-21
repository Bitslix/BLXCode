#!/usr/bin/env bash
# Sync docs/ to the GitHub Wiki repository (Bitslix/BLXCode.wiki).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OWNER="${WIKI_OWNER:-Bitslix}"
REPO="${WIKI_REPO:-BLXCode}"
BRANCH="${WIKI_BRANCH:-main}"
DRY_RUN="${WIKI_DRY_RUN:-0}"
PUSH="${WIKI_PUSH:-1}"

usage() {
  cat <<'EOF'
Usage: scripts/sync_github_wiki.sh [options]

Environment:
  WIKI_OWNER       GitHub owner (default: Bitslix)
  WIKI_REPO        Repository name (default: BLXCode)
  WIKI_BRANCH      Branch for raw image URLs (default: main)
  WIKI_DRY_RUN     1 = transform only, print output dir, no git push
  WIKI_PUSH        1 = commit and push wiki repo (default); 0 = transform only
  WIKI_SYNC_TOKEN  Optional PAT if GITHUB_TOKEN cannot push the wiki repo
  GITHUB_SHA       Optional commit SHA for wiki commit message (CI sets this)

Options:
  --dry-run        Same as WIKI_DRY_RUN=1
  --no-push        Same as WIKI_PUSH=0
  -h, --help       Show this help

Prerequisites:
  - Wiki enabled on https://github.com/Bitslix/BLXCode (Settings → Features → Wiki)
  - Edit docs/ only in the main repo; do not edit the wiki in the browser (CI overwrites)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; PUSH=0 ;;
    --no-push) PUSH=0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
  shift
done

OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blxcode-wiki.XXXXXX")"
trap 'rm -rf "$OUT_DIR"' EXIT

python3 "$ROOT/scripts/sync_github_wiki.py" \
  --repo-root "$ROOT" \
  --output-dir "$OUT_DIR" \
  --owner "$OWNER" \
  --repo "$REPO" \
  --branch "$BRANCH"

if [[ "$DRY_RUN" == "1" ]]; then
  echo "Dry run — transformed wiki pages:"
  ls -1 "$OUT_DIR"
  exit 0
fi

WIKI_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blxcode-wiki-git.XXXXXX")"
WIKI_URL="https://github.com/${OWNER}/${REPO}.wiki.git"

if [[ -n "${WIKI_SYNC_TOKEN:-}" ]]; then
  CLONE_URL="https://x-access-token:${WIKI_SYNC_TOKEN}@${GITHUB_SERVER:-github.com}/${OWNER}/${REPO}.wiki.git"
else
  CLONE_URL="$WIKI_URL"
fi

if ! git clone "$CLONE_URL" "$WIKI_DIR" 2>/dev/null; then
  echo "error: could not clone ${WIKI_URL}" >&2
  echo "Enable Wiki on the repository (Settings → Features → Wiki), then retry." >&2
  exit 1
fi

# Replace all tracked wiki pages with generated output
find "$WIKI_DIR" -maxdepth 1 -name '*.md' -delete
cp "$OUT_DIR"/*.md "$WIKI_DIR/"

cd "$WIKI_DIR"
git add -A
if git diff --staged --quiet; then
  echo "Wiki already up to date."
  exit 0
fi

SHA="${GITHUB_SHA:-$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)}"
git -c user.name="blxcode-wiki-sync" -c user.email="blxcode-wiki-sync@users.noreply.github.com" \
  commit -m "docs: sync from ${REPO}@${SHA}"

if [[ "$PUSH" != "1" ]]; then
  echo "Committed locally in $WIKI_DIR (WIKI_PUSH=0, not pushing)."
  trap - EXIT
  exit 0
fi

if [[ -n "${WIKI_SYNC_TOKEN:-}" ]]; then
  git push "$CLONE_URL" HEAD
else
  git push origin HEAD
fi

echo "Wiki synced: https://github.com/${OWNER}/${REPO}/wiki"
