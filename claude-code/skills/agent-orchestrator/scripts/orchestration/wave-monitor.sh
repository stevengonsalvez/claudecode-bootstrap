#!/bin/bash

# ABOUTME: Monitors a wave of agents until all complete or one fails
# Part of the agent-orchestrator skill - orchestration mode

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(dirname "$SCRIPT_DIR")/core"

# Parse arguments
SESSION_ID="${1:-}"
WAVE_NUMBER="${2:-}"
TIMEOUT_MINUTES="${3:-120}"
POLL_INTERVAL="${4:-30}"

if [ -z "$SESSION_ID" ] || [ -z "$WAVE_NUMBER" ]; then
    echo "Usage: wave-monitor.sh <session-id> <wave-number> [timeout-minutes] [poll-interval]"
    echo ""
    echo "Monitors all agents in a wave until complete or failed."
    echo ""
    echo "Arguments:"
    echo "  session-id      Orchestration session ID"
    echo "  wave-number     Wave number to monitor"
    echo "  timeout-minutes Max time to wait (default: 120)"
    echo "  poll-interval   Seconds between checks (default: 30)"
    echo ""
    echo "Exit codes:"
    echo "  0  All agents completed successfully"
    echo "  1  One or more agents failed"
    echo "  2  Timeout reached"
    exit 1
fi

# Load session
ORCH_DIR="$HOME/.claude/orchestration/state"
SESSION_FILE="$ORCH_DIR/session-${SESSION_ID}.json"
DAG_FILE="$ORCH_DIR/dag-${SESSION_ID}.json"

if [ ! -f "$SESSION_FILE" ]; then
    echo "Error: Session not found: $SESSION_ID"
    exit 1
fi

# Get wave agents from session state
get_wave_agents() {
    jq -r --arg w "$WAVE_NUMBER" \
        '.agents | to_entries[] | select(.value.wave == ($w | tonumber)) | .key' \
        "$SESSION_FILE"
}

# Detect agent status from tmux
detect_agent_status() {
    local AGENT_SESSION=$1

    if ! tmux has-session -t "$AGENT_SESSION" 2>/dev/null; then
        echo "killed"
        return 0
    fi

    local OUTPUT=$(tmux capture-pane -t "$AGENT_SESSION" -p -S -100 2>/dev/null || echo "")

    # Check for explicit completion signal
    if echo "$OUTPUT" | grep -qE "TASK COMPLETE|All tasks complete|Successfully completed"; then
        echo "complete"
        return 0
    fi

    # Check for commit (strong indicator of completion)
    if echo "$OUTPUT" | grep -qE "committed|Commit.*created|\[.*\].*commit"; then
        # Also check if at prompt (truly done)
        local LAST_LINES=$(echo "$OUTPUT" | tail -5)
        if echo "$LAST_LINES" | grep -qE "^>|Human:|Style:"; then
            echo "complete"
            return 0
        fi
    fi

    # Check for failure indicators
    if echo "$OUTPUT" | grep -qiE "fatal error|FAILED|Error:.*cannot|panic:"; then
        echo "failed"
        return 0
    fi

    # Check for idle (at prompt, waiting)
    local LAST_LINES=$(echo "$OUTPUT" | tail -5)
    if echo "$LAST_LINES" | grep -qE "^>|Human:|Style:|bypass permissions"; then
        echo "idle"
        return 0
    fi

    echo "active"
}

# Update agent status in session state
update_agent_status() {
    local AGENT_SESSION=$1
    local STATUS=$2

    jq --arg a "$AGENT_SESSION" --arg s "$STATUS" \
        '.agents[$a].status = $s | .agents[$a].last_updated = (now | todate)' \
        "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"
}

# Extract cost from tmux (if visible)
extract_cost() {
    local AGENT_SESSION=$1
    local OUTPUT=$(tmux capture-pane -t "$AGENT_SESSION" -p -S -50 2>/dev/null || echo "")
    local COST=$(echo "$OUTPUT" | grep -oE 'Cost:\s*\$[0-9]+\.[0-9]{2}' | tail -1 | grep -oE '[0-9]+\.[0-9]{2}' || echo "0.00")
    echo "$COST"
}

echo "========================================"
echo "Monitoring Wave $WAVE_NUMBER"
echo "========================================"
echo ""
echo "Session: $SESSION_ID"
echo "Timeout: ${TIMEOUT_MINUTES}m"
echo "Poll:    ${POLL_INTERVAL}s"
echo ""

START_TIME=$(date +%s)
TIMEOUT_SECONDS=$((TIMEOUT_MINUTES * 60))

