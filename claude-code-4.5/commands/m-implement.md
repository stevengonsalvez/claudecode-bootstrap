---
description: Multi-agent implementation - Execute DAG in waves with automated monitoring
tags: [orchestration, implementation, multi-agent]
---

# Multi-Agent Implementation (`/m-implement`)

You are now in **multi-agent implementation mode**. Your task is to execute a pre-planned DAG by spawning agents in waves and monitoring their progress.

## Your Role

Act as an **orchestrator** that manages parallel agent execution, monitors progress, and handles failures.

## Prerequisites

1. **DAG file must exist**: `~/.claude/orchestration/state/dag-<session-id>.json`
2. **Session must be created**: Via `/m-plan` or manually
3. **Git worktrees setup**: Project must support git worktrees

## Process

### Step 1: Load DAG and Session

```bash
# Load DAG file
DAG_FILE="~/.claude/orchestration/state/dag-${SESSION_ID}.json"

# Verify DAG exists
if [ ! -f "$DAG_FILE" ]; then
    echo "Error: DAG file not found: $DAG_FILE"
    exit 1
fi

# Load session
SESSION=$(~/.claude/utils/orchestrator-state.sh get "$SESSION_ID")

if [ -z "$SESSION" ]; then
    echo "Error: Session not found: $SESSION_ID"
    exit 1
fi
```

### Step 2: Calculate Waves

```bash
# Get waves from DAG (already calculated in /m-plan)
WAVES=$(jq -r '.waves[] | "\(.wave_number):\(.nodes | join(" "))"' "$DAG_FILE")

# Example output:
# 1:ws-1 ws-3
# 2:ws-2 ws-4
# 3:ws-5
```

### Step 3: Execute Wave-by-Wave

**For each wave:**

```bash
WAVE_NUMBER=1

# Get nodes in this wave
WAVE_NODES=$(echo "$WAVES" | grep "^${WAVE_NUMBER}:" | cut -d: -f2)

echo "🌊 Starting Wave $WAVE_NUMBER: $WAVE_NODES"

# Update wave status
~/.claude/utils/orchestrator-state.sh update-wave-status "$SESSION_ID" "$WAVE_NUMBER" "active"

# Spawn all agents in wave (parallel)
for node in $WAVE_NODES; do
    spawn_agent "$SESSION_ID" "$node" &
done

# Wait for all agents in wave to complete
wait

# Check if wave completed successfully
if wave_all_complete "$SESSION_ID" "$WAVE_NUMBER"; then
    ~/.claude/utils/orchestrator-state.sh update-wave-status "$SESSION_ID" "$WAVE_NUMBER" "complete"
    echo "✅ Wave $WAVE_NUMBER complete"
else
    echo "❌ Wave $WAVE_NUMBER failed"
    exit 1
fi
```

### Step 4: Spawn Agent Function

**Function to spawn a single agent:**

```bash
spawn_agent() {
    local session_id="$1"
    local node_id="$2"

    # Get node details from DAG
    local node=$(jq -r --arg n "$node_id" '.nodes[$n]' "$DAG_FILE")
    local task=$(echo "$node" | jq -r '.task')
    local agent_type=$(echo "$node" | jq -r '.agent_type')
    local workstream_id=$(echo "$node" | jq -r '.workstream_id')

    # Create git worktree
    local worktree_dir="worktrees/${workstream_id}-${node_id}"
    local branch="feat/${workstream_id}"

    git worktree add "$worktree_dir" -b "$branch" 2>/dev/null || git worktree add "$worktree_dir" "$branch"

    # Create tmux session
    local agent_id="agent-${workstream_id}-$(date +%s)"
    tmux new-session -d -s "$agent_id" -c "$worktree_dir"

    # Start Claude in tmux
    tmux send-keys -t "$agent_id" "claude --dangerously-skip-permissions" C-m

    # Wait for Claude to initialize
    wait_for_claude_ready "$agent_id"

    # Send task
    local full_task="$task

AGENT ROLE: Act as a ${agent_type}.

CRITICAL REQUIREMENTS:
- Work in worktree: $worktree_dir
- Branch: $branch
- When complete: Run tests, commit with clear message, report status

DELIVERABLES:
$(echo "$node" | jq -r '.deliverables[]' | sed 's/^/- /')

When complete: Commit all changes and report status."

    tmux send-keys -t "$agent_id" -l "$full_task"
    tmux send-keys -t "$agent_id" C-m

    # Add agent to session state
    local agent_config=$(cat <<EOF
{
  "status": "active",
  "tmux_session": "$agent_id",
  "worktree_dir": "$worktree_dir",
  "branch": "$branch",
  "dependencies": $(echo "$node" | jq '.dependencies'),
  "cost_usd": 0,
  "created_at": "$(date -Iseconds)",
  "last_updated": "$(date -Iseconds)"
}
EOF
)

    ~/.claude/utils/orchestrator-state.sh add-agent "$session_id" "$agent_id" "$agent_config"

    echo "✅ Spawned agent: $agent_id ($workstream_id)"
}
```

### Step 5: Monitor Wave Progress

**Function to check if wave is complete:**

```bash
wave_all_complete() {
    local session_id="$1"
    local wave_number="$2"

    # Get agents in this wave
    local wave_agents=$(jq -r --arg w "$wave_number" \
        '.waves[] | select(.wave_number == ($w | tonumber)).agents[]' \
        "$DAG_FILE")

    # Check status of each agent
    for agent_id in $wave_agents; do
        local status=$(~/.claude/utils/orchestrator-state.sh get-agent "$session_id" "$agent_id" | jq -r '.status')

        if [ "$status" != "complete" ]; then
            return 1  # Not all complete
        fi
    done

    return 0  # All complete
}
```

