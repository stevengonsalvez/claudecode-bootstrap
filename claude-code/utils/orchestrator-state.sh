#!/bin/bash

# Orchestrator State Management Utility
# Manages sessions.json, completed.json, and DAG state files

set -euo pipefail

# Paths
STATE_DIR="${HOME}/.claude/orchestration/state"
SESSIONS_FILE="${STATE_DIR}/sessions.json"
COMPLETED_FILE="${STATE_DIR}/completed.json"
CONFIG_FILE="${STATE_DIR}/config.json"

# Ensure jq is available
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed. Install with: brew install jq"
    exit 1
fi

# ============================================================================
# Session Management Functions
# ============================================================================

# create_session <session_id> <tmux_session> [config_json]
# Creates a new orchestration session
create_session() {
    local session_id="$1"
    local tmux_session="$2"
    local custom_config="${3:-{}}"

    # Load default config
    local default_config=$(jq -r '.orchestrator' "$CONFIG_FILE")

    # Merge custom config with defaults
    local merged_config=$(echo "$default_config" | jq ". + $custom_config")

    # Create session object
    local session=$(cat <<EOF
{
  "session_id": "$session_id",
  "created_at": "$(date -Iseconds)",
  "status": "active",
  "tmux_session": "$tmux_session",
  "config": $merged_config,
  "agents": {},
  "waves": [],
  "total_cost_usd": 0,
  "metadata": {}
}
EOF
)

    # Add to active sessions
    local updated=$(jq ".active_sessions += [$session] | .last_updated = \"$(date -Iseconds)\"" "$SESSIONS_FILE")
    echo "$updated" > "$SESSIONS_FILE"

    echo "$session_id"
}

# get_session <session_id>
# Retrieves a session by ID
get_session() {
    local session_id="$1"
    jq -r ".active_sessions[] | select(.session_id == \"$session_id\")" "$SESSIONS_FILE"
}

