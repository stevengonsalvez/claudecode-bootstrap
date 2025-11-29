#!/bin/bash

# ABOUTME: Agent spawning script - launches Claude Code agents in isolated tmux sessions
# Supports both simple mode and orchestration mode (integration with /m-plan workflow)
# Part of the agent-orchestrator skill

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
TOOL_DIR="$(dirname "$(dirname "$SKILL_DIR")")"

# Source utilities
source "${TOOL_DIR}/utils/git-worktree-utils.sh"

# Function: Wait for Claude Code to be ready for input
wait_for_claude_ready() {
    local SESSION=$1
    local TIMEOUT=30
    local START=$(date +%s)

    echo "Waiting for Claude to initialize..."

    while true; do
        # Capture pane output (suppress errors if session not ready)
        PANE_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")

        # Check for Claude prompt/splash (any of these indicates readiness)
        if echo "$PANE_OUTPUT" | grep -qE "Claude Code|Welcome back|Style:|bypass permissions"; then
            # Verify not in error state
            if ! echo "$PANE_OUTPUT" | grep -qiE "error|crash|failed|command not found"; then
                echo "Claude initialized successfully"
                return 0
            fi
        fi

        # Timeout check
        local ELAPSED=$(($(date +%s) - START))
        if [ $ELAPSED -gt $TIMEOUT ]; then
            echo "Timeout: Claude did not initialize within ${TIMEOUT}s"
            echo "Capturing debug output..."
            tmux capture-pane -t "$SESSION" -p > "/tmp/spawn-agent-${SESSION}-failure.log" 2>&1
            echo "Debug output saved to /tmp/spawn-agent-${SESSION}-failure.log"
            return 1
        fi

        sleep 0.2
    done
}

# Parse arguments
TASK="${1:-}"
shift || true

# Simple mode flags
WITH_HANDOVER=false
WITH_WORKTREE=false

# Orchestration mode flags
ORCH_SESSION=""
ORCH_WAVE=""
ORCH_WORKSTREAM=""
ORCH_AGENT_TYPE=""
ORCH_DAG_NODE=""
ORCH_DEPENDENCIES=""

# Parse flags
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-handover)
            WITH_HANDOVER=true
            shift
            ;;
        --with-worktree)
            WITH_WORKTREE=true
            shift
            ;;
        # Orchestration flags
        --orchestration-session)
            ORCH_SESSION="$2"
            shift 2
            ;;
        --wave)
            ORCH_WAVE="$2"
            shift 2
            ;;
        --workstream)
            ORCH_WORKSTREAM="$2"
            shift 2
            ;;
        --agent-type)
            ORCH_AGENT_TYPE="$2"
            shift 2
            ;;
        --dag-node)
            ORCH_DAG_NODE="$2"
            shift 2
            ;;
        --dependencies)
            ORCH_DEPENDENCIES="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

if [ -z "$TASK" ]; then
    echo "Task description required"
    echo ""
    echo "Usage: spawn.sh \"task description\" [options]"
    echo ""
    echo "Simple Mode Options:"
    echo "  --with-handover          Include git context in task"
    echo "  --with-worktree          Run in isolated git worktree"
    echo ""
    echo "Orchestration Mode Options (for /m-implement integration):"
    echo "  --orchestration-session  Orchestration session ID"
    echo "  --wave                   Wave number in DAG"
    echo "  --workstream             Workstream ID"
    echo "  --agent-type             Specialized agent type"
    echo "  --dag-node               DAG node ID"
    echo "  --dependencies           JSON array of dependencies"
    exit 1
fi

# Determine if we're in orchestration mode
ORCHESTRATION_MODE=false
if [ -n "$ORCH_SESSION" ]; then
    ORCHESTRATION_MODE=true
    # Orchestration mode implies worktree by default
    WITH_WORKTREE=true
fi

# Generate session info
TASK_ID=$(date +%s)
if [ "$ORCHESTRATION_MODE" = true ] && [ -n "$ORCH_WORKSTREAM" ]; then
    SESSION="agent-${ORCH_WORKSTREAM}-${TASK_ID}"
else
    SESSION="agent-${TASK_ID}"
fi

