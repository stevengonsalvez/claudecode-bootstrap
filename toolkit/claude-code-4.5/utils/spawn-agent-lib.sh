#!/bin/bash

# Spawn Agent Library
# Core agent spawning functions extracted from spawn-agent.md
# Used by orchestrator-runner.sh for multi-agent orchestration

set -euo pipefail

# Function: Wait for CLI to be ready for input
# Usage: wait_for_cli_ready <session_name> [cli_provider] [timeout]
# cli_provider: claude (default) | codex | gemini
# Returns: 0 on success, 1 on timeout
wait_for_cli_ready() {
    local SESSION=$1
    local CLI_PROVIDER="${2:-claude}"
    local TIMEOUT=${3:-30}
    local START=$(date +%s)

    # Provider-specific ready patterns
    local READY_PATTERN
    case "$CLI_PROVIDER" in
        codex)
            READY_PATTERN="Codex|Ready|Working|>|\\$"
            ;;
        gemini)
            READY_PATTERN="Gemini|Ready|>|\\$"
            ;;
        *)
            READY_PATTERN="Claude Code|Welcome back|──────|Style:|bypass permissions"
            ;;
    esac

    while true; do
        # Capture pane output (suppress errors if session not ready)
        PANE_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")

        # Check for CLI prompt/splash
        if echo "$PANE_OUTPUT" | grep -qE "$READY_PATTERN"; then
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

# Backwards compatibility alias
wait_for_claude_ready() {
    wait_for_cli_ready "$1" "claude" "${2:-30}"
}

# Function: Spawn a CLI agent in tmux
# Usage: spawn_agent_tmux <session_name> <work_dir> <task> [agent_type] [cli_provider]
# cli_provider: claude (default) | codex | gemini
# Returns: 0 on success, 1 on failure
spawn_agent_tmux() {
    local SESSION="$1"
    local WORK_DIR="$2"
    local TASK="$3"
    local AGENT_TYPE="${4:-general-purpose}"  # Optional agent type
    local CLI_PROVIDER="${5:-claude}"  # Optional CLI provider

    # Determine CLI command based on provider
    local CLI_CMD
    local CLI_FLAGS
    case "$CLI_PROVIDER" in
        codex)
            CLI_CMD="codex"
            CLI_FLAGS="--full-auto"
            ;;
        gemini)
            CLI_CMD="gemini"
            CLI_FLAGS=""
            ;;
        *)
            CLI_CMD="claude"
            CLI_FLAGS="--dangerously-skip-permissions"
            ;;
    esac

    # Create tmux session
    tmux new-session -d -s "$SESSION" -c "$WORK_DIR" || return 1

    # Verify session creation
    if ! tmux has-session -t "$SESSION" 2>/dev/null; then
        return 1
    fi

    # Start CLI in the session
    if [ -n "$CLI_FLAGS" ]; then
        tmux send-keys -t "$SESSION" "$CLI_CMD $CLI_FLAGS" C-m
    else
        tmux send-keys -t "$SESSION" "$CLI_CMD" C-m
    fi

    # Wait for CLI to be ready
    if ! wait_for_cli_ready "$SESSION" "$CLI_PROVIDER" 30; then
        # Cleanup on failure
        tmux kill-session -t "$SESSION" 2>/dev/null || true
        return 1
    fi

    # Additional small delay for UI stabilization
    sleep 0.5

    # Send the task (use literal mode for safety with special characters)
    tmux send-keys -t "$SESSION" -l "$TASK"
    tmux send-keys -t "$SESSION" C-m

    # Small delay for CLI to start processing
    sleep 1

    # Verify task was received (provider-agnostic check)
    local CURRENT_OUTPUT=$(tmux capture-pane -t "$SESSION" -p 2>/dev/null || echo "")
    if echo "$CURRENT_OUTPUT" | grep -qiE "error|failed|crash|command not found"; then
        # Error detected
        return 1
    fi

    # Task likely received (different CLIs have different processing indicators)
    return 0
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
