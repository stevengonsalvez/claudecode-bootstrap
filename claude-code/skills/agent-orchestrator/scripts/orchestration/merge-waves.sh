#!/bin/bash

# ABOUTME: Merges completed wave worktrees into the main branch
# Part of the agent-orchestrator skill - orchestration mode

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL_DIR="$(dirname "$(dirname "$(dirname "$SCRIPT_DIR")")")"

# Source utilities
source "${TOOL_DIR}/utils/git-worktree-utils.sh"

# Parse arguments
SESSION_ID="${1:-}"
DRY_RUN=false
FORCE=false
WAVE_FILTER=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run|-n)
            DRY_RUN=true
            shift
            ;;
        --force|-f)
            FORCE=true
            shift
            ;;
        --wave)
            WAVE_FILTER="$2"
            shift 2
            ;;
        *)
            if [ -z "$SESSION_ID" ] || [[ "$1" != -* ]]; then
                SESSION_ID="$1"
            fi
            shift
            ;;
    esac
done

if [ -z "$SESSION_ID" ]; then
    echo "Usage: merge-waves.sh <session-id> [options]"
    echo ""
    echo "Merges completed agent worktrees into current branch."
    echo ""
    echo "Options:"
    echo "  --dry-run, -n    Show what would be merged without merging"
    echo "  --force, -f      Force merge even if session not complete"
    echo "  --wave N         Only merge specific wave"
    echo ""
    echo "Example:"
    echo "  merge-waves.sh orch-1705161234"
    echo "  merge-waves.sh orch-1705161234 --dry-run"
    echo "  merge-waves.sh orch-1705161234 --wave 1"
    exit 1
fi

# Load session
ORCH_DIR="$HOME/.claude/orchestration/state"
SESSION_FILE="$ORCH_DIR/session-${SESSION_ID}.json"

if [ ! -f "$SESSION_FILE" ]; then
    echo "Error: Session not found: $SESSION_ID"
    exit 1
fi

STATUS=$(jq -r '.status' "$SESSION_FILE")

if [ "$STATUS" != "complete" ] && [ "$FORCE" = false ]; then
    echo "Error: Session is not complete (status: $STATUS)"
    echo "Hint: Wait for all waves to complete or use --force"
    exit 1
fi

echo "========================================"
echo "Merging Worktrees: $SESSION_ID"
echo "========================================"
echo ""

# Get completed agents with worktrees
if [ -n "$WAVE_FILTER" ]; then
    AGENTS=$(jq -r --arg w "$WAVE_FILTER" \
        '.agents | to_entries[] | select(.value.wave == ($w | tonumber) and .value.status == "complete") | .key' \
        "$SESSION_FILE")
else
    AGENTS=$(jq -r '.agents | to_entries[] | select(.value.status == "complete") | .key' "$SESSION_FILE")
fi

if [ -z "$AGENTS" ]; then
    echo "No completed agents to merge"
    exit 0
fi

# Count commits per agent
echo "Agents to merge:"
TOTAL_COMMITS=0
for AGENT in $AGENTS; do
    BRANCH=$(jq -r --arg a "$AGENT" '.agents[$a].branch' "$SESSION_FILE")
    WORKSTREAM=$(jq -r --arg a "$AGENT" '.agents[$a].workstream_id' "$SESSION_FILE")

    # Count commits ahead of current branch
    COMMITS=$(git rev-list --count HEAD.."$BRANCH" 2>/dev/null || echo "0")
    TOTAL_COMMITS=$((TOTAL_COMMITS + COMMITS))

    echo "  $WORKSTREAM ($BRANCH): $COMMITS commit(s)"
done

echo ""
echo "Total commits to merge: $TOTAL_COMMITS"
echo ""

if [ "$DRY_RUN" = true ]; then
    echo "[DRY RUN] Would merge the following branches:"
    for AGENT in $AGENTS; do
        BRANCH=$(jq -r --arg a "$AGENT" '.agents[$a].branch' "$SESSION_FILE")
        echo "  git merge $BRANCH"
    done
    echo ""
    echo "Run without --dry-run to perform merge"
    exit 0
fi

# Confirm merge
if [ "$TOTAL_COMMITS" -gt 0 ]; then
    echo "Proceeding with merge..."
    echo ""
fi

# Merge each agent's branch
MERGED=0
FAILED=0

for AGENT in $AGENTS; do
    BRANCH=$(jq -r --arg a "$AGENT" '.agents[$a].branch' "$SESSION_FILE")
    WORKSTREAM=$(jq -r --arg a "$AGENT" '.agents[$a].workstream_id' "$SESSION_FILE")
    WORKTREE_DIR=$(jq -r --arg a "$AGENT" '.agents[$a].worktree_dir' "$SESSION_FILE")

    echo "Merging: $WORKSTREAM ($BRANCH)"

    # Check for uncommitted changes in worktree
    if [ -d "$WORKTREE_DIR" ]; then
        if ! git -C "$WORKTREE_DIR" diff --quiet 2>/dev/null; then
            echo "  Warning: Uncommitted changes in $WORKTREE_DIR"
            if [ "$FORCE" = false ]; then
                echo "  Skipping (use --force to include)"
                FAILED=$((FAILED + 1))
                continue
            fi
        fi
    fi

    # Attempt merge
    if git merge "$BRANCH" -m "Merge $WORKSTREAM from orchestration $SESSION_ID" 2>/dev/null; then
        echo "  Merged successfully"
        MERGED=$((MERGED + 1))

        # Mark agent as merged in session
        jq --arg a "$AGENT" '.agents[$a].merged = true | .agents[$a].merged_at = (now | todate)' \
            "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"
    else
        echo "  Merge conflict!"
        echo "  Resolve conflicts and run: git merge --continue"
        FAILED=$((FAILED + 1))

        # Abort the merge to allow user to handle
        git merge --abort 2>/dev/null || true
    fi

    echo ""
done

echo "========================================"
echo "Merge Summary"
echo "========================================"
echo ""
echo "Merged:  $MERGED"
echo "Failed:  $FAILED"
echo ""

if [ "$FAILED" -gt 0 ]; then
    echo "Some merges failed. Resolve conflicts manually and retry."
    exit 1
fi

# Offer cleanup
echo "All branches merged successfully!"
echo ""
echo "Next steps:"
echo "  1. Run tests: npm test (or equivalent)"
echo "  2. Review changes: git log --oneline -$TOTAL_COMMITS"
echo "  3. Clean up worktrees: for each agent, run cleanup"
echo ""
echo "Cleanup commands:"
for AGENT in $AGENTS; do
    WORKTREE_DIR=$(jq -r --arg a "$AGENT" '.agents[$a].worktree_dir' "$SESSION_FILE")
    BRANCH=$(jq -r --arg a "$AGENT" '.agents[$a].branch' "$SESSION_FILE")

    if [ -d "$WORKTREE_DIR" ]; then
        echo "  git worktree remove $WORKTREE_DIR && git branch -d $BRANCH"
    fi
done
echo ""
echo "Or clean all at once:"
echo "  for wt in worktrees/agent-*; do git worktree remove \"\$wt\"; done"
