#!/bin/bash

# Spawn Agent Library
# Core agent spawning functions extracted from spawn-agent.md
# Used by orchestrator-runner.sh for multi-agent orchestration

set -euo pipefail

# Function: Wait for Claude Code to be ready for input
# Usage: wait_for_claude_ready <session_name>
# Returns: 0 on success, 1 on timeout
wait_for_claude_ready() {
    local SESSION=$1
    local TIMEOUT=${2:-30}  # Optional timeout parameter, default 30s
    local START=$(date +%s)

    while true; do
        # Capture pane output (suppress errors if session not ready)
        PANE_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")

        # Check for Claude prompt/splash (any of these indicates readiness)
        if echo "$PANE_OUTPUT" | grep -qE "Claude Code|Welcome back|──────|Style:|bypass permissions"; then
            # Verify not in error state
            if ! echo "$PANE_OUTPUT" | grep -qiE "error|crash|failed|command not found"; then
                return 0
            fi
        fi

        # Timeout check
        local ELAPSED=$(($(date +%s) - START))
        if [ $ELAPSED -gt $TIMEOUT ]; then
            # Save debug output
            tmux capture-pane -t "$SESSION" -p > "/tmp/spawn-agent-${SESSION}-failure.log" 2>&1 || true
            return 1
        fi

        sleep 0.2
    done
}

# Function: Spawn a Claude agent in tmux
# Usage: spawn_agent_tmux <session_name> <work_dir> <task> [agent_type]
# Returns: 0 on success, 1 on failure
spawn_agent_tmux() {
    local SESSION="$1"
    local WORK_DIR="$2"
    local TASK="$3"
    local AGENT_TYPE="${4:-general-purpose}"  # Optional agent type

    # Create tmux session
    tmux new-session -d -s "$SESSION" -c "$WORK_DIR" || return 1

    # Verify session creation
    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        return 1
    fi

    # Start Claude Code in the session
    tmux send-keys -t "$SESSION" "claude --dangerously-skip-permissions" C-m

    # Wait for Claude to be ready
    if ! wait_for_claude_ready "$SESSION" 30; then
        # Cleanup on failure
        tmux kill-session -t "$SESSION" 2>/dev/null || true
        return 1
    fi

    # Additional small delay for UI stabilization
    sleep 0.5

    # Send the task (use literal mode for safety with special characters)
    tmux send-keys -t "$SESSION" -l "$TASK"
    tmux send-keys -t "$SESSION" C-m

    # Small delay for Claude to start processing
    sleep 1

    # Verify task was received
    local CURRENT_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")
    if echo "$CURRENT_OUTPUT" | grep -qE "Thought for|Forming|Creating|Implement|⏳|✽|∴"; then
        # Task received and processing
        return 0
    elif echo "$CURRENT_OUTPUT" | grep -qE "error|failed|crash"; then
        # Error detected
        return 1
    else
        # Unable to confirm but likely ok (agent may still be starting)
        return 0
    fi
}

# Function: Send additional message to running agent
# Usage: send_to_agent <session_name> <message>
send_to_agent() {
    local SESSION="$1"
    local MESSAGE="$2"

    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        return 1
    fi

    tmux send-keys -t "$SESSION" -l "$MESSAGE"
    tmux send-keys -t "$SESSION" C-m
    return 0
}

# Function: Get tmux pane output
# Usage: get_agent_output <session_name>
get_agent_output() {
    local SESSION="$1"

    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        return 1
    fi

    tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo ""
}

# Function: Check if agent session is alive
# Usage: is_agent_alive <session_name>
is_agent_alive() {
    local SESSION="$1"
    tmux has-session -t "$SESSION" 2>/dev/null
}

# Function: Kill agent session
# Usage: kill_agent <session_name>
kill_agent() {
    local SESSION="$1"

    if tmux has-session -t "$SESSION" 2>/dev/null; then
        tmux kill-session -t "$SESSION" 2>/dev/null || true
    fi
}