### Step 6: Monitoring Loop

**While wave is running, monitor agent status:**

```bash
monitor_wave() {
    local session_id="$1"
    local wave_number="$2"

    while true; do
        # Get all agents in wave
        local wave_agents=$(~/.claude/utils/orchestrator-state.sh list-agents "$session_id" | grep "agent-ws")

        for agent_id in $wave_agents; do
            # Get agent's tmux session
            local tmux_session=$(~/.claude/utils/orchestrator-state.sh get-agent "$session_id" "$agent_id" | jq -r '.tmux_session')

            # Detect current status
            local new_status=$(~/.claude/utils/orchestrator-agent.sh detect-status "$tmux_session")

            # Update if changed
            ~/.claude/utils/orchestrator-state.sh update-agent-status "$session_id" "$agent_id" "$new_status"

            # Extract and update cost
            local cost=$(~/.claude/utils/orchestrator-agent.sh extract-cost "$tmux_session")
            ~/.claude/utils/orchestrator-state.sh update-agent-cost "$session_id" "$agent_id" "$cost"

            # Check idle timeout
            local idle=$(~/.claude/utils/orchestrator-agent.sh check-idle "$session_id" "$agent_id" 15)
            if [ "$idle" = "true" ] && [ "$new_status" = "idle" ]; then
                echo "⚠️  Agent $agent_id idle for >15min, killing..."
                ~/.claude/utils/orchestrator-agent.sh kill "$tmux_session"
                ~/.claude/utils/orchestrator-state.sh update-agent-status "$session_id" "$agent_id" "killed"
            fi
        done

        # Check if wave is complete
        if wave_all_complete "$session_id" "$wave_number"; then
            return 0
        fi

        # Check if wave failed
        local failed_count=$(~/.claude/utils/orchestrator-state.sh list-agents "$session_id" | \
            xargs -I {} ~/.claude/utils/orchestrator-state.sh get-agent "$session_id" {} | \
            jq -r 'select(.status == "failed")' | wc -l)

        if [ "$failed_count" -gt 0 ]; then
            echo "❌ Wave $wave_number failed ($failed_count agents failed)"
            return 1
        fi

        # Sleep before next check
        sleep 30
    done
}
```

### Step 7: Handle Completion

**When all waves complete:**

```bash
# Archive session
~/.claude/utils/orchestrator-state.sh archive "$SESSION_ID"

# Print summary
echo "🎉 All waves complete!"
echo ""
echo "Summary:"
echo "  Total Cost: \$$(jq -r '.total_cost_usd' sessions.json)"
echo "  Total Agents: $(jq -r '.agents | length' sessions.json)"
echo "  Duration: <calculate from timestamps>"
echo ""
echo "Next steps:"
echo "  1. Review agent outputs in worktrees"
echo "  2. Merge worktrees to main branch"
echo "  3. Run integration tests"
```

## Output Format

**During execution, display:**

```
🚀 Multi-Agent Implementation: <session-id>

📊 Plan Summary:
  - Total Workstreams: 7
  - Total Waves: 4
  - Max Concurrent: 4

🌊 Wave 1 (2 agents)
  ✅ agent-ws1-xxx (complete) - Cost: $1.86
  ✅ agent-ws3-xxx (complete) - Cost: $0.79
  Duration: 8m 23s

🌊 Wave 2 (2 agents)
  🔄 agent-ws2-xxx (active) - Cost: $0.45
  🔄 agent-ws4-xxx (active) - Cost: $0.38
  Elapsed: 3m 12s

🌊 Wave 3 (1 agent)
  ⏸️  agent-ws5-xxx (pending)

🌊 Wave 4 (2 agents)
  ⏸️  agent-ws6-xxx (pending)
  ⏸️  agent-ws7-xxx (pending)

💰 Total Cost: $3.48 / $50.00 (7%)
⏱️  Total Time: 11m 35s

Press Ctrl+C to pause monitoring (agents continue in background)
```

## Important Notes

- **Non-blocking**: Agents run in background tmux sessions
- **Resumable**: Can exit and resume with `/m-monitor <session-id>`
- **Auto-recovery**: Idle agents are killed automatically
- **Budget limits**: Stops if budget exceeded
- **Parallel execution**: Multiple agents per wave (up to max_concurrent)

## Error Handling

**If agent fails:**
1. Mark agent as "failed"
2. Continue other agents in wave
3. Do not proceed to next wave
4. Present failure summary to user
5. Allow manual retry or skip

**If timeout:**
1. Check if agent is actually running (may be false positive)
2. If truly stuck, kill and mark as failed
3. Offer retry option

## Resume Support

**To resume a paused/stopped session:**

```bash
/m-implement <session-id> --resume
```

**Resume logic:**
1. Load existing session state
2. Determine current wave
3. Check which agents are still running
4. Continue from where it left off

## CLI Options (Future)

```bash
/m-implement <session-id> [options]

Options:
  --resume              Resume from last checkpoint
  --from-wave N         Start from specific wave number
  --dry-run             Show what would be executed
  --max-concurrent N    Override max concurrent agents
  --no-monitoring       Spawn agents and exit (no monitoring loop)
```

## Integration with `/spawn-agent`

This command reuses logic from `~/.claude/commands/spawn-agent.md`:
- Git worktree creation
- Claude initialization detection
- Task sending via tmux

## Exit Conditions

**Success:**
- All waves complete
- All agents have status "complete"
- No failures

**Failure:**
- Any agent has status "failed"
- Budget limit exceeded
- User manually aborts

**Pause:**
- User presses Ctrl+C
- Session state saved
- Agents continue in background
- Resume with `/m-monitor <session-id>`

---

**End of `/m-implement` command**
