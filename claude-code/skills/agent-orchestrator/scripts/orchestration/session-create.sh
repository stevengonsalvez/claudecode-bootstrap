#!/bin/bash

# ABOUTME: Creates an orchestration session from a DAG file
# Part of the agent-orchestrator skill - orchestration mode

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Parse arguments
DAG_FILE="${1:-}"
SESSION_ID="${2:-orch-$(date +%s)}"

if [ -z "$DAG_FILE" ]; then
    echo "Usage: session-create.sh <dag-file> [session-id]"
    echo ""
    echo "Creates an orchestration session from a DAG JSON file."
    echo ""
    echo "Arguments:"
    echo "  dag-file     Path to DAG JSON file (from /m-plan)"
    echo "  session-id   Optional session ID (default: orch-<timestamp>)"
    echo ""
    echo "Example:"
    echo "  session-create.sh plan.json"
    echo "  session-create.sh plan.json orch-my-feature"
    exit 1
fi

# Validate DAG file exists
if [ ! -f "$DAG_FILE" ]; then
    echo "Error: DAG file not found: $DAG_FILE"
    exit 1
fi

# Validate DAG structure
if ! jq -e '.nodes' "$DAG_FILE" > /dev/null 2>&1; then
    echo "Error: Invalid DAG file - missing 'nodes' field"
    exit 1
fi

if ! jq -e '.waves' "$DAG_FILE" > /dev/null 2>&1; then
    echo "Error: Invalid DAG file - missing 'waves' field"
    echo "Hint: Run topological sort first or ensure /m-plan calculated waves"
    exit 1
fi

# Create orchestration state directory
ORCH_DIR="$HOME/.claude/orchestration/state"
mkdir -p "$ORCH_DIR"

# Copy DAG with session ID
DAG_DEST="$ORCH_DIR/dag-${SESSION_ID}.json"
cp "$DAG_FILE" "$DAG_DEST"

# Extract metadata from DAG
TOTAL_NODES=$(jq '.nodes | length' "$DAG_FILE")
TOTAL_WAVES=$(jq '.waves | length' "$DAG_FILE")
TASK_DESC=$(jq -r '.task_description // "Multi-agent orchestration"' "$DAG_FILE")

# Calculate max concurrent (largest wave)
MAX_CONCURRENT=$(jq '[.waves[].nodes | length] | max' "$DAG_FILE")

# Create session state
SESSION_FILE="$ORCH_DIR/session-${SESSION_ID}.json"
cat > "$SESSION_FILE" <<EOF
{
  "session_id": "$SESSION_ID",
  "created_at": "$(date -Iseconds)",
  "task_description": $(echo "$TASK_DESC" | jq -Rs .),
  "status": "pending",
  "current_wave": 0,
  "total_waves": $TOTAL_WAVES,
  "total_nodes": $TOTAL_NODES,
  "max_concurrent": $MAX_CONCURRENT,
  "agents": {},
  "total_cost_usd": 0,
  "dag_file": "$DAG_DEST",
  "waves_status": $(jq '[.waves[] | {wave_number: .wave_number, status: "pending", started_at: null, completed_at: null}]' "$DAG_FILE")
}
EOF

echo "========================================"
echo "Orchestration Session Created"
echo "========================================"
echo ""
echo "Session ID:      $SESSION_ID"
echo "DAG File:        $DAG_DEST"
echo "Session State:   $SESSION_FILE"
echo ""
echo "Summary:"
echo "  Total Workstreams: $TOTAL_NODES"
echo "  Total Waves:       $TOTAL_WAVES"
echo "  Max Concurrent:    $MAX_CONCURRENT"
echo ""
echo "Waves:"
jq -r '.waves[] | "  Wave \(.wave_number): \(.nodes | length) agent(s) - \(.nodes | join(", "))"' "$DAG_FILE"
echo ""
echo "Next Steps:"
echo "  Execute: bash \"$SCRIPT_DIR/wave-spawn.sh\" $SESSION_ID 1"
echo "  Monitor: bash \"$SCRIPT_DIR/session-status.sh\" $SESSION_ID"
echo ""

# Output session ID for programmatic use
echo "SESSION_ID=$SESSION_ID"
