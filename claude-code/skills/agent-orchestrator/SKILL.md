---
name: agent-orchestrator
description: Spawn, monitor, and manage Claude Code agents in parallel tmux sessions. Supports simple ad-hoc agents and complex DAG-based multi-agent orchestration with wave execution.
version: 2.0.0
---

# Agent Orchestrator Skill

## Purpose

Provide comprehensive management of Claude Code agents running in parallel tmux sessions. This skill supports two modes:

1. **Simple Mode**: Ad-hoc agent spawning for quick parallel tasks
2. **Orchestration Mode**: DAG-based multi-agent execution with dependencies and waves

## Directory Structure

```
agent-orchestrator/
├── SKILL.md                          # This file
└── scripts/
    ├── core/                         # Core agent management
    │   ├── spawn.sh                  # Spawn single agent
    │   ├── status.sh                 # Check agent status
    │   └── cleanup.sh                # Clean up agents
    └── orchestration/                # DAG-based orchestration
        ├── session-create.sh         # Create orchestration session
        ├── wave-spawn.sh             # Spawn wave of agents
        ├── wave-monitor.sh           # Monitor wave progress
        ├── session-status.sh         # Full session status
        └── merge-waves.sh            # Merge completed worktrees
```

## When to Use This Skill

### Simple Mode
- Quick parallel tasks without dependencies
- Background research while working on something else
- Code review while implementing
- Single isolated experiments

### Orchestration Mode
- Complex features with multiple dependent workstreams
- Multi-team parallel development
- Large refactoring with clear phases
- Integration with `/m-plan` workflow

---

# Simple Mode

## Capabilities

1. **Agent Spawning**: Launch Claude agents in isolated tmux sessions
2. **Worktree Isolation**: Optionally run agents in separate git worktrees
3. **Status Monitoring**: Real-time visibility into running agents
4. **Lifecycle Management**: Clean up completed agents

## Usage

### Spawn an Agent

```bash
SKILL_DIR="${TOOL_DIR}/skills/agent-orchestrator/scripts"

# Basic spawn
bash "$SKILL_DIR/core/spawn.sh" "implement user authentication"

# With handover context (passes git status, recent commits, etc.)
bash "$SKILL_DIR/core/spawn.sh" "refactor the API layer" --with-handover

# With git worktree isolation (agent works on separate branch)
bash "$SKILL_DIR/core/spawn.sh" "implement feature X" --with-worktree

# With both
bash "$SKILL_DIR/core/spawn.sh" "implement caching" --with-handover --with-worktree
```

### Check Status

```bash
# Quick overview
bash "$SKILL_DIR/core/status.sh"

# Detailed status
bash "$SKILL_DIR/core/status.sh" --detailed

# JSON output
bash "$SKILL_DIR/core/status.sh" --json

# Specific agent
bash "$SKILL_DIR/core/status.sh" agent-1705161234
```

### Clean Up

```bash
# Clean specific agent
bash "$SKILL_DIR/core/cleanup.sh" agent-1705161234

# Merge worktree before cleanup
bash "$SKILL_DIR/core/cleanup.sh" agent-1705161234 --merge

# Clean all completed agents
bash "$SKILL_DIR/core/cleanup.sh" --all-completed
```

## Simple Mode Example

```bash
# Spawn reviewer while you implement
bash "$SKILL_DIR/core/spawn.sh" "review src/auth/ for security issues" --with-handover

# Continue working in main session...

# Check reviewer status
bash "$SKILL_DIR/core/status.sh"

# Get reviewer output
tmux capture-pane -t agent-xxx -p > review-results.md

# Clean up
bash "$SKILL_DIR/core/cleanup.sh" agent-xxx
```

---

# Orchestration Mode

## Integration with /m-plan Workflow

The orchestration mode integrates with `/m-plan` and `/m-implement` commands:

```
/m-plan → DAG file → session-create.sh → wave-spawn.sh → wave-monitor.sh → merge-waves.sh
```

## Capabilities

1. **Session Management**: Create and track orchestration sessions
2. **Wave Execution**: Spawn agents respecting DAG dependencies
3. **Progress Monitoring**: Real-time status across all agents
4. **Cost Tracking**: Track Claude API costs per agent and total
5. **Failure Handling**: Detect failures and pause execution
6. **Merge Automation**: Merge completed worktrees systematically

## Usage

### 1. Create Session from DAG

After `/m-plan` creates a DAG file:

```bash
SKILL_DIR="${TOOL_DIR}/skills/agent-orchestrator/scripts"

# Create orchestration session
bash "$SKILL_DIR/orchestration/session-create.sh" ~/.claude/orchestration/state/dag-plan.json

# Output: SESSION_ID=orch-1705161234
```

### 2. Execute Waves

```bash
SESSION_ID="orch-1705161234"

# Spawn Wave 1 (no dependencies)
bash "$SKILL_DIR/orchestration/wave-spawn.sh" $SESSION_ID 1

# Monitor Wave 1 until complete
bash "$SKILL_DIR/orchestration/wave-monitor.sh" $SESSION_ID 1

# Spawn Wave 2 (depends on Wave 1)
bash "$SKILL_DIR/orchestration/wave-spawn.sh" $SESSION_ID 2

# Monitor Wave 2
bash "$SKILL_DIR/orchestration/wave-monitor.sh" $SESSION_ID 2

# Continue for all waves...
```

### 3. Monitor Session

