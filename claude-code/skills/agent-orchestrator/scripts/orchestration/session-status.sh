#!/bin/bash

# ABOUTME: Shows comprehensive status of an orchestration session
# Part of the agent-orchestrator skill - orchestration mode

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Parse arguments
SESSION_ID="${1:-}"
DETAILED=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --detailed|-d)
            DETAILED=true
            shift
            ;;
        --json|-j)
            JSON_OUTPUT=true
            shift
            ;;
        --list|-l)
            LIST_SESSIONS=true
            shift
            ;;
        *)
            if [ -z "$SESSION_ID" ] || [[ "$1" != -* ]]; then
                SESSION_ID="$1"
            fi
            shift
            ;;
    esac
done

ORCH_DIR="$HOME/.claude/orchestration/state"

# List all sessions if requested
if [ "${LIST_SESSIONS:-false}" = true ]; then
    echo "Orchestration Sessions:"
    echo ""

    if [ ! -d "$ORCH_DIR" ]; then
        echo "  (none)"
        exit 0
    fi

    for SESSION_FILE in "$ORCH_DIR"/session-*.json; do
        [ -f "$SESSION_FILE" ] || continue

        SID=$(jq -r '.session_id' "$SESSION_FILE")
        STATUS=$(jq -r '.status' "$SESSION_FILE")
        CREATED=$(jq -r '.created_at' "$SESSION_FILE")
        WAVES=$(jq -r '.total_waves' "$SESSION_FILE")
        CURRENT=$(jq -r '.current_wave' "$SESSION_FILE")
        COST=$(jq -r '.total_cost_usd // 0' "$SESSION_FILE")

        case $STATUS in
            pending)  STATUS_ICON="[ ]" ;;
            running)  STATUS_ICON="[>]" ;;
            complete) STATUS_ICON="[x]" ;;
            failed)   STATUS_ICON="[!]" ;;
            *)        STATUS_ICON="[?]" ;;
        esac

        echo "  $STATUS_ICON $SID"
        echo "      Waves: $CURRENT/$WAVES | Cost: \$$COST | Created: $CREATED"
    done

    exit 0
fi

if [ -z "$SESSION_ID" ]; then
    echo "Usage: session-status.sh <session-id> [options]"
    echo ""
    echo "Shows status of an orchestration session."
    echo ""
    echo "Options:"
    echo "  --detailed, -d   Show detailed agent information"
    echo "  --json, -j       Output as JSON"
    echo "  --list, -l       List all sessions"
    echo ""
    echo "Examples:"
    echo "  session-status.sh --list"
    echo "  session-status.sh orch-1705161234"
    echo "  session-status.sh orch-1705161234 --detailed"
    exit 1
fi

# Load session
SESSION_FILE="$ORCH_DIR/session-${SESSION_ID}.json"
DAG_FILE="$ORCH_DIR/dag-${SESSION_ID}.json"

if [ ! -f "$SESSION_FILE" ]; then
    echo "Error: Session not found: $SESSION_ID"
    echo "Hint: Use --list to see available sessions"
    exit 1
fi

# JSON output
if [ "${JSON_OUTPUT:-false}" = true ]; then
    jq '.' "$SESSION_FILE"
    exit 0
fi

# Load session data
STATUS=$(jq -r '.status' "$SESSION_FILE")
CREATED=$(jq -r '.created_at' "$SESSION_FILE")
TASK_DESC=$(jq -r '.task_description' "$SESSION_FILE")
TOTAL_WAVES=$(jq -r '.total_waves' "$SESSION_FILE")
CURRENT_WAVE=$(jq -r '.current_wave' "$SESSION_FILE")
TOTAL_NODES=$(jq -r '.total_nodes' "$SESSION_FILE")
TOTAL_COST=$(jq -r '[.agents[].cost_usd] | add // 0' "$SESSION_FILE")

# Header
echo "========================================"
echo "Orchestration: $SESSION_ID"
echo "========================================"
echo ""
echo "Task: $TASK_DESC"
echo ""

# Status with icon
case $STATUS in
    pending)  echo "Status: [ ] Pending" ;;
    running)  echo "Status: [>] Running (Wave $CURRENT_WAVE/$TOTAL_WAVES)" ;;
    complete) echo "Status: [x] Complete" ;;
    failed)   echo "Status: [!] Failed" ;;
esac

echo "Created: $CREATED"
echo "Cost:    \$$TOTAL_COST"
echo ""

# Waves summary
echo "Waves:"
jq -r '.waves_status[] |
    if .status == "pending" then "  [ ] Wave \(.wave_number)"
    elif .status == "active" then "  [>] Wave \(.wave_number) (running since \(.started_at))"
    elif .status == "complete" then "  [x] Wave \(.wave_number) (completed \(.completed_at))"
    elif .status == "failed" then "  [!] Wave \(.wave_number) (failed)"
    else "  [?] Wave \(.wave_number)"
    end' "$SESSION_FILE"

echo ""

# Agents summary
AGENT_COUNT=$(jq '.agents | length' "$SESSION_FILE")
if [ "$AGENT_COUNT" -gt 0 ]; then
    echo "Agents ($AGENT_COUNT):"

    if [ "$DETAILED" = true ]; then
        # Detailed agent list
        jq -r '.agents | to_entries[] |
            "  \(.key)"
            + "\n    Status: \(.value.status)"
            + "\n    Wave: \(.value.wave)"
            + "\n    Workstream: \(.value.workstream_id)"
            + "\n    Cost: $\(.value.cost_usd)"
            + "\n    Worktree: \(.value.worktree_dir)"
            + "\n    Last Updated: \(.value.last_updated)"
            + "\n"' "$SESSION_FILE"
    else
        # Compact agent list
        jq -r '.agents | to_entries[] |
            if .value.status == "active" then "  [>] \(.key) (wave \(.value.wave), $\(.value.cost_usd))"
            elif .value.status == "complete" then "  [x] \(.key) (wave \(.value.wave), $\(.value.cost_usd))"
            elif .value.status == "failed" then "  [!] \(.key) (wave \(.value.wave), $\(.value.cost_usd))"
            elif .value.status == "idle" then "  [~] \(.key) (wave \(.value.wave), $\(.value.cost_usd))"
            else "  [ ] \(.key) (wave \(.value.wave))"
            end' "$SESSION_FILE"
    fi
else
    echo "Agents: (none spawned yet)"
fi

echo ""

# Next actions based on status
echo "Actions:"
case $STATUS in
    pending)
        echo "  Start:   bash \"$SCRIPT_DIR/wave-spawn.sh\" $SESSION_ID 1"
        ;;
    running)
        echo "  Monitor: bash \"$SCRIPT_DIR/wave-monitor.sh\" $SESSION_ID $CURRENT_WAVE"
        if [ "$CURRENT_WAVE" -lt "$TOTAL_WAVES" ]; then
            NEXT=$((CURRENT_WAVE + 1))
            echo "  Next:    bash \"$SCRIPT_DIR/wave-spawn.sh\" $SESSION_ID $NEXT"
        fi
        ;;
    complete)
        echo "  Merge:   bash \"$SCRIPT_DIR/merge-waves.sh\" $SESSION_ID"
        echo "  Archive: mv \"$SESSION_FILE\" \"$ORCH_DIR/archive/\""
        ;;
    failed)
        echo "  Retry:   bash \"$SCRIPT_DIR/wave-spawn.sh\" $SESSION_ID $CURRENT_WAVE"
        echo "  Check agents for errors and fix issues"
        ;;
esac

echo ""
