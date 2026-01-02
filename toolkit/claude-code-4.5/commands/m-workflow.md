---
description: Unified multi-agent workflow - Plan and execute complex tasks with parallel agents
tags: [orchestration, workflow, multi-agent]
---

# Multi-Agent Workflow (`/m-workflow`)

You are now in **multi-agent workflow mode**. This command orchestrates the complete flow: planning → execution → monitoring → reporting.

## Arguments

```
/m-workflow <task-description> [--tmux | --subagent]
```

- `--tmux`: Use external tmux agents (separate Claude CLI processes) - **DEFAULT**
- `--subagent`: Use internal subagents (Task tool with background execution)

## Execution Flow

### Step 1: Determine Agent Mode

**If no mode flag provided, ask the user:**

Use the AskUserQuestion tool with:
```json
{
  "questions": [{
    "question": "Which agent execution mode should I use for this multi-agent workflow?",
    "header": "Agent Mode",
    "options": [
      {
        "label": "tmux agents (Recommended)",
        "description": "Separate Claude CLI processes in tmux sessions. Persistent, independent token budgets, survives disconnects. Best for large/long tasks."
      },
      {
        "label": "Subagents",
        "description": "Task tool with background execution. Faster startup, shared context, but shared token budget. Best for smaller coordinated tasks."
      }
    ],
    "multiSelect": false
  }]
}
```

**Default to tmux if user doesn't specify.**

### Step 2: Generate Session ID

```bash
SESSION_ID="orch-$(date +%s)"
echo "Session ID: $SESSION_ID"
```

### Step 3: Run Planning Phase

**Invoke `/m-plan` behavior inline:**

1. **Analyze the task** - Break down into parallel workstreams
2. **Identify dependencies** - What must complete before what?
3. **Assign agent types** - Match workstream to specialist agent
4. **Calculate waves** - Group independent workstreams
5. **Estimate costs** - Time and budget projections

**Create DAG file:**
```bash
DAG_FILE="${HOME}/.claude/orchestration/state/dag-${SESSION_ID}.json"
```

**Present plan to user and get approval before proceeding.**

### Step 4: Execute Based on Mode

#### Mode: tmux (Default)

Delegate to orchestrator-runner.sh:

```bash
~/.claude/utils/orchestrator-runner.sh run "$SESSION_ID"
```

This will:
- Spawn agents in tmux sessions
- Each agent gets isolated git worktree
- Monitor via tmux pane output
- Track costs independently

**Monitor with:**
```bash
# View all tmux sessions
tmux ls

# Attach to specific agent
tmux attach -t agent-<workstream-id>-<timestamp>

# Or use /m-monitor
/m-monitor $SESSION_ID
```

#### Mode: subagent

Orchestrate using Task tool directly:

**For each wave:**
1. Get nodes in wave from DAG
2. For each node, spawn a Task agent:

```
Task tool invocation:
- subagent_type: <agent_type from node>
- run_in_background: true
- prompt: <task from node + deliverables + completion criteria>
```

3. Collect task IDs
4. Monitor with TaskOutput (block=false for polling, block=true for completion)
5. When all tasks in wave complete, proceed to next wave

**Subagent task prompt template:**
```
You are working on workstream: {workstream_id}

TASK:
{task}

DELIVERABLES:
{deliverables}

COMPLETION CRITERIA:
1. All deliverables implemented
2. Tests passing
3. Changes committed to branch feat/{workstream_id}
4. Report completion status when done

When complete, summarize what was accomplished.
```

### Step 5: Monitor Progress

**Display real-time status:**

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🚀 Multi-Agent Workflow: {SESSION_ID}
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Mode: {tmux | subagent}
Status: Wave {N} of {TOTAL}

📊 Wave {N} Status:
  🔄 ws-1-auth-service    | active   | $0.45 | 2m ago
  ✅ ws-2-database-schema | complete | $0.32 | 5m ago
  🔄 ws-3-api-gateway     | active   | $0.28 | 1m ago

Budget: $1.05 / $50.00 (2%)
```

### Step 6: Handle Completion

**On Success:**
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎉 Workflow Complete!
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Summary:
  Duration: 45m 23s
  Cost: $4.50 / $50.00 (9%)
  Agents: 6 spawned, 6 successful, 0 failed

Branches Created:
  - feat/ws-1-auth-service
  - feat/ws-2-database-schema
  - feat/ws-3-api-gateway
  - feat/ws-4-user-profiles
  - feat/ws-5-frontend-auth
  - feat/ws-6-integration-tests

Next Steps:
1. Review each branch: git log feat/<workstream>
2. Run integration tests
3. Merge branches: /merge-agent-work <workstream-id>
4. Cleanup worktrees: /cleanup-agent-worktree <workstream-id>
```

**On Failure:**
```
⚠️ Workflow encountered issues

Failed Workstreams:
  - ws-3-api-gateway: Agent timeout after 15m idle

Recovery Options:
1. Review failed agent: tmux attach -t agent-ws-3-...
2. Resume workflow: /m-implement {SESSION_ID} --resume
3. Restart from wave: /m-implement {SESSION_ID} --from-wave 2
```

## Important Behaviors

### Agent Mode Comparison

| Aspect | tmux Agents | Subagents |
|--------|-------------|-----------|
| Process | Separate Claude CLI | Same Claude process |
| Token Budget | Independent | Shared with parent |
| Persistence | Survives disconnects | Dies with parent |
| Startup Time | ~30s per agent | ~5s per agent |
| Git Worktree | Yes (isolated) | Optional |
| Best For | Large tasks, long-running | Quick coordinated tasks |
| Cost Tracking | Per-agent | Bundled |
| Resume | Full support | Limited |

### Default Behaviors

- **Default mode**: tmux (more robust for complex tasks)
- **Max concurrent agents**: 4 (configurable in config.json)
- **Idle timeout**: 15 minutes (kills unresponsive agents)
- **Budget limit**: $50 (configurable)

### Session Persistence

Session ID enables:
- Resume interrupted workflows: `/m-implement {SESSION_ID} --resume`
- Independent plan/execute: `/m-plan` then `/m-implement {SESSION_ID}`
- Monitor any time: `/m-monitor {SESSION_ID}`
- Review completed: `~/.claude/orchestration/state/sessions.json`

## Example Usage

**Basic (asks for mode):**
```
/m-workflow Implement user authentication with OAuth, JWT tokens, and profile management
```

**Explicit tmux:**
```
/m-workflow Refactor the payment system to use event sourcing --tmux
```

**Explicit subagent:**
```
/m-workflow Add form validation to all input components --subagent
```

## Config File

Located at `~/.claude/orchestration/state/config.json`:

```json
{
  "orchestrator": {
    "max_concurrent_agents": 4,
    "poll_interval_seconds": 30,
    "idle_timeout_seconds": 900,
    "default_agent_mode": "tmux"
  },
  "resource_limits": {
    "max_budget_usd": 50,
    "warn_at_percent": 70,
    "hard_stop_at_percent": 90
  }
}
```

---

**End of `/m-workflow` command**
