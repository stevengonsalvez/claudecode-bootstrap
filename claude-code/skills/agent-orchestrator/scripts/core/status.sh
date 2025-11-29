#!/bin/bash

# ABOUTME: Agent status script - shows status of all running Claude agents
# Part of the agent-orchestrator skill

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"
TOOL_DIR="$(dirname "$(dirname "$SKILL_DIR")")"

# Output mode: compact (default), detailed, json
OUTPUT_MODE="compact"
SPECIFIC_SESSION=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --detailed|-d)
            OUTPUT_MODE="detailed"
            shift
            ;;
        --json|-j)
            OUTPUT_MODE="json"
            shift
            ;;
        agent-*)
            SPECIFIC_SESSION="$1"
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# Check if tmux is available
if ! command -v tmux &> /dev/null; then
    echo "tmux is not installed"
    exit 1
fi

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
        if echo "$OUTPUT" | grep -qE "git.*commit|Commit.*created|committed"; then
            echo "complete"
            return 0
        fi
    fi

    # Check for failure indicators
    if echo "$OUTPUT" | grep -qiE "error|failed|fatal|Error:"; then
        echo "failed"
        return 0
    fi

    # Check for idle (at Claude prompt waiting for input)
    local LAST_LINE=$(echo "$OUTPUT" | tail -5)
    if echo "$LAST_LINE" | grep -qE "^>|Style:|bypass permissions|Human:"; then
        echo "idle"
        return 0
    fi

    # Check for thinking/processing
    if echo "$OUTPUT" | grep -qE "Thought for|Planning|Implementing|Creating|Reading|Writing"; then
        echo "active"
        return 0
    fi

    # Default to active
    echo "active"
}

# Calculate runtime from created timestamp
calculate_runtime() {
    local CREATED=$1
    local NOW=$(date +%s)

    # Parse ISO timestamp
    local CREATED_TS=$(date -j -f "%Y-%m-%dT%H:%M:%S" "${CREATED:0:19}" +%s 2>/dev/null || echo "$NOW")
    local DIFF=$((NOW - CREATED_TS))

    if [ $DIFF -lt 60 ]; then
        echo "${DIFF}s"
    elif [ $DIFF -lt 3600 ]; then
        echo "$((DIFF / 60))m"
    else
        echo "$((DIFF / 3600))h $((DIFF % 3600 / 60))m"
    fi
}

# Get all agent sessions
get_agent_sessions() {
    tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^agent-" || true
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

# Output functions

output_compact() {
    local SESSIONS=$1

    if [ -z "$SESSIONS" ]; then
        echo "No agent sessions running"
        return 0
    fi

    local COUNT=$(echo "$SESSIONS" | wc -l | tr -d ' ')
    echo "$COUNT active agent(s):"
    echo ""

    while IFS= read -r SESSION; do
        [ -z "$SESSION" ] && continue

        local STATUS=$(detect_agent_status "$SESSION")
        local METADATA=$(get_agent_metadata "$SESSION")
        local TASK=$(echo "$METADATA" | jq -r '.task // "unknown"' 2>/dev/null)
        local CREATED=$(echo "$METADATA" | jq -r '.created // ""' 2>/dev/null)
        local WITH_WORKTREE=$(echo "$METADATA" | jq -r '.with_worktree // false' 2>/dev/null)

        local RUNTIME=""
        if [ -n "$CREATED" ] && [ "$CREATED" != "null" ]; then
            RUNTIME=" ($(calculate_runtime "$CREATED"))"
        fi

        local STATUS_ICON=""
        case $STATUS in
            active) STATUS_ICON="running" ;;
            idle) STATUS_ICON="idle" ;;
            complete) STATUS_ICON="complete" ;;
            failed) STATUS_ICON="FAILED" ;;
            killed) STATUS_ICON="killed" ;;
        esac

        local WORKTREE_FLAG=""
        [ "$WITH_WORKTREE" = "true" ] && WORKTREE_FLAG=" [worktree]"

        echo "$SESSION ($STATUS_ICON$RUNTIME)$WORKTREE_FLAG"

        # Truncate task to 60 chars
        local SHORT_TASK="${TASK:0:60}"
        [ ${#TASK} -gt 60 ] && SHORT_TASK="${SHORT_TASK}..."
        echo "  Task: $SHORT_TASK"

    done <<< "$SESSIONS"

    echo ""
    echo "Use --detailed for full information"
}

output_detailed() {
    local SESSIONS=$1

    if [ -z "$SESSIONS" ]; then
        echo "No agent sessions running"
        return 0
    fi

    local COUNT=$(echo "$SESSIONS" | wc -l | tr -d ' ')
    echo "========================================"
    echo "Agent Status Report"
    echo "========================================"
    echo ""
    echo "Total Agents: $COUNT"
    echo ""

    local INDEX=1
    while IFS= read -r SESSION; do
        [ -z "$SESSION" ] && continue

        local STATUS=$(detect_agent_status "$SESSION")
        local METADATA=$(get_agent_metadata "$SESSION")
        local TASK=$(echo "$METADATA" | jq -r '.task // "unknown"' 2>/dev/null)
        local DIRECTORY=$(echo "$METADATA" | jq -r '.directory // "unknown"' 2>/dev/null)
        local CREATED=$(echo "$METADATA" | jq -r '.created // ""' 2>/dev/null)
        local WITH_WORKTREE=$(echo "$METADATA" | jq -r '.with_worktree // false' 2>/dev/null)
        local WITH_HANDOVER=$(echo "$METADATA" | jq -r '.with_handover // false' 2>/dev/null)
        local WORKTREE_BRANCH=$(echo "$METADATA" | jq -r '.worktree_branch // ""' 2>/dev/null)

        echo "----------------------------------------"
        echo "## $INDEX. $SESSION"
        echo "----------------------------------------"
        echo ""
        echo "Task: $TASK"
        echo ""

        # Status with icon
        case $STATUS in
            active) echo "Status: Running (processing)" ;;
            idle) echo "Status: Idle (waiting for input)" ;;
            complete) echo "Status: Complete" ;;
            failed) echo "Status: FAILED (check output)" ;;
            killed) echo "Status: Killed (session ended)" ;;
        esac

        echo "Directory: $DIRECTORY"

        if [ -n "$CREATED" ] && [ "$CREATED" != "null" ]; then
            echo "Created: $CREATED"
            echo "Runtime: $(calculate_runtime "$CREATED")"
        fi

        echo "Handover: $WITH_HANDOVER"

        if [ "$WITH_WORKTREE" = "true" ]; then
            echo "Worktree: Yes"
            [ -n "$WORKTREE_BRANCH" ] && [ "$WORKTREE_BRANCH" != "null" ] && echo "Branch: $WORKTREE_BRANCH"
        fi

        echo ""
        echo "Commands:"
        echo "  Attach:  tmux attach -t $SESSION"
        echo "  Output:  tmux capture-pane -t $SESSION -p -S -50"
        echo "  Kill:    tmux kill-session -t $SESSION"

        if [ "$WITH_WORKTREE" = "true" ]; then
            echo "  Cleanup: bash \"$SKILL_DIR/scripts/cleanup.sh\" $SESSION --merge"
        else
            echo "  Cleanup: bash \"$SKILL_DIR/scripts/cleanup.sh\" $SESSION"
        fi

        echo ""

        # Show last few lines of output
        if [ "$STATUS" != "killed" ]; then
            echo "Recent Output:"
            echo "---"
            tmux capture-pane -t "$SESSION" -p -S -10 2>/dev/null | tail -5 || echo "(unable to capture)"
            echo "---"
        fi

        echo ""
        INDEX=$((INDEX + 1))

    done <<< "$SESSIONS"
}

