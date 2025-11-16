# /spawn-agent - Spawn Claude Agent in tmux Session

Spawn a Claude Code agent in a separate tmux session with optional handover context.

## Usage

```bash
/spawn-agent "implement user authentication"
/spawn-agent "refactor the API layer" --with-handover
/spawn-agent "implement feature X" --with-worktree
/spawn-agent "review the PR" --with-worktree --with-handover
```

## Implementation

```bash
#!/bin/bash

# Parse arguments
TASK="$1"
WITH_HANDOVER=false
WITH_WORKTREE=false
shift

# Parse flags
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-handover)
            WITH_HANDOVER=true
            shift
            ;;
        --with-worktree)
            WITH_WORKTREE=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

if [ -z "$TASK" ]; then
    echo "❌ Task description required"
    echo "Usage: /spawn-agent \"task description\" [--with-handover] [--with-worktree]"
    exit 1
fi

# Generate session info
TASK_ID=$(date +%s)
SESSION="agent-${TASK_ID}"

# Setup working directory (worktree or current)
if [ "$WITH_WORKTREE" = true ]; then
    # Detect transcrypt (informational only - works transparently with worktrees)
    if git config --get-regexp '^transcrypt\.' >/dev/null 2>&1; then
        echo "📦 Transcrypt detected - worktree will inherit encryption config automatically"
        echo ""
    fi

    # Get current branch as base
    CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "HEAD")

    # Generate task slug from task description
    TASK_SLUG=$(echo "$TASK" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 -]//g' | tr -s ' ' '-' | cut -c1-40 | sed 's/-$//')

    # Source worktree utilities
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    source "$SCRIPT_DIR/../utils/git-worktree-utils.sh"

    # Create worktree with task slug
    echo "🌳 Creating isolated git worktree..."
    WORK_DIR=$(create_agent_worktree "$TASK_ID" "$CURRENT_BRANCH" "$TASK_SLUG")
    AGENT_BRANCH="agent/agent-${TASK_ID}"

    echo "✅ Worktree created:"
    echo "   Directory: $WORK_DIR"
    echo "   Branch: $AGENT_BRANCH"
    echo "   Base: $CURRENT_BRANCH"
    echo ""
else
    WORK_DIR=$(pwd)
    AGENT_BRANCH=""
fi

echo "🚀 Spawning Claude agent in tmux session..."
echo ""

# Generate handover if requested
HANDOVER_CONTENT=""
if [ "$WITH_HANDOVER" = true ]; then
    echo "📝 Generating handover context..."

    # Get current branch and recent commits
    CURRENT_BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
    RECENT_COMMITS=$(git log --oneline -5 2>/dev/null || echo "No git history")
    GIT_STATUS=$(git status -sb 2>/dev/null || echo "Not a git repo")

    # Create handover content
    HANDOVER_CONTENT=$(cat << EOF

# Handover Context

## Current State
- Branch: $CURRENT_BRANCH
- Directory: $WORK_DIR
- Time: $(date)

## Recent Commits
$RECENT_COMMITS

## Git Status
$GIT_STATUS

## Your Task
$TASK

---
Please review the above context and proceed with the task.
EOF
)

    echo "✅ Handover generated"
    echo ""
fi

# Create tmux session
tmux new-session -d -s "$SESSION" -c "$WORK_DIR"

echo "✅ Created tmux session: $SESSION"
echo ""

# Start Claude Code in the session
tmux send-keys -t "$SESSION" "claude --dangerously-skip-permissions" C-m

# Wait for Claude to start
sleep 2

# Send handover context if generated
if [ "$WITH_HANDOVER" = true ]; then
    echo "📤 Sending handover context to agent..."
    # Send the handover content
    tmux send-keys -t "$SESSION" "$HANDOVER_CONTENT" C-m
    sleep 1
fi

# Send the task
echo "📤 Sending task to agent..."
tmux send-keys -t "$SESSION" "$TASK" C-m

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✨ Agent spawned successfully!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Session: $SESSION"
echo "Task: $TASK"
echo "Directory: $WORK_DIR"
echo ""
echo "To monitor:"
echo "  tmux attach -t $SESSION"
echo ""
echo "To send more commands:"
echo "  tmux send-keys -t $SESSION \"your command\" C-m"
echo ""
echo "To kill session:"
echo "  tmux kill-session -t $SESSION"
echo ""

# Save metadata
mkdir -p ~/.claude/agents
cat > ~/.claude/agents/${SESSION}.json <<EOF
{
  "session": "$SESSION",
  "task": "$TASK",
  "directory": "$WORK_DIR",
  "created": "$(date -Iseconds)",
  "status": "running",
  "with_handover": $WITH_HANDOVER,
  "with_worktree": $WITH_WORKTREE,
  "worktree_branch": "$AGENT_BRANCH"
}
EOF

exit 0
```

## Notes

- Default agent is Claude Code with `--dangerously-skip-permissions`
- Agent runs in tmux session named `agent-{timestamp}`
- Use `tmux attach -t agent-{timestamp}` to monitor
- Use `tmux send-keys` to send additional prompts
- Metadata saved to `~/.claude/agents/agent-{timestamp}.json`

## Worktree Isolation

- Use `--with-worktree` flag for isolated git workspace
- Creates worktree in `worktrees/agent-{timestamp}-{task-slug}`
  - Example: `worktrees/agent-1763250000-implement-user-auth`
- Creates branch `agent/agent-{timestamp}` based on current branch
- Transcrypt encryption inherited automatically (no special setup needed)
- Use `/list-agent-worktrees` to see all worktrees
- Use `/cleanup-agent-worktree {timestamp}` to remove when done