```bash
# List all sessions
bash "$SKILL_DIR/orchestration/session-status.sh" --list

# Session overview
bash "$SKILL_DIR/orchestration/session-status.sh" $SESSION_ID

# Detailed status
bash "$SKILL_DIR/orchestration/session-status.sh" $SESSION_ID --detailed

# JSON output
bash "$SKILL_DIR/orchestration/session-status.sh" $SESSION_ID --json
```

### 4. Merge Results

```bash
# Preview merges
bash "$SKILL_DIR/orchestration/merge-waves.sh" $SESSION_ID --dry-run

# Merge all completed agents
bash "$SKILL_DIR/orchestration/merge-waves.sh" $SESSION_ID

# Merge specific wave
bash "$SKILL_DIR/orchestration/merge-waves.sh" $SESSION_ID --wave 1
```

## DAG File Format

The orchestration expects a DAG file with this structure:

```json
{
  "task_description": "Implement authentication system",
  "nodes": {
    "ws-1-auth-service": {
      "task": "Create authentication service with JWT support",
      "agent_type": "backend-developer",
      "workstream_id": "ws-1",
      "dependencies": [],
      "deliverables": ["src/services/AuthService.ts", "tests"]
    },
    "ws-2-oauth": {
      "task": "Integrate OAuth providers (Google, GitHub)",
      "agent_type": "backend-developer",
      "workstream_id": "ws-2",
      "dependencies": ["ws-1"],
      "deliverables": ["src/services/OAuthService.ts"]
    }
  },
  "waves": [
    {"wave_number": 1, "nodes": ["ws-1-auth-service"]},
    {"wave_number": 2, "nodes": ["ws-2-oauth"]}
  ]
}
```

## Orchestration Example

```bash
# After running /m-plan for a complex feature...

SKILL_DIR="${TOOL_DIR}/skills/agent-orchestrator/scripts"
DAG_FILE="~/.claude/orchestration/state/dag-auth-feature.json"

# Create session
SESSION=$(bash "$SKILL_DIR/orchestration/session-create.sh" "$DAG_FILE" | grep SESSION_ID | cut -d= -f2)

# Execute all waves
TOTAL_WAVES=$(jq '.waves | length' "$DAG_FILE")
for wave in $(seq 1 $TOTAL_WAVES); do
    echo "Starting Wave $wave..."
    bash "$SKILL_DIR/orchestration/wave-spawn.sh" "$SESSION" "$wave"
    bash "$SKILL_DIR/orchestration/wave-monitor.sh" "$SESSION" "$wave"
done

# Merge all results
bash "$SKILL_DIR/orchestration/merge-waves.sh" "$SESSION"

# View final status
bash "$SKILL_DIR/orchestration/session-status.sh" "$SESSION" --detailed
```

---

# Agent Status Detection

Both modes use the same status detection:

| Status | Indicators | Recommended Action |
|--------|-----------|-------------------|
| `active` | Agent is processing | Wait for completion |
| `idle` | At Claude prompt | Send instructions or complete |
| `complete` | Task finished, commits created | Review and cleanup |
| `failed` | Error messages detected | Attach and debug |
| `killed` | Session no longer exists | Already cleaned up |

---

# Metadata Storage

## Simple Mode
- `~/.claude/agents/{session}.json` - Agent metadata

## Orchestration Mode
- `~/.claude/orchestration/state/dag-{session-id}.json` - DAG definition
- `~/.claude/orchestration/state/session-{session-id}.json` - Session state

## Unified Agent Metadata Format

```json
{
  "session": "agent-1705161234",
  "task": "implement user authentication",
  "directory": "/path/to/project",
  "created": "2025-01-13T14:30:00Z",
  "status": "running",
  "with_worktree": true,
  "worktree_branch": "agent/agent-1705161234",
  "orchestration": {
    "session_id": "orch-1705161000",
    "wave": 2,
    "workstream_id": "ws-auth",
    "dag_node": "ws-2-auth-service",
    "agent_type": "backend-developer",
    "dependencies": ["ws-1-database"]
  }
}
```

---

# Best Practices

## Task Descriptions

Write clear, specific task descriptions:

```bash
# Good - clear and specific
"Implement user authentication with JWT tokens and refresh token rotation"

# Bad - vague
"Add auth"
```

## When to Use Worktrees

**Use worktree (`--with-worktree`)** when:
- Agent will make many commits
- Multiple agents might edit same files
- Want to easily revert agent's work

**Use shared directory (default)** when:
- Quick, small tasks
- Task is read-heavy (research, review)

## Parallel Limits

Recommended concurrent agents:
- **Standard machine**: 2-4 agents
- **High-memory machine**: 4-8 agents

Consider I/O and API rate limits.

---

# Troubleshooting

## Agent Not Starting

```bash
# Check spawn debug log
cat /tmp/spawn-agent-{session}-failure.log

# Verify Claude Code installed
which claude

# Check tmux running
tmux list-sessions
```

## Agent Stuck

```bash
# Attach and check output
tmux attach -t agent-{timestamp}

# Send wake-up prompt
tmux send-keys -t agent-{timestamp} "continue with the task" C-m
```

## Merge Conflicts

```bash
# Check worktree status
git -C worktrees/agent-xxx status

# Resolve manually then continue
git merge --continue
```

---

# Related Commands

- `/spawn-agent` - Original spawn command
- `/m-plan` - Multi-agent planning (creates DAG)
- `/m-implement` - Multi-agent implementation
- `/m-monitor` - Multi-agent monitoring
- `/tmux-status` - General tmux status

---

# Dependencies

Required:
- `tmux` (session management)
- `jq` (JSON parsing)
- `git` (for worktree features)

Optional:
- `tmuxwatch` (enhanced monitoring)
