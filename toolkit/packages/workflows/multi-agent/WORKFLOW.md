---
name: multi-agent-workflow
version: 1.0.0
description: DAG-based parallel agent execution for complex tasks with wave-based orchestration
license: Apache-2.0

metadata:
  author: stevengonsalvez
  repository: https://github.com/stevengonsalvez/ai-coder-rules
  category: orchestration
  keywords: [dag, parallel, multi-agent, orchestration, tmux, workstreams]

type: multi-agent
pipeline:
  - command: m-plan
    description: Decompose task into parallel workstreams with dependency DAG
  - command: m-implement
    description: Execute DAG with wave-based agent spawning
  - command: m-monitor
    description: Real-time dashboard of agent progress

execution_modes:
  - name: tmux
    description: Separate Claude CLI processes in tmux sessions (default)
    features: [persistent, independent-tokens, survives-disconnects]
  - name: subagent
    description: Task tool with background execution
    features: [faster-startup, shared-context]

dependencies:
  agents:
    - backend-developer
    - frontend-developer
    - superstar-engineer
  utilities:
    - tmux
    - jq
    - bc
---

# Multi-Agent Workflow

Orchestrate complex tasks by decomposing them into parallel workstreams executed by specialized agents.

## Overview

This workflow enables parallel execution of complex tasks:

1. **Planning Phase** (`/m-plan`): Decompose task into workstreams, identify dependencies, create DAG
2. **Execution Phase** (`/m-implement`): Spawn agents in waves based on DAG dependencies
3. **Monitoring Phase** (`/m-monitor`): Real-time visibility into agent progress and status

## Usage

```
/m-workflow <task-description> [--tmux | --subagent]
```

### Execution Modes

| Aspect | tmux Agents (Default) | Subagents |
|--------|----------------------|-----------|
| Process | Separate Claude CLI | Same Claude process |
| Token Budget | Independent | Shared with parent |
| Persistence | Survives disconnects | Dies with parent |
| Startup Time | ~30s per agent | ~5s per agent |
| Git Worktree | Yes (isolated) | Optional |
| Best For | Large tasks, long-running | Quick coordinated tasks |

## Workflow Commands

### `/m-plan` - Planning Phase

Decomposes a complex task into parallel workstreams:

1. Analyze the task requirements
2. Identify independent workstreams
3. Map dependencies between workstreams
4. Assign specialist agent types
5. Group into execution waves
6. Estimate time and cost

Output: DAG file at `~/.claude/orchestration/state/dag-{SESSION_ID}.json`

### `/m-implement` - Execution Phase

Executes the planned DAG:

1. Load DAG configuration
2. For each wave:
   - Spawn agents for all nodes in wave
   - Monitor progress and costs
   - Wait for wave completion
3. Handle failures and retries
4. Merge results

Options:
- `--resume`: Resume interrupted workflow
- `--from-wave N`: Restart from specific wave

### `/m-monitor` - Monitoring Phase

Real-time dashboard showing:

- Active sessions and their status
- Per-agent costs and duration
- Wave progress
- Budget utilization
- Recovery options for failures

## Architecture

### DAG Structure

```json
{
  "session_id": "orch-1234567890",
  "task": "Implement user authentication",
  "waves": [
    {
      "wave_number": 1,
      "nodes": [
        {
          "id": "ws-1-database",
          "agent_type": "backend-developer",
          "task": "Create user tables",
          "dependencies": []
        }
      ]
    },
    {
      "wave_number": 2,
      "nodes": [
        {
          "id": "ws-2-api",
          "agent_type": "backend-developer",
          "task": "Create auth endpoints",
          "dependencies": ["ws-1-database"]
        }
      ]
    }
  ]
}
```

### Agent Isolation

Each agent operates in an isolated environment:

- **Git Worktree**: Separate working directory (`worktrees/agent-{id}`)
- **Feature Branch**: Independent branch (`feat/{workstream-id}`)
- **Token Budget**: Separate cost tracking per agent

## Configuration

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

