#!/bin/bash

# Orchestrator Runner - Multi-Agent DAG Execution Engine
# Executes DAG plans by spawning agents in waves with automated monitoring

set -euo pipefail

# ============================================================================
# Configuration and Paths
# ============================================================================

UTILS_DIR="${HOME}/.claude/utils"
STATE_DIR="${HOME}/.claude/orchestration/state"
CHECKPOINTS_DIR="${HOME}/.claude/orchestration/checkpoints"

# Source utilities
source "${UTILS_DIR}/spawn-agent-lib.sh"
source "${UTILS_DIR}/git-worktree-utils.sh"

# Orchestrator utilities (called as commands, not sourced)
ORCH_STATE="${UTILS_DIR}/orchestrator-state.sh"
ORCH_DAG="${UTILS_DIR}/orchestrator-dag.sh"
ORCH_AGENT="${UTILS_DIR}/orchestrator-agent.sh"

# ============================================================================
# Cross-Platform Helpers
# ============================================================================

# Parse ISO8601 timestamp to epoch seconds (cross-platform)
parse_iso_to_epoch() {
    local ts="$1"
    # Remove timezone suffix for parsing
    local clean_ts="${ts%+*}"
    clean_ts="${clean_ts%Z}"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        date -j -f "%Y-%m-%dT%H:%M:%S" "$clean_ts" +%s 2>/dev/null || date +%s
    else
        # Linux
        date -d "$clean_ts" +%s 2>/dev/null || date +%s
    fi
}

# ============================================================================
# Display and Logging Functions
# ============================================================================

log_info() {
    echo "ℹ️  $*"
}

log_success() {
    echo "✅ $*"
}

log_error() {
    echo "❌ $*" >&2
}

log_warning() {
    echo "⚠️  $*"
}

log_wave() {
    echo ""
    echo "🌊 $*"
    echo ""
}

display_banner() {
    local session_id="$1"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🚀 Multi-Agent Orchestration: $session_id"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
}

display_plan_summary() {
    local dag_file="$1"

    local total_nodes=$(jq -r '.nodes | length' "$dag_file")
    local total_waves=$(jq -r '.waves | length' "$dag_file")
    local budget=$(jq -r '.config.max_budget_usd // 50' "$dag_file")

    echo "📊 Plan Summary:"
    echo "   Total Workstreams: $total_nodes"
    echo "   Total Waves: $total_waves"
    echo "   Budget Limit: \$${budget}"
    echo ""
}

display_wave_status() {
    local session_id="$1"
    local wave_num="$2"

    local session=$(${ORCH_STATE} get "$session_id")
    local agents=$(echo "$session" | jq -r ".waves[] | select(.wave_number == $wave_num) | .agents[]")

    for agent_id in $agents; do
        local agent=$(echo "$session" | jq -r ".agents[\"$agent_id\"]")
        local status=$(echo "$agent" | jq -r '.status')
        local cost=$(echo "$agent" | jq -r '.cost_usd')
        local last_active=$(echo "$agent" | jq -r '.last_active')

        # Calculate time since last active
        local now=$(date +%s)
        local last_ts=$(parse_iso_to_epoch "$last_active")
        local elapsed=$((now - last_ts))

        # Format status with emoji
        local status_emoji="⏸️"
        case "$status" in
            active) status_emoji="🔄" ;;
            complete) status_emoji="✅" ;;
            failed) status_emoji="❌" ;;
            killed) status_emoji="⚰️" ;;
        esac

        printf "  %s %-20s | %s | \$%.2f | %ds ago\n" \
            "$status_emoji" "${agent_id#agent-}" "$status" "$cost" "$elapsed"
    done
}

