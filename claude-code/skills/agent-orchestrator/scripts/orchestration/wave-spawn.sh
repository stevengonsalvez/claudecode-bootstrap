#!/bin/bash

# ABOUTME: Spawns all agents in a DAG wave in parallel
# Part of the agent-orchestrator skill - orchestration mode

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(dirname "$SCRIPT_DIR")/core"

# Parse arguments
SESSION_ID="${1:-}"
WAVE_NUMBER="${2:-}"

if [ -z "$SESSION_ID" ] || [ -z "$WAVE_NUMBER" ]; then
    echo "Usage: wave-spawn.sh <session-id> <wave-number>"
    echo ""
    echo "Spawns all agents in the specified wave of the DAG."
    echo ""
    echo "Arguments:"
    echo "  session-id    Orchestration session ID"
    echo "  wave-number   Wave number to spawn (1-based)"
    echo ""
    echo "Example:"
    echo "  wave-spawn.sh orch-1705161234 1"
    exit 1
fi

# Load session and DAG
ORCH_DIR="$HOME/.claude/orchestration/state"
SESSION_FILE="$ORCH_DIR/session-${SESSION_ID}.json"
DAG_FILE="$ORCH_DIR/dag-${SESSION_ID}.json"

if [ ! -f "$SESSION_FILE" ]; then
    echo "Error: Session not found: $SESSION_ID"
    echo "Hint: Create session first with session-create.sh"
    exit 1
fi

if [ ! -f "$DAG_FILE" ]; then
    echo "Error: DAG file not found: $DAG_FILE"
    exit 1
fi

# Check if previous waves are complete (if wave > 1)
if [ "$WAVE_NUMBER" -gt 1 ]; then
    PREV_WAVE=$((WAVE_NUMBER - 1))
    PREV_STATUS=$(jq -r --arg w "$PREV_WAVE" \
        '.waves_status[] | select(.wave_number == ($w | tonumber)) | .status' \
        "$SESSION_FILE")

    if [ "$PREV_STATUS" != "complete" ]; then
        echo "Error: Wave $PREV_WAVE is not complete (status: $PREV_STATUS)"
        echo "Hint: Wait for previous wave to complete before spawning next wave"
        exit 1
    fi
fi

# Get nodes in this wave
WAVE_NODES=$(jq -r --arg w "$WAVE_NUMBER" \
    '.waves[] | select(.wave_number == ($w | tonumber)) | .nodes[]' \
    "$DAG_FILE" 2>/dev/null)

if [ -z "$WAVE_NODES" ]; then
    echo "Error: No nodes found in wave $WAVE_NUMBER"
    exit 1
fi

NODE_COUNT=$(echo "$WAVE_NODES" | wc -l | tr -d ' ')

echo "========================================"
echo "Spawning Wave $WAVE_NUMBER"
echo "========================================"
echo ""
echo "Session: $SESSION_ID"
echo "Agents:  $NODE_COUNT"
echo ""

# Update wave status to active
jq --arg w "$WAVE_NUMBER" \
    '(.waves_status[] | select(.wave_number == ($w | tonumber))) |= . + {status: "active", started_at: (now | todate)}' \
    "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

# Update session current wave
jq --arg w "$WAVE_NUMBER" '.current_wave = ($w | tonumber) | .status = "running"' \
    "$SESSION_FILE" > "${SESSION_FILE}.tmp" && mv "${SESSION_FILE}.tmp" "$SESSION_FILE"

# Spawn each agent in parallel
PIDS=()
for NODE_ID in $WAVE_NODES; do
    # Extract node details from DAG
    NODE=$(jq --arg n "$NODE_ID" '.nodes[$n]' "$DAG_FILE")

    TASK=$(echo "$NODE" | jq -r '.task')
    AGENT_TYPE=$(echo "$NODE" | jq -r '.agent_type // "backend-developer"')
    WORKSTREAM=$(echo "$NODE" | jq -r '.workstream_id // $n' --arg n "$NODE_ID")
    DEPENDENCIES=$(echo "$NODE" | jq -c '.dependencies // []')

    echo "Spawning: $NODE_ID ($AGENT_TYPE)"

    # Spawn agent using core spawn.sh with orchestration flags
    bash "$CORE_DIR/spawn.sh" "$TASK" \
        --with-worktree \
        --orchestration-session "$SESSION_ID" \
        --wave "$WAVE_NUMBER" \
        --workstream "$WORKSTREAM" \
        --agent-type "$AGENT_TYPE" \
        --dag-node "$NODE_ID" \
        --dependencies "$DEPENDENCIES" &

    PIDS+=($!)

    # Small delay between spawns to avoid race conditions
    sleep 1
done

echo ""
echo "Waiting for all agents to initialize..."

# Wait for all spawn processes
FAILED=0
for PID in "${PIDS[@]}"; do
    if ! wait "$PID"; then
        FAILED=$((FAILED + 1))
    fi
done

if [ "$FAILED" -gt 0 ]; then
    echo ""
    echo "Warning: $FAILED agent(s) failed to spawn"
fi

echo ""
echo "========================================"
echo "Wave $WAVE_NUMBER Spawned"
echo "========================================"
echo ""
echo "Agents spawned: $((NODE_COUNT - FAILED))/$NODE_COUNT"
echo ""
echo "Next Steps:"
echo "  Monitor wave: bash \"$SCRIPT_DIR/wave-monitor.sh\" $SESSION_ID $WAVE_NUMBER"
echo "  Full status:  bash \"$SCRIPT_DIR/session-status.sh\" $SESSION_ID"
echo ""
echo "To attach to agents:"
for NODE_ID in $WAVE_NODES; do
    WORKSTREAM=$(jq -r --arg n "$NODE_ID" '.nodes[$n].workstream_id // $n' "$DAG_FILE" --arg n "$NODE_ID")
    echo "  tmux attach -t agent-${WORKSTREAM}-*"
done