# Setup working directory (worktree or current)
if [ "$WITH_WORKTREE" = true ]; then
    # Detect transcrypt (informational only)
    if git config --get-regexp '^transcrypt\.' >/dev/null 2>&1; then
        echo "Transcrypt detected - worktree will inherit encryption config"
    fi

    # Get current branch as base
    CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "HEAD")

    # Generate task slug from task description or workstream
    if [ -n "$ORCH_WORKSTREAM" ]; then
        TASK_SLUG="$ORCH_WORKSTREAM"
    else
        TASK_SLUG=$(echo "$TASK" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 -]//g' | tr -s ' ' '-' | cut -c1-40 | sed 's/-$//')
    fi

    # Create worktree with task slug
    echo "Creating isolated git worktree..."
    WORK_DIR=$(create_agent_worktree "$TASK_ID" "$CURRENT_BRANCH" "$TASK_SLUG")
    AGENT_BRANCH="agent/agent-${TASK_ID}"

    echo "Worktree created:"
    echo "   Directory: $WORK_DIR"
    echo "   Branch: $AGENT_BRANCH"
    echo "   Base: $CURRENT_BRANCH"
    echo ""
else
    WORK_DIR=$(pwd)
    AGENT_BRANCH=""
fi

echo "Spawning Claude agent in tmux session..."
echo ""

# Generate handover if requested
HANDOVER_CONTENT=""
if [ "$WITH_HANDOVER" = true ]; then
    echo "Generating handover context..."

    # Get current branch and recent commits
    CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
    RECENT_COMMITS=$(git log --oneline -5 2>/dev/null || echo "No git history")
    GIT_STATUS=$(git status -sb 2>/dev/null || echo "Not a git repo")

    # Create handover content
    HANDOVER_CONTENT=$(cat << EOF

# Handover Context

## Current State
- Branch: $CURRENT_BRANCH
- Directory: $WORK_DIR
- Time: $(date)

## Recent Commits
$RECENT_COMMITS

## Git Status
$GIT_STATUS

## Your Task
$TASK

---
Please review the above context and proceed with the task.
EOF
)

    echo "Handover generated"
    echo ""
fi

# Create tmux session
tmux new-session -d -s "$SESSION" -c "$WORK_DIR"

# Verify session creation
if ! tmux has-session -t "$SESSION" 2>/dev/null; then
    echo "Failed to create tmux session"
    exit 1
fi

echo "Created tmux session: $SESSION"
echo ""

# Start Claude Code in the session
tmux send-keys -t "$SESSION" "claude --dangerously-skip-permissions" C-m

# Wait for Claude to be ready
if ! wait_for_claude_ready "$SESSION"; then
    echo "Failed to start Claude agent - cleaning up..."
    tmux kill-session -t "$SESSION" 2>/dev/null
    exit 1
fi

# Additional small delay for UI stabilization
sleep 0.5

# Send handover context if generated (line-by-line to handle newlines)
if [ "$WITH_HANDOVER" = true ]; then
    echo "Sending handover context to agent..."

    # Send line-by-line to handle multi-line content properly
    echo "$HANDOVER_CONTENT" | while IFS= read -r LINE || [ -n "$LINE" ]; do
        # Use -l flag to send literal text (handles special characters)
        tmux send-keys -t "$SESSION" -l "$LINE"
        tmux send-keys -t "$SESSION" C-m
        sleep 0.05  # Small delay between lines
    done

    # Final Enter to submit
    tmux send-keys -t "$SESSION" C-m
    sleep 0.5
fi

# Build the full task with orchestration context if applicable
FULL_TASK="$TASK"
if [ "$ORCHESTRATION_MODE" = true ]; then
    FULL_TASK="$TASK

ORCHESTRATION CONTEXT:
- Session: $ORCH_SESSION
- Wave: $ORCH_WAVE
- Workstream: $ORCH_WORKSTREAM
- Agent Role: Act as a ${ORCH_AGENT_TYPE:-general-purpose} agent.

CRITICAL REQUIREMENTS:
- Work in worktree: $WORK_DIR
- Branch: $AGENT_BRANCH
- When complete: Run tests, commit with clear message, then output 'TASK COMPLETE' on a new line

When finished, commit all changes and output 'TASK COMPLETE' to signal completion."
fi