display_completion_summary() {
    local session_id="$1"
    local session=$(${ORCH_STATE} get "$session_id")

    local total_cost=$(echo "$session" | jq -r '.total_cost_usd')
    local budget=$(echo "$session" | jq -r '.config.max_budget_usd // 50')
    local start_time=$(echo "$session" | jq -r '.created_at')
    local end_time=$(date -Iseconds)

    # Calculate duration
    local start_ts=$(parse_iso_to_epoch "$start_time")
    local end_ts=$(date +%s)
    local duration=$((end_ts - start_ts))
    local minutes=$((duration / 60))
    local seconds=$((duration % 60))

    # Count successful vs failed agents
    local total_agents=$(echo "$session" | jq -r '.agents | length')
    local successful=$(echo "$session" | jq -r '[.agents[] | select(.status == "complete")] | length')
    local failed=$(echo "$session" | jq -r '[.agents[] | select(.status == "failed" or .status == "killed")] | length')

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🎉 Orchestration Complete!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "Summary:"
    echo "  Duration: ${minutes}m ${seconds}s"
    local percent=0
    if [ "$(echo "$budget > 0" | bc)" -eq 1 ]; then
        percent=$(echo "scale=0; ($total_cost * 100) / $budget" | bc)
    fi
    printf "  Cost: \$%.2f / \$%.2f (%d%%)\n" "$total_cost" "$budget" "$percent"
    echo "  Agents Spawned: $total_agents"
    echo "  Successful: $successful"
    echo "  Failed: $failed"
    echo ""

    # Show branches created
    echo "Branches Created:"
    echo "$session" | jq -r '.agents[] | "  - \(.branch) (\(.worktree_dir))"'
    echo ""

    echo "Next Steps:"
    echo "1. Review agent outputs: tmux attach -t <agent-id>"
    echo "2. Merge branches: /merge-agent-work <workstream-id>"
    echo "3. Run integration tests"
    echo "4. Cleanup worktrees: /cleanup-agent-worktree <workstream-id>"
    echo ""
}

# ============================================================================
# Agent Spawning Functions
# ============================================================================

spawn_dag_node() {
    local session_id="$1"
    local node_id="$2"
    local dag_file="$3"
    local wave_num="$4"

    # Get node details from DAG
    local node=$(jq -r ".nodes[\"$node_id\"]" "$dag_file")
    local task=$(echo "$node" | jq -r '.task')
    local agent_type=$(echo "$node" | jq -r '.agent_type // "backend-developer"')
    local workstream_id=$(echo "$node" | jq -r '.workstream_id')
    local deliverables=$(echo "$node" | jq -r '.deliverables // [] | join("\n- ")')

    # Get current branch as base
    local base_branch=$(git branch --show-current 2>/dev/null || echo "main")

    # Create worktree
    local worktree_dir=$(create_agent_worktree "$workstream_id" "$base_branch" "$node_id")
    local branch="feat/$workstream_id"

    # Build comprehensive task prompt
    local full_task="$task

AGENT ROLE: Act as a $agent_type.

CRITICAL REQUIREMENTS:
- Work in worktree: $worktree_dir
- Branch: $branch
- When complete: Run tests, commit with clear message, report status

DELIVERABLES:
- $deliverables

COMPLETION CRITERIA:
1. All deliverables implemented
2. Tests passing
3. Changes committed to branch $branch
4. No uncommitted changes in worktree

When complete: Commit all changes and respond with 'TASK COMPLETE'."

    # Generate unique agent ID
    local agent_id="agent-${workstream_id}-$(date +%s)"

    log_info "Spawning agent: $agent_id"
    log_info "  Workstream: $workstream_id"
    log_info "  Worktree: $worktree_dir"
    log_info "  Branch: $branch"

    # Spawn agent in tmux
    if spawn_agent_tmux "$agent_id" "$worktree_dir" "$full_task" "$agent_type"; then
        # Register agent in orchestration state (expects JSON config)
        local agent_config=$(cat <<EOF
{
  "node_id": "$node_id",
  "worktree_dir": "$worktree_dir",
  "branch": "$branch",
  "agent_type": "$agent_type",
  "status": "active",
  "cost_usd": 0,
  "last_active": "$(date -Iseconds)",
  "created_at": "$(date -Iseconds)"
}
EOF
)
        ${ORCH_STATE} add-agent "$session_id" "$agent_id" "$agent_config"

        # Add agent to the wave's agent list
        ${ORCH_STATE} add-agent-to-wave "$session_id" "$wave_num" "$agent_id"

        log_success "Agent spawned: $agent_id"
        return 0
    else
        log_error "Failed to spawn agent: $agent_id"
        return 1
    fi
}

# ============================================================================
# Monitoring Functions
# ============================================================================

update_agent_status() {
    local session_id="$1"
    local agent_id="$2"

    # Detect status from tmux
    local status=$(${ORCH_AGENT} detect-status "$agent_id" 2>/dev/null || echo "unknown")

    # Extract cost if available
    local cost=$(${ORCH_AGENT} extract-cost "$agent_id" 2>/dev/null || echo "0")

    # Update in state
    ${ORCH_STATE} update-agent "$session_id" "$agent_id" \
        --status "$status" \
        --cost "$cost" \
        --last-active "$(date -Iseconds)"
}