while true; do
    AGENTS=$(get_wave_agents)

    if [ -z "$AGENTS" ]; then
        echo "Warning: No agents found for wave $WAVE_NUMBER"
        echo "Hint: Run wave-spawn.sh first"
        exit 1
    fi

    # Count statuses
    COMPLETE=0
    FAILED=0
    ACTIVE=0
    IDLE=0
    TOTAL=0

    echo "----------------------------------------"
    echo "$(date '+%H:%M:%S') - Status Check"
    echo "----------------------------------------"

    for AGENT_SESSION in $AGENTS; do
        TOTAL=$((TOTAL + 1))
        STATUS=$(detect_agent_status "$AGENT_SESSION")
        COST=$(extract_cost "$AGENT_SESSION")

        # Update session state
        update_agent_status "$AGENT_SESSION" "$STATUS"

        # Update cost
        jq --arg a "$AGENT_SESSION" --arg c "$COST" \
            '.agents[$a].cost_usd = ($c | tonumber)' \
            "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

        # Display status
        case $STATUS in
            complete)
                echo "  [DONE] $AGENT_SESSION (\$$COST)"
                COMPLETE=$((COMPLETE + 1))
                ;;
            failed)
                echo "  [FAIL] $AGENT_SESSION (\$$COST)"
                FAILED=$((FAILED + 1))
                ;;
            active)
                echo "  [....] $AGENT_SESSION (\$$COST)"
                ACTIVE=$((ACTIVE + 1))
                ;;
            idle)
                echo "  [IDLE] $AGENT_SESSION (\$$COST)"
                IDLE=$((IDLE + 1))
                ;;
            killed)
                echo "  [KILL] $AGENT_SESSION"
                FAILED=$((FAILED + 1))
                ;;
        esac
    done

    echo ""
    echo "Summary: $COMPLETE complete, $ACTIVE active, $IDLE idle, $FAILED failed (of $TOTAL)"

    # Calculate total cost
    TOTAL_COST=$(jq '[.agents[].cost_usd] | add // 0' "$SESSION_FILE")
    echo "Cost:    \$$TOTAL_COST"

    # Check for completion
    if [ "$COMPLETE" -eq "$TOTAL" ]; then
        echo ""
        echo "========================================"
        echo "Wave $WAVE_NUMBER COMPLETE"
        echo "========================================"

        # Update wave status
        jq --arg w "$WAVE_NUMBER" \
            '(.waves_status[] | select(.wave_number == ($w | tonumber))) |= . + {status: "complete", completed_at: (now | todate)}' \
            "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

        # Check if this was the last wave
        TOTAL_WAVES=$(jq '.total_waves' "$SESSION_FILE")
        if [ "$WAVE_NUMBER" -eq "$TOTAL_WAVES" ]; then
            jq '.status = "complete"' "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"
            echo ""
            echo "All waves complete! Orchestration finished."
            echo ""
            echo "Next Steps:"
            echo "  Review: bash \"$SCRIPT_DIR/session-status.sh\" $SESSION_ID --detailed"
            echo "  Merge:  bash \"$SCRIPT_DIR/merge-waves.sh\" $SESSION_ID"
        else
            NEXT_WAVE=$((WAVE_NUMBER + 1))
            echo ""
            echo "Next wave: $NEXT_WAVE"
            echo "  Spawn: bash \"$SCRIPT_DIR/wave-spawn.sh\" $SESSION_ID $NEXT_WAVE"
        fi

        exit 0
    fi

    # Check for failures
    if [ "$FAILED" -gt 0 ]; then
        echo ""
        echo "========================================"
        echo "Wave $WAVE_NUMBER FAILED"
        echo "========================================"
        echo ""
        echo "$FAILED agent(s) failed. Check their output:"
        for AGENT_SESSION in $AGENTS; do
            STATUS=$(jq -r --arg a "$AGENT_SESSION" '.agents[$a].status' "$SESSION_FILE")
            if [ "$STATUS" = "failed" ] || [ "$STATUS" = "killed" ]; then
                echo "  tmux attach -t $AGENT_SESSION"
            fi
        done

        # Update wave status
        jq --arg w "$WAVE_NUMBER" \
            '(.waves_status[] | select(.wave_number == ($w | tonumber))) |= . + {status: "failed", completed_at: (now | todate)}' \
            "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

        jq '.status = "failed"' "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

        exit 1
    fi

    # Check timeout
    ELAPSED=$(($(date +%s) - START_TIME))
    if [ "$ELAPSED" -gt "$TIMEOUT_SECONDS" ]; then
        echo ""
        echo "========================================"
        echo "Wave $WAVE_NUMBER TIMEOUT"
        echo "========================================"
        echo ""
        echo "Timeout reached after ${TIMEOUT_MINUTES} minutes"
        echo "Active agents may still be running in background"
        echo ""
        echo "Options:"
        echo "  Continue monitoring: bash \"$SCRIPT_DIR/wave-monitor.sh\" $SESSION_ID $WAVE_NUMBER 60"
        echo "  Check status: bash \"$SCRIPT_DIR/session-status.sh\" $SESSION_ID"

        exit 2
    fi

    REMAINING=$((TIMEOUT_SECONDS - ELAPSED))
    echo "Timeout in: $((REMAINING / 60))m $((REMAINING % 60))s"
    echo ""
    echo "Next check in ${POLL_INTERVAL}s... (Ctrl+C to stop monitoring)"
    sleep "$POLL_INTERVAL"
done
