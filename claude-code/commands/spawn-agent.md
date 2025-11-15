# /spawn-agent - Spawn Async Agent in Isolated Workspace

Spin off a long-running AI agent in a separate tmux session with git worktree isolation and optional handover context.

## Usage

```bash
/spawn-agent codex "refactor auth module"
/spawn-agent codex "refactor auth" --with-handover
/spawn-agent aider "generate docs" --no-worktree
/spawn-agent claude "implement feature X" --with-handover --use-worktree
```

## Process

### Step 1: Gather Requirements

```bash
AGENT_TYPE=${1:-codex}  # codex, aider, claude
TASK=${2:-""}
WITH_HANDOVER=false
USE_WORKTREE=true

# Parse flags
shift 2
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-handover) WITH_HANDOVER=true ;;
        --no-handover) WITH_HANDOVER=false ;;
        --use-worktree) USE_WORKTREE=true ;;
        --no-worktree) USE_WORKTREE=false ;;
    esac
    shift
done

[ -z "$TASK" ] && echo "❌ Task description required" && exit 1
```

### Step 2: Generate Handover Document (if requested)

```bash
TASK_ID=$(date +%s)
HANDOVER_FILE=""

if [ "$WITH_HANDOVER" = true ]; then
    HANDOVER_FILE="${TOOL_DIR}/session/handover-${TASK_ID}.md"

    # Use /handover command to generate
    # This creates context document with:
    # - Session health metrics
    # - Current work context
    # - Technical details
    # - Resumption instructions

    # Generate handover (invoke /handover command internally)
    # Content saved to $HANDOVER_FILE
fi
```

### Step 3: Create Git Worktree (if requested)

```bash
WORKTREE_DIR=""
BRANCH_NAME=""
BASE_BRANCH=$(git branch --show-current)

if [ "$USE_WORKTREE" = true ]; then
    WORKTREE_DIR="worktrees/agent-${TASK_ID}"
    BRANCH_NAME="agent/agent-${TASK_ID}"

    mkdir -p worktrees
    git worktree add -b "$BRANCH_NAME" "$WORKTREE_DIR" "$BASE_BRANCH"

    WORK_DIR="$WORKTREE_DIR"
else
    WORK_DIR=$(pwd)
fi
```

### Step 4: Create tmux Session

```bash
SESSION="agent-${TASK_ID}"

tmux new-session -d -s "$SESSION" -n work -c "$WORK_DIR"
tmux split-window -h -t "$SESSION:work" -c "$WORK_DIR"

# Pane 0: Agent workspace
# Pane 1: Monitoring
```

### Step 5: Copy Handover to Workspace (if exists)

```bash
if [ -n "$HANDOVER_FILE" ] && [ -f "$HANDOVER_FILE" ]; then
    cp "$HANDOVER_FILE" "$WORK_DIR/.agent-handover.md"
fi
```

### Step 6: Start Agent

```bash
case $AGENT_TYPE in
    codex)
        if [ -n "$HANDOVER_FILE" ]; then
            tmux send-keys -t "$SESSION:work.0" "cat .agent-handover.md" C-m
            tmux send-keys -t "$SESSION:work.0" "codex --task 'Review handover and: $TASK'" C-m
        else
            tmux send-keys -t "$SESSION:work.0" "codex --task '$TASK'" C-m
        fi
        ;;
    aider)
        if [ -n "$HANDOVER_FILE" ]; then
            tmux send-keys -t "$SESSION:work.0" "aider --read .agent-handover.md --message '$TASK'" C-m
        else
            tmux send-keys -t "$SESSION:work.0" "aider --message '$TASK'" C-m
        fi
        ;;
    claude)
        tmux send-keys -t "$SESSION:work.0" "claude code" C-m
        sleep 3
        if [ -n "$HANDOVER_FILE" ]; then
            tmux send-keys -t "$SESSION:work.0" "Read .agent-handover.md for context, then: $TASK" C-m
        else
            tmux send-keys -t "$SESSION:work.0" "$TASK" C-m
        fi
        ;;
esac
```