# update_session <session_id> <json_update>
# Updates a session with new data (merges)
update_session() {
    local session_id="$1"
    local update="$2"

    local updated=$(jq \
        --arg id "$session_id" \
        --argjson upd "$update" \
        '(.active_sessions[] | select(.session_id == $id)) |= (. + $upd) | .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"
}

# update_session_status <session_id> <status>
# Updates session status
update_session_status() {
    local session_id="$1"
    local status="$2"

    update_session "$session_id" "{\"status\": \"$status\"}"
}

# archive_session <session_id>
# Moves session from active to completed
archive_session() {
    local session_id="$1"

    # Get session data
    local session=$(get_session "$session_id")

    if [ -z "$session" ]; then
        echo "Error: Session $session_id not found"
        return 1
    fi

    # Mark as complete with end time
    local completed_session=$(echo "$session" | jq ". + {\"completed_at\": \"$(date -Iseconds)\"}")

    # Add to completed sessions
    local updated_completed=$(jq ".completed_sessions += [$completed_session] | .last_updated = \"$(date -Iseconds)\"" "$COMPLETED_FILE")
    echo "$updated_completed" > "$COMPLETED_FILE"

    # Update totals
    local total_cost=$(echo "$completed_session" | jq -r '.total_cost_usd')
    local updated_totals=$(jq \
        --arg cost "$total_cost" \
        '.total_cost_usd += ($cost | tonumber) | .total_agents_spawned += 1' \
        "$COMPLETED_FILE")
    echo "$updated_totals" > "$COMPLETED_FILE"

    # Remove from active sessions
    local updated_active=$(jq \
        --arg id "$session_id" \
        '.active_sessions = [.active_sessions[] | select(.session_id != $id)] | .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")
    echo "$updated_active" > "$SESSIONS_FILE"

    echo "Session $session_id archived"
}

# list_active_sessions
# Lists all active sessions
list_active_sessions() {
    jq -r '.active_sessions[] | .session_id' "$SESSIONS_FILE"
}

# ============================================================================
# Agent Management Functions
# ============================================================================

# add_agent <session_id> <agent_id> <agent_config>
# Adds an agent to a session
add_agent() {
    local session_id="$1"
    local agent_id="$2"
    local agent_config="$3"

    local updated=$(jq \
        --arg sid "$session_id" \
        --arg aid "$agent_id" \
        --argjson cfg "$agent_config" \
        '(.active_sessions[] | select(.session_id == $sid).agents[$aid]) = $cfg | .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"
}

# update_agent_status <session_id> <agent_id> <status>
# Updates an agent's status
update_agent_status() {
    local session_id="$1"
    local agent_id="$2"
    local status="$3"

    local updated=$(jq \
        --arg sid "$session_id" \
        --arg aid "$agent_id" \
        --arg st "$status" \
        '(.active_sessions[] | select(.session_id == $sid).agents[$aid].status) = $st |
         (.active_sessions[] | select(.session_id == $sid).agents[$aid].last_updated) = "'$(date -Iseconds)'" |
         .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"
}

# update_agent_cost <session_id> <agent_id> <cost_usd>
# Updates an agent's cost
update_agent_cost() {
    local session_id="$1"
    local agent_id="$2"
    local cost_usd="$3"

    local updated=$(jq \
        --arg sid "$session_id" \
        --arg aid "$agent_id" \
        --arg cost "$cost_usd" \
        '(.active_sessions[] | select(.session_id == $sid).agents[$aid].cost_usd) = ($cost | tonumber) |
         .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"

    # Update session total cost
    update_session_total_cost "$session_id"
}

# update_session_total_cost <session_id>
# Recalculates and updates session total cost
update_session_total_cost() {
    local session_id="$1"

    local total=$(jq -r \
        --arg sid "$session_id" \
        '(.active_sessions[] | select(.session_id == $sid).agents | to_entries | map(.value.cost_usd // 0) | add) // 0' \
        "$SESSIONS_FILE")

    update_session "$session_id" "{\"total_cost_usd\": $total}"
}

# get_agent <session_id> <agent_id>
# Gets agent data
get_agent() {
    local session_id="$1"
    local agent_id="$2"

    jq -r \
        --arg sid "$session_id" \
        --arg aid "$agent_id" \
        '.active_sessions[] | select(.session_id == $sid).agents[$aid]' \
        "$SESSIONS_FILE"
}

# list_agents <session_id>
# Lists all agents in a session
list_agents() {
    local session_id="$1"

    jq -r \
        --arg sid "$session_id" \
        '.active_sessions[] | select(.session_id == $sid).agents | keys[]' \
        "$SESSIONS_FILE"
}

# ============================================================================
# Wave Management Functions
# ============================================================================

# add_wave <session_id> <wave_number> <agent_ids_array>
# Adds a wave to the session
add_wave() {
    local session_id="$1"
    local wave_number="$2"
    local agent_ids="$3"  # JSON array like '["agent-1", "agent-2"]'

    local wave=$(cat <<EOF
{
  "wave_number": $wave_number,
  "status": "pending",
  "agents": $agent_ids,
  "started_at": null,
  "completed_at": null
}
EOF
)

    local updated=$(jq \
        --arg sid "$session_id" \
        --argjson wave "$wave" \
        '(.active_sessions[] | select(.session_id == $sid).waves) += [$wave] | .last_updated = "'$(date -Iseconds)'"' \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"
}

# update_wave_status <session_id> <wave_number> <status>
# Updates wave status
update_wave_status() {
    local session_id="$1"
    local wave_number="$2"
    local status="$3"

    local timestamp_field=""
    if [ "$status" = "active" ]; then
        timestamp_field="started_at"
    elif [ "$status" = "complete" ] || [ "$status" = "failed" ]; then
        timestamp_field="completed_at"
    fi

    local jq_filter='(.active_sessions[] | select(.session_id == $sid).waves[] | select(.wave_number == ($wn | tonumber)).status) = $st'

    if [ -n "$timestamp_field" ]; then
        jq_filter="$jq_filter | (.active_sessions[] | select(.session_id == \$sid).waves[] | select(.wave_number == (\$wn | tonumber)).$timestamp_field) = \"$(date -Iseconds)\""
    fi

    jq_filter="$jq_filter | .last_updated = \"$(date -Iseconds)\""

    local updated=$(jq \
        --arg sid "$session_id" \
        --arg wn "$wave_number" \
        --arg st "$status" \
        "$jq_filter" \
        "$SESSIONS_FILE")

    echo "$updated" > "$SESSIONS_FILE"
}

# get_current_wave <session_id>
# Gets the current active or next pending wave number
get_current_wave() {
    local session_id="$1"

    # First check for active waves
    local active_wave=$(jq -r \
        --arg sid "$session_id" \
        '.active_sessions[] | select(.session_id == $sid).waves[] | select(.status == "active") | .wave_number' \
        "$SESSIONS_FILE" | head -1)

    if [ -n "$active_wave" ]; then
        echo "$active_wave"
        return
    fi

    # Otherwise get first pending wave
    local pending_wave=$(jq -r \
        --arg sid "$session_id" \
        '.active_sessions[] | select(.session_id == $sid).waves[] | select(.status == "pending") | .wave_number' \
        "$SESSIONS_FILE" | head -1)

    echo "${pending_wave:-0}"
}

# ============================================================================
# Utility Functions
# ============================================================================

# check_budget_limit <session_id>
# Checks if session is within budget limits
check_budget_limit() {
    local session_id="$1"

    local max_budget=$(jq -r '.resource_limits.max_budget_usd' "$CONFIG_FILE")
    local warn_percent=$(jq -r '.resource_limits.warn_at_percent' "$CONFIG_FILE")
    local stop_percent=$(jq -r '.resource_limits.hard_stop_at_percent' "$CONFIG_FILE")

    local current_cost=$(jq -r \
        --arg sid "$session_id" \
        '.active_sessions[] | select(.session_id == $sid).total_cost_usd' \
        "$SESSIONS_FILE")

    local percent=$(echo "scale=2; ($current_cost / $max_budget) * 100" | bc)

    if (( $(echo "$percent >= $stop_percent" | bc -l) )); then
        echo "STOP"
        return 1
    elif (( $(echo "$percent >= $warn_percent" | bc -l) )); then
        echo "WARN"
        return 0
    else
        echo "OK"
        return 0
    fi
}

# pretty_print_session <session_id>
# Pretty prints a session
pretty_print_session() {
    local session_id="$1"
    get_session "$session_id" | jq '.'
}

# ============================================================================
# Main CLI Interface
# ============================================================================

case "${1:-}" in
    create)
        create_session "$2" "$3" "${4:-{}}"
        ;;
    get)
        get_session "$2"
        ;;
    update)
        update_session "$2" "$3"
        ;;
    archive)
        archive_session "$2"
        ;;
    list)
        list_active_sessions
        ;;
    add-agent)
        add_agent "$2" "$3" "$4"
        ;;
    update-agent-status)
        update_agent_status "$2" "$3" "$4"
        ;;
    update-agent-cost)
        update_agent_cost "$2" "$3" "$4"
        ;;
    get-agent)
        get_agent "$2" "$3"
        ;;
    list-agents)
        list_agents "$2"
        ;;
    add-wave)
        add_wave "$2" "$3" "$4"
        ;;
    update-wave-status)
        update_wave_status "$2" "$3" "$4"
        ;;
    get-current-wave)
        get_current_wave "$2"
        ;;
    check-budget)
        check_budget_limit "$2"
        ;;
    print)
        pretty_print_session "$2"
        ;;
    *)
        echo "Usage: orchestrator-state.sh <command> [args...]"
        echo ""
        echo "Commands:"
        echo "  create <session_id> <tmux_session> [config_json]"
        echo "  get <session_id>"
        echo "  update <session_id> <json_update>"
        echo "  archive <session_id>"
        echo "  list"
        echo "  add-agent <session_id> <agent_id> <agent_config>"
        echo "  update-agent-status <session_id> <agent_id> <status>"
        echo "  update-agent-cost <session_id> <agent_id> <cost_usd>"
        echo "  get-agent <session_id> <agent_id>"
        echo "  list-agents <session_id>"
        echo "  add-wave <session_id> <wave_number> <agent_ids_json_array>"
        echo "  update-wave-status <session_id> <wave_number> <status>"
        echo "  get-current-wave <session_id>"
        echo "  check-budget <session_id>"
        echo "  print <session_id>"
        exit 1
        ;;
esac