# Send the task (use literal mode for safety with special characters)
echo "Sending task to agent..."
tmux send-keys -t "$SESSION" -l "$FULL_TASK"
tmux send-keys -t "$SESSION" C-m

# Small delay for Claude to start processing
sleep 1

# Verify task was received
CURRENT_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")
if echo "$CURRENT_OUTPUT" | grep -qE "Thought for|Forming|Creating|Implement|Planning"; then
    echo "Task received and processing"
elif echo "$CURRENT_OUTPUT" | grep -qE "error|failed|crash"; then
    echo "Warning: Detected error in agent output"
    echo "Last 10 lines of output:"
    tmux capture-pane -t "$SESSION" -p | tail -10
else
    echo "Task sent (unable to confirm receipt - agent may still be starting)"
fi

echo ""
echo "========================================"
echo "Agent spawned successfully!"
echo "========================================"
echo ""
echo "Session: $SESSION"
echo "Task: ${TASK:0:60}$([ ${#TASK} -gt 60 ] && echo '...')"
echo "Directory: $WORK_DIR"
[ -n "$AGENT_BRANCH" ] && echo "Branch: $AGENT_BRANCH"

if [ "$ORCHESTRATION_MODE" = true ]; then
    echo ""
    echo "Orchestration:"
    echo "  Session: $ORCH_SESSION"
    echo "  Wave: $ORCH_WAVE"
    echo "  Workstream: $ORCH_WORKSTREAM"
    [ -n "$ORCH_AGENT_TYPE" ] && echo "  Agent Type: $ORCH_AGENT_TYPE"
fi

echo ""
echo "Commands:"
echo "  Monitor:  tmux attach -t $SESSION"
echo "  Output:   tmux capture-pane -t $SESSION -p -S -50"
echo "  Kill:     tmux kill-session -t $SESSION"
echo ""

# Save metadata
mkdir -p ~/.claude/agents

# Build orchestration JSON block
ORCH_JSON="null"
if [ "$ORCHESTRATION_MODE" = true ]; then
    ORCH_JSON=$(cat <<EOF
{
    "session_id": "$ORCH_SESSION",
    "wave": ${ORCH_WAVE:-0},
    "workstream_id": "${ORCH_WORKSTREAM:-}",
    "dag_node": "${ORCH_DAG_NODE:-}",
    "agent_type": "${ORCH_AGENT_TYPE:-}",
    "dependencies": ${ORCH_DEPENDENCIES:-[]}
}
EOF
)
fi

cat > ~/.claude/agents/${SESSION}.json <<EOF
{
  "session": "$SESSION",
  "task": $(echo "$TASK" | jq -Rs .),
  "directory": "$WORK_DIR",
  "created": "$(date -Iseconds)",
  "status": "running",
  "with_handover": $WITH_HANDOVER,
  "with_worktree": $WITH_WORKTREE,
  "worktree_branch": "$AGENT_BRANCH",
  "orchestration": $ORCH_JSON
}
EOF

# If in orchestration mode, also update orchestration state
if [ "$ORCHESTRATION_MODE" = true ]; then
    ORCH_STATE_FILE="$HOME/.claude/orchestration/state/session-${ORCH_SESSION}.json"
    if [ -f "$ORCH_STATE_FILE" ]; then
        # Add agent to orchestration session
        AGENT_CONFIG=$(cat <<EOF
{
  "status": "active",
  "tmux_session": "$SESSION",
  "worktree_dir": "$WORK_DIR",
  "branch": "$AGENT_BRANCH",
  "wave": ${ORCH_WAVE:-0},
  "workstream_id": "${ORCH_WORKSTREAM:-}",
  "cost_usd": 0,
  "created_at": "$(date -Iseconds)",
  "last_updated": "$(date -Iseconds)"
}
EOF
)
        # Update session state with new agent
        jq --arg agent "$SESSION" --argjson config "$AGENT_CONFIG" \
            '.agents[$agent] = $config' "$ORCH_STATE_FILE" > "${ORCH_STATE_FILE}.tmp" && \
            mv "${ORCH_STATE_FILE}.tmp" "$ORCH_STATE_FILE"
    fi
fi

# Output session ID for programmatic use
echo "AGENT_SESSION=$SESSION"

exit 0