### Step 7: Setup Monitoring Pane

```bash
tmux send-keys -t "$SESSION:work.1" "watch -n 5 'echo \"=== Agent Output ===\" && tmux capture-pane -t $SESSION:work.0 -p | tail -20 && echo && echo \"=== Git Status ===\" && git status -sb && echo && echo \"=== Recent Commits ===\" && git log --oneline -3'" C-m
```

### Step 8: Save Session Metadata

```bash
mkdir -p ~/.claude/agents

cat > ~/.claude/agents/${SESSION}.json <<EOF
{
  "session": "$SESSION",
  "agent_type": "$AGENT_TYPE",
  "task": "$TASK",
  "directory": "$WORK_DIR",
  "base_branch": "$BASE_BRANCH",
  "agent_branch": "$BRANCH_NAME",
  "worktree": $USE_WORKTREE,
  "handover_document": "$( [ -n "$HANDOVER_FILE" ] && echo ".agent-handover.md" || echo "null" )",
  "handover_source": "$( [ -n "$HANDOVER_FILE" ] && echo "$HANDOVER_FILE" || echo "null" )",
  "created": "$(date -Iseconds)",
  "status": "running",
  "parent_session": "$(tmux display-message -p '#S' 2>/dev/null || echo 'unknown')"
}
EOF
```

### Step 9: Display Summary

```bash
echo ""
echo "✨ Agent Spawned: $SESSION"
echo ""
echo "Agent: $AGENT_TYPE"
echo "Task: $TASK"
[ "$USE_WORKTREE" = true ] && echo "Worktree: $WORKTREE_DIR (branch: $BRANCH_NAME)"
[ -n "$HANDOVER_FILE" ] && echo "Handover: Passed via .agent-handover.md"
echo ""
echo "Monitor: tmux attach -t $SESSION"
echo "Status: /tmux-status"
echo ""
echo "💡 Agent works independently in $( [ "$USE_WORKTREE" = true ] && echo "isolated worktree" || echo "current directory" )"
echo ""

if [ "$USE_WORKTREE" = true ]; then
    echo "When complete:"
    echo "  Review: git diff $BASE_BRANCH..agent/agent-${TASK_ID}"
    echo "  Merge: git merge agent/agent-${TASK_ID}"
    echo "  Cleanup: git worktree remove $WORKTREE_DIR"
    echo ""
fi
```

## When to Use

**Use /spawn-agent**:
- ✅ Long-running refactoring (30+ minutes)
- ✅ Batch code generation
- ✅ Overnight processing
- ✅ Parallel experimentation

**Use Task tool instead**:
- ❌ Quick code reviews
- ❌ Single-file changes
- ❌ Work needing immediate conversation

## Handover Documents

**With handover** (`--with-handover`):
- Agent receives full session context
- Understands current work, goals, constraints
- Best for complex tasks building on current work

**Without handover** (default):
- Agent starts fresh
- Best for isolated, self-contained tasks

## Git Worktrees

**With worktree** (default):
- Agent works on separate branch: `agent/agent-{timestamp}`
- Main session continues on current branch
- Easy to review via: `git diff main..agent/agent-{timestamp}`
- Can merge or discard cleanly

**Without worktree** (`--no-worktree`):
- Agent works in current directory
- Commits to current branch
- Simpler for sequential work

## Cleanup

```bash
# Kill agent session
tmux kill-session -t agent-{timestamp}

# Remove worktree (if used)
git worktree remove worktrees/agent-{timestamp}

# Delete branch (if merged)
git branch -d agent/agent-{timestamp}

# Remove metadata
rm ~/.claude/agents/agent-{timestamp}.json
```

## Notes

- Spawned agents run independently (no conversation loop)
- With handover: agent receives context but still autonomous
- With worktree: true parallel development (no conflicts)
- Metadata tracked in `~/.claude/agents/*.json`
- Visible in `/tmux-status` output
