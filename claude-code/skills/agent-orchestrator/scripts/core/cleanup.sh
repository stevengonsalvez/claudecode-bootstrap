#!/bin/bash

# ABOUTME: Agent cleanup script - removes completed agents and optionally merges worktree changes
# Part of the agent-orchestrator skill

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"
TOOL_DIR="$(dirname "$(dirname "$SKILL_DIR")")"

# Source utilities
source "${TOOL_DIR}/utils/git-worktree-utils.sh"

# Parse arguments
SESSION=""
MERGE=false
FORCE=false
ALL_COMPLETED=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --merge|-m)
            MERGE=true
            shift
            ;;
        --force|-f)
            FORCE=true
            shift
            ;;
        --all-completed)
            ALL_COMPLETED=true
            shift
            ;;
        agent-*)
            SESSION="$1"
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# Detect agent status from tmux output
detect_agent_status() {
    local SESSION=$1

    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        echo "killed"
        return 0
    fi

    local OUTPUT=$(tmux capture-pane -t "$SESSION" -p -S -100 2>/dev/null || echo "")

    # Check for completion indicators
    if echo "$OUTPUT" | grep -qiE "complete|done|finished|All.*tasks.*complete"; then
        echo "complete"
        return 0
    fi

    # Check for failure indicators
    if echo "$OUTPUT" | grep -qiE "error|failed|fatal|Error:"; then
        echo "failed"
        return 0
    fi

    # Check for idle
    local LAST_LINE=$(echo "$OUTPUT" | tail -5)
    if echo "$LAST_LINE" | grep -qE "^>|Style:|bypass permissions"; then
        echo "idle"
        return 0
    fi

    echo "active"
}

# Get agent metadata
get_agent_metadata() {
    local SESSION=$1
    local METADATA_FILE="$HOME/.claude/agents/${SESSION}.json"

    if [ -f "$METADATA_FILE" ]; then
        cat "$METADATA_FILE"
    else
        echo "{}"
    fi
}

# Cleanup a single agent
cleanup_agent() {
    local SESSION=$1
    local MERGE_OPT=$2
    local FORCE_OPT=$3

    echo "Cleaning up agent: $SESSION"

    local METADATA=$(get_agent_metadata "$SESSION")
    local WITH_WORKTREE=$(echo "$METADATA" | jq -r '.with_worktree // false' 2>/dev/null)
    local WORKTREE_BRANCH=$(echo "$METADATA" | jq -r '.worktree_branch // ""' 2>/dev/null)
    local DIRECTORY=$(echo "$METADATA" | jq -r '.directory // ""' 2>/dev/null)

    # Check if session is still active
    local STATUS=$(detect_agent_status "$SESSION")
    if [ "$STATUS" = "active" ] && [ "$FORCE_OPT" = false ]; then
        echo "Warning: Agent is still active. Use --force to cleanup anyway."
        return 1
    fi

    # Handle worktree cleanup
    if [ "$WITH_WORKTREE" = "true" ] && [ -n "$WORKTREE_BRANCH" ]; then
        # Extract agent ID from session name (agent-{timestamp})
        local AGENT_ID=$(echo "$SESSION" | sed 's/agent-//')

        # Check for uncommitted changes in worktree
        if [ -d "$DIRECTORY" ]; then
            if ! git -C "$DIRECTORY" diff --quiet 2>/dev/null; then
                if [ "$FORCE_OPT" = false ]; then
                    echo "Warning: Worktree has uncommitted changes."
                    echo "  Directory: $DIRECTORY"
                    echo "  Use --force to discard changes, or commit them first."
                    return 1
                fi
            fi

            # Merge if requested
            if [ "$MERGE_OPT" = true ]; then
                echo "Merging agent branch: $WORKTREE_BRANCH"

                # Check for commits to merge
                local COMMITS=$(git log --oneline "HEAD..$WORKTREE_BRANCH" 2>/dev/null | wc -l | tr -d ' ')
                if [ "$COMMITS" -gt 0 ]; then
                    echo "Found $COMMITS commit(s) to merge"

                    # Try merge
                    if git merge "$WORKTREE_BRANCH" -m "Merge agent work: $SESSION"; then
                        echo "Merge successful"
                    else
                        echo "Merge failed - resolve conflicts and run cleanup again"
                        return 1
                    fi
                else
                    echo "No commits to merge"
                fi
            fi
        fi

        # Remove worktree
        echo "Removing worktree..."
        if [ "$FORCE_OPT" = true ]; then
            cleanup_agent_worktree "$AGENT_ID" true 2>/dev/null || true
        else
            cleanup_agent_worktree "$AGENT_ID" false 2>/dev/null || true
        fi
    fi

    # Kill tmux session
    if tmux has-session -t "$SESSION" 2>/dev/null; then
        echo "Killing tmux session..."
        tmux kill-session -t "$SESSION"
    fi

    # Remove metadata file
    local METADATA_FILE="$HOME/.claude/agents/${SESSION}.json"
    if [ -f "$METADATA_FILE" ]; then
        rm "$METADATA_FILE"
        echo "Removed metadata file"
    fi

    echo "Cleanup complete: $SESSION"
    echo ""
}

# Get all agent sessions
get_agent_sessions() {
    tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^agent-" || true
}

# Main

if [ "$ALL_COMPLETED" = true ]; then
    echo "Cleaning up all completed agents..."
    echo ""

    SESSIONS=$(get_agent_sessions)
    CLEANED=0

    while IFS= read -r SESSION; do
        [ -z "$SESSION" ] && continue

        STATUS=$(detect_agent_status "$SESSION")
        if [ "$STATUS" = "complete" ] || [ "$STATUS" = "killed" ]; then
            cleanup_agent "$SESSION" "$MERGE" "$FORCE" || true
            CLEANED=$((CLEANED + 1))
        fi
    done <<< "$SESSIONS"

    if [ $CLEANED -eq 0 ]; then
        echo "No completed agents to clean up"
    else
        echo "Cleaned up $CLEANED agent(s)"
    fi

elif [ -n "$SESSION" ]; then
    # Cleanup specific session
    cleanup_agent "$SESSION" "$MERGE" "$FORCE"

else
    echo "Usage: cleanup.sh <agent-session> [options]"
    echo ""
    echo "Options:"
    echo "  --merge, -m        Merge worktree changes before cleanup"
    echo "  --force, -f        Force cleanup even with uncommitted changes or active agents"
    echo "  --all-completed    Clean up all completed agents"
    echo ""
    echo "Examples:"
    echo "  cleanup.sh agent-1705161234"
    echo "  cleanup.sh agent-1705161234 --merge"
    echo "  cleanup.sh agent-1705161234 --force"
    echo "  cleanup.sh --all-completed"
    echo ""
    echo "Current agents:"
    bash "$SCRIPT_DIR/status.sh" 2>/dev/null || echo "  (none)"
    exit 1
fi