## Supporting Commands

- `/spawn-agent` - Spawn single agent in tmux
- `/attach-agent-worktree` - Attach to agent session
- `/merge-agent-work` - Merge agent branch to main
- `/cleanup-agent-worktree` - Remove worktree after merge
- `/list-agent-worktrees` - List all active worktrees
- `/recover-sessions` - Recover orphaned sessions after crash/shutdown

## Utility Scripts

Located in `utils/`:

- `orchestrator-runner.sh` - Main execution driver
- `orchestrator-dag.sh` - DAG parsing and wave extraction
- `orchestrator-state.sh` - State management functions
- `orchestrator-agent.sh` - Agent spawning and monitoring
- `spawn-agent-lib.sh` - Shared agent spawning library (with recovery support)
- `session-registry.sh` - Session registry management and orphan detection
- `git-worktree-utils.sh` - Git worktree management

## Example Usage

### Basic (asks for mode)
```
/m-workflow Implement user authentication with OAuth, JWT tokens, and profile management
```

### Explicit tmux
```
/m-workflow Refactor the payment system to use event sourcing --tmux
```

### Explicit subagent
```
/m-workflow Add form validation to all input components --subagent
```

## Recovery

### On Success
```
Workflow Complete!
Branches Created:
  - feat/ws-1-auth-service
  - feat/ws-2-database-schema
  - feat/ws-3-api-gateway

Next Steps:
1. Review each branch: git log feat/<workstream>
2. Run integration tests
3. Merge branches: /merge-agent-work <workstream-id>
4. Cleanup worktrees: /cleanup-agent-worktree <workstream-id>
```

### On Failure
```
Workflow encountered issues

Failed Workstreams:
  - ws-3-api-gateway: Agent timeout after 15m idle

Recovery Options:
1. Review failed agent: tmux attach -t agent-ws-3-...
2. Resume workflow: /m-implement {SESSION_ID} --resume
3. Restart from wave: /m-implement {SESSION_ID} --from-wave 2
```

## Session Persistence and Recovery

When tmux sessions are killed externally (system shutdown, network disconnect, crash):

1. **Worktrees remain** on disk in `worktrees/agent-{ID}-{SLUG}/`
2. **Agent metadata** persists at `~/.claude/agents/{SESSION}.json`
3. **Transcript path** stored for `claude --resume` capability
4. **Registry log** at `~/.claude/agents/registry.jsonl` tracks all events

### Recovery Workflow

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  System Crash   │────▶│  Worktrees Stay  │────▶│  User Restarts  │
│  (tmux dies)    │     │  Metadata Stays  │     │  Claude Code    │
└─────────────────┘     └──────────────────┘     └────────┬────────┘
                                                          │
                                                          ▼
                                               ┌──────────────────┐
                                               │ Session startup  │
                                               │ detects orphans  │
                                               │ (auto-warning)   │
                                               └────────┬─────────┘
                                                        │
                                                        ▼
                                               ┌──────────────────┐
                                               │/recover-sessions │
                                               │  list | resume   │
                                               │    | cleanup     │
                                               └──────────────────┘
```

### Recovery Commands

```bash
# List orphaned sessions
/recover-sessions list

# Resume session with transcript (continues conversation)
/recover-sessions resume agent-1234567890

# Archive session metadata
/recover-sessions cleanup agent-1234567890

# Archive all orphaned sessions
/recover-sessions cleanup-all
```

### Session Metadata

Each agent session stores enhanced metadata at `~/.claude/agents/{session}.json`:

```json
{
  "session": "agent-1234567890",
  "task": "Implement feature X",
  "directory": "/path/to/worktree",
  "created": "2025-01-24T10:00:00+00:00",
  "status": "running",
  "transcript_path": "~/.claude/projects/{encoded}/session.jsonl",
  "tmux_pid": "12345",
  "with_worktree": true,
  "worktree_branch": "agent/agent-1234567890"
}
```

Archived sessions are moved to `~/.claude/agents/archived/`.