check_idle_timeout() {
    local session_id="$1"
    local agent_id="$2"
    local max_idle_seconds="${3:-900}"  # Default 15 minutes

    # Check if agent has been idle too long
    if ${ORCH_AGENT} check-idle "$agent_id" "$max_idle_seconds" 2>/dev/null; then
        log_warning "Agent $agent_id idle for >${max_idle_seconds}s - killing"
        ${ORCH_AGENT} kill "$agent_id"
        ${ORCH_STATE} update-agent "$session_id" "$agent_id" \
            --status "killed" \
            --last-active "$(date -Iseconds)"
        return 0
    fi
    return 1
}

check_budget_status() {
    local session_id="$1"

    # Get budget check result
    local result=$(${ORCH_STATE} check-budget "$session_id")
    echo "$result"
}

all_agents_in_status() {
    local session_id="$1"
    local wave_num="$2"
    local target_status="$3"

    local session=$(${ORCH_STATE} get "$session_id")
    local agents=$(echo "$session" | jq -r ".waves[] | select(.wave_number == $wave_num) | .agents[]")

    for agent_id in $agents; do
        local status=$(echo "$session" | jq -r ".agents[\"$agent_id\"].status")
        if [ "$status" != "$target_status" ] && [ "$status" != "complete" ]; then
            return 1
        fi
    done
    return 0
}

