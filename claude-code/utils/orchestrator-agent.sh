#!/bin/bash

# Agent Lifecycle Management Utility
# Handles agent spawning, status detection, and termination

set -euo pipefail

# Source the spawn-agent logic
SPAWN_AGENT_CMD="${HOME}/.claude/commands/spawn-agent.md"

# detect_agent_status <tmux_session>
# Detects agent status from tmux output
detect_agent_status() {
    local tmux_session="$1"

    if ! tmux has-session -t "$tmux_session" 2>/dev/null; then
        echo "killed"
        return 0
    fi

    local output=$(tmux capture-pane -t "$tmux_session" -p -S -100 2>/dev/null || echo "")

    # Check for completion indicators
    if echo "$output" | grep -qiE "complete|done|finished|✅.*complete"; then
        if echo "$output" | grep -qE "git.*commit|Commit.*created"; then
            echo "complete"
            return 0
        fi
    fi

    # Check for failure indicators
    if echo "$output" | grep -qiE "error|failed|❌|fatal"; then
        echo "failed"
        return 0
    fi

    # Check for idle (no recent activity)
    local last_line=$(echo "$output" | tail -1)
    if echo "$last_line" | grep -qE "^>|^│|^─|Style:|bypass permissions"; then
        echo "idle"
        return 0
    fi

    # Active by default
    echo "active"
}

# check_idle_timeout <session_id> <agent_id> <timeout_minutes>
# Checks if agent has been idle too long
check_idle_timeout() {
    local session_id="$1"
    local agent_id="$2"
    local timeout_minutes="$3"

    # Get agent's last_updated timestamp
    local last_updated=$(~/.claude/utils/orchestrator-state.sh get-agent "$session_id" "$agent_id" | jq -r '.last_updated // empty')

    if [ -z "$last_updated" ]; then
        echo "false"
        return 0
    fi

    local now=$(date +%s)
    local last=$(date -j -f "%Y-%m-%dT%H:%M:%S" "${last_updated:0:19}" +%s 2>/dev/null || echo "$now")
    local diff=$(( (now - last) / 60 ))

    if [ "$diff" -gt "$timeout_minutes" ]; then
        echo "true"
    else
        echo "false"
    fi
}

# kill_agent <tmux_session>
# Kills an agent tmux session
kill_agent() {
    local tmux_session="$1"

    if tmux has-session -t "$tmux_session" 2>/dev/null; then
        tmux kill-session -t "$tmux_session"
        echo "Killed agent session: $tmux_session"
    fi
}

# extract_cost_from_tmux <tmux_session>
# Extracts cost from Claude status bar in tmux
extract_cost_from_tmux() {
    local tmux_session="$1"

    local output=$(tmux capture-pane -t "$tmux_session" -p -S -50 2>/dev/null || echo "")

    # Look for "Cost: $X.XX" pattern
    local cost=$(echo "$output" | grep -oE 'Cost:\s*\$[0-9]+\.[0-9]{2}' | tail -1 | grep -oE '[0-9]+\.[0-9]{2}')

    echo "${cost:-0.00}"
}

case "${1:-}" in
    detect-status)
        detect_agent_status "$2"
        ;;
    check-idle)
        check_idle_timeout "$2" "$3" "$4"
        ;;
    kill)
        kill_agent "$2"
        ;;
    extract-cost)
        extract_cost_from_tmux "$2"
        ;;
    *)
        echo "Usage: orchestrator-agent.sh <command> [args...]"
        echo "Commands:"
        echo "  detect-status <tmux_session>"
        echo "  check-idle <session_id> <agent_id> <timeout_minutes>"
        echo "  kill <tmux_session>"
        echo "  extract-cost <tmux_session>"
        exit 1
        ;;
esac
