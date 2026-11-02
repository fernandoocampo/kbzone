#!/usr/bin/env bash
# Two-layer git Merkle lock (SCAR-GIT-23 pattern, adapted from article 11).
#
# Flow:
#   1. At review sign-off (Target 45):  .ai/hooks/pre-commit.sh lock      -> stages all, stores tree hash
#   2. As a git pre-commit hook:        .ai/hooks/pre-commit.sh           -> verifies working tree + index == approved tree
#   3. After commit (optional):         .ai/hooks/pre-commit.sh verify-head -> catches hooks that rewrote code
# Install: ln -sf ../../.ai/hooks/pre-commit.sh .git/hooks/pre-commit
set -euo pipefail

# Lock lives inside .git so it is never part of the tree it describes.
LOCK="${APPROVED_TREE_FILE:-$(git rev-parse --git-dir)/ai-approved-tree}"

case "${1:-check}" in
  lock)
    git add -A
    git write-tree > "$LOCK"
    echo "locked approved tree: $(cat "$LOCK")"
    ;;
  check)
    [ -f "$LOCK" ] || { echo "SCAR-GIT-23: no approved tree; run '$0 lock' after review sign-off" >&2; exit 1; }
    approved=$(cat "$LOCK")
    # Layer 1: no unstaged edits, no rogue untracked files
    if [ -n "$(git diff --name-status)" ]; then
      echo "SCAR-GIT-23: unstaged edits since approval:" >&2; git diff --name-status >&2; exit 1
    fi
    if [ -n "$(git status --porcelain | grep '^??' || true)" ]; then
      echo "SCAR-GIT-23: untracked files not in approved tree:" >&2; git status --porcelain | grep '^??' >&2; exit 1
    fi
    # Layer 2: staged index must equal the approved tree
    current=$(git write-tree)
    if [ "$current" != "$approved" ]; then
      echo "SCAR-GIT-23: staged tree differs from approved tree:" >&2
      git diff-tree --name-status "$approved" "$current" >&2
      exit 1
    fi
    ;;
  verify-head)
    approved=$(cat "$LOCK")
    head_tree=$(git rev-parse 'HEAD^{tree}')
    if [ "$head_tree" != "$approved" ]; then
      echo "SCAR-GIT-23: a hook rewrote code during commit:" >&2
      git diff-tree --name-status "$approved" "$head_tree" >&2
      exit 1
    fi
    ;;
  *)
    echo "usage: $0 [lock|check|verify-head]" >&2; exit 2
    ;;
esac