any_agent_failed() {
    local session_id="$1"
    local wave_num="$2"

    local session=$(${ORCH_STATE} get "$session_id")
    # Get agent IDs from wave, then check their status in the agents object
    local failed=$(echo "$session" | jq -r "
        (.waves[] | select(.wave_number == $wave_num) | .agents) as \$agent_ids |
        [.agents | to_entries[] | select(.key as \$k | \$agent_ids | index(\$k)) | select(.value.status == \"failed\" or .value.status == \"killed\")] | length
    ")

    [ "$failed" -gt 0 ]
}

all_agents_complete() {
    local session_id="$1"
    local wave_num="$2"

    all_agents_in_status "$session_id" "$wave_num" "complete"
}

# ============================================================================
# Wave Execution Functions
# ============================================================================

spawn_wave() {
    local session_id="$1"
    local wave_num="$2"
    local dag_file="$3"
    local max_concurrent="${4:-4}"

    log_wave "Wave $wave_num: Spawning agents..."

    # Get nodes in this wave
    local wave_nodes=$(jq -r ".waves[] | select(.wave_number == $wave_num) | .nodes[]" "$dag_file")

    # Convert to array
    local nodes_array=($wave_nodes)
    local total_nodes=${#nodes_array[@]}

    log_info "Wave $wave_num has $total_nodes agents to spawn"

    # Update wave status
    ${ORCH_STATE} update-wave-status "$session_id" "$wave_num" "active"

    # Spawn in batches if more than max_concurrent
    local spawned=0
    local failed=0

    for node_id in "${nodes_array[@]}"; do
        # Respect max concurrent limit
        local active_count=0
        while true; do
            active_count=$(jobs -r | wc -l | tr -d ' ')
            if [ "$active_count" -lt "$max_concurrent" ]; then
                break
            fi
            sleep 1
        done

        # Spawn agent in background
        (
            if spawn_dag_node "$session_id" "$node_id" "$dag_file" "$wave_num"; then
                exit 0
            else
                exit 1
            fi
        ) &

        spawned=$((spawned + 1))
        sleep 0.5  # Small delay between spawns
    done

    # Wait for all spawning to complete
    wait

    log_success "Wave $wave_num: All agents spawned"
    return 0
}

monitor_wave() {
    local session_id="$1"
    local wave_num="$2"
    local poll_interval="${3:-30}"
    local idle_timeout="${4:-900}"

    log_info "Monitoring Wave $wave_num (poll every ${poll_interval}s)..."
    echo ""

    while true; do
        # Get all agents in this wave
        local session=$(${ORCH_STATE} get "$session_id")
        local agents=$(echo "$session" | jq -r ".waves[] | select(.wave_number == $wave_num) | .agents[]")

        # Update status for each agent
        for agent_id in $agents; do
            update_agent_status "$session_id" "$agent_id"
            check_idle_timeout "$session_id" "$agent_id" "$idle_timeout" || true
        done

        # Check budget
        local budget_status=$(check_budget_status "$session_id")
        if echo "$budget_status" | grep -q "STOP"; then
            log_error "Budget limit exceeded - stopping orchestration"
            return 2
        elif echo "$budget_status" | grep -q "WARN"; then
            log_warning "Budget warning: approaching limit"
        fi

        # Check if all agents complete
        if all_agents_complete "$session_id" "$wave_num"; then
            log_success "Wave $wave_num: All agents complete!"
            ${ORCH_STATE} update-wave-status "$session_id" "$wave_num" "complete"
            return 0
        fi

        # Check for failures
        if any_agent_failed "$session_id" "$wave_num"; then
            log_error "Wave $wave_num: One or more agents failed"
            ${ORCH_STATE} update-wave-status "$session_id" "$wave_num" "failed"
            return 1
        fi

        # Display current status
        echo "📊 Wave $wave_num Status:"
        display_wave_status "$session_id" "$wave_num"
        echo ""

        sleep "$poll_interval"
    done
}

# ============================================================================
# Main Orchestration Loop
# ============================================================================

run_orchestration() {
    local session_id="$1"
    local resume="${2:-false}"
    local from_wave="${3:-1}"

    # Construct DAG file path
    local dag_file="${STATE_DIR}/dag-${session_id}.json"

    # Verify DAG exists
    if [ ! -f "$dag_file" ]; then
        log_error "DAG file not found: $dag_file"
        return 1
    fi

    # Display banner
    display_banner "$session_id"
    display_plan_summary "$dag_file"

    # Get total waves
    local total_waves=$(jq -r '.waves | length' "$dag_file")

    # Determine starting wave
    local start_wave="$from_wave"
    if [ "$resume" = "true" ]; then
        # Get last completed wave
        local last_completed=$(${ORCH_STATE} get-last-completed-wave "$session_id" 2>/dev/null || echo "0")
        start_wave=$((last_completed + 1))
        log_info "Resuming from wave $start_wave"
    else
        # Initialize new session
        log_info "Initializing session state..."
        local tmux_session="orch-${session_id}"
        local config=$(jq -r '.config // {}' "$dag_file")
        ${ORCH_STATE} create "$session_id" "$tmux_session" "$config"

        # Register waves from DAG
        log_info "Registering waves from DAG..."
        for ((w=1; w<=total_waves; w++)); do
            # Get nodes for this wave and convert to agent ID format
            local wave_nodes=$(jq -r ".waves[] | select(.wave_number == $w) | .nodes" "$dag_file")
            # For now, register empty agent list - agents will be added when spawned
            ${ORCH_STATE} add-wave "$session_id" "$w" "[]"
        done
    fi

    # Execute waves sequentially
    for ((wave=start_wave; wave<=total_waves; wave++)); do
        # Spawn agents in wave
        if ! spawn_wave "$session_id" "$wave" "$dag_file" 4; then
            log_error "Failed to spawn wave $wave"
            return 1
        fi

        # Monitor wave until completion
        local monitor_result=0
        monitor_wave "$session_id" "$wave" 30 900 || monitor_result=$?

        # Handle monitor results
        case $monitor_result in
            0)
                # Success - continue to next wave
                log_success "Wave $wave completed successfully"
                ${ORCH_STATE} mark-wave-checkpoint "$session_id" "$wave"
                ;;
            1)
                # Failures detected
                log_error "Wave $wave failed - stopping orchestration"
                return 1
                ;;
            2)
                # Budget exceeded
                log_error "Budget exceeded - stopping orchestration"
                return 2
                ;;
            *)
                log_error "Unknown error in wave $wave"
                return 1
                ;;
        esac
    done

    # All waves complete - archive session
    ${ORCH_STATE} archive "$session_id"

    # Display completion summary
    display_completion_summary "$session_id"

    return 0
}

# ============================================================================
# Entry Point
# ============================================================================

main() {
    local command="${1:-}"

    case "$command" in
        run)
            local session_id="${2:-}"
            local resume=false
            local from_wave=1

            # Parse flags
            shift 2
            while [[ $# -gt 0 ]]; do
                case $1 in
                    --resume)
                        resume=true
                        shift
                        ;;
                    --from-wave)
                        from_wave="$2"
                        shift 2
                        ;;
                    *)
                        shift
                        ;;
                esac
            done

            if [ -z "$session_id" ]; then
                log_error "Session ID required"
                echo "Usage: $0 run <session_id> [--resume] [--from-wave N]"
                exit 1
            fi

            run_orchestration "$session_id" "$resume" "$from_wave"
            ;;
        *)
            echo "Orchestrator Runner - Multi-Agent DAG Execution"
            echo ""
            echo "Usage:"
            echo "  $0 run <session_id>              - Run orchestration from start"
            echo "  $0 run <session_id> --resume     - Resume interrupted orchestration"
            echo "  $0 run <session_id> --from-wave N - Start from specific wave"
            echo ""
            exit 1
            ;;
    esac
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