output_json() {
    local SESSIONS=$1

    echo "{"
    echo "  \"agents\": ["

    local FIRST=true
    while IFS= read -r SESSION; do
        [ -z "$SESSION" ] && continue

        [ "$FIRST" = false ] && echo ","
        FIRST=false

        local STATUS=$(detect_agent_status "$SESSION")
        local METADATA=$(get_agent_metadata "$SESSION")

        local TASK=$(echo "$METADATA" | jq -r '.task // "unknown"' 2>/dev/null)
        local DIRECTORY=$(echo "$METADATA" | jq -r '.directory // "unknown"' 2>/dev/null)
        local CREATED=$(echo "$METADATA" | jq -r '.created // null' 2>/dev/null)
        local WITH_WORKTREE=$(echo "$METADATA" | jq -r '.with_worktree // false' 2>/dev/null)
        local WORKTREE_BRANCH=$(echo "$METADATA" | jq -r '.worktree_branch // null' 2>/dev/null)

        echo "    {"
        echo "      \"session\": \"$SESSION\","
        echo "      \"task\": \"$TASK\","
        echo "      \"status\": \"$STATUS\","
        echo "      \"directory\": \"$DIRECTORY\","
        echo "      \"created\": $CREATED,"
        echo "      \"with_worktree\": $WITH_WORKTREE,"
        echo "      \"worktree_branch\": $WORKTREE_BRANCH"
        echo -n "    }"

    done <<< "$SESSIONS"

    echo ""
    echo "  ],"
    echo "  \"summary\": {"

    local TOTAL=$(echo "$SESSIONS" | grep -c "^agent-" || echo "0")
    echo "    \"total\": $TOTAL"

    echo "  }"
    echo "}"
}

# Main

# Get sessions to check
if [ -n "$SPECIFIC_SESSION" ]; then
    if tmux has-session -t "$SPECIFIC_SESSION" 2>/dev/null; then
        SESSIONS="$SPECIFIC_SESSION"
    else
        echo "Session not found: $SPECIFIC_SESSION"
        exit 1
    fi
else
    SESSIONS=$(get_agent_sessions)
fi

# Output based on mode
case "$OUTPUT_MODE" in
    compact)
        output_compact "$SESSIONS"
        ;;
    detailed)
        output_detailed "$SESSIONS"
        ;;
    json)
        output_json "$SESSIONS"
        ;;
esac
