# /spawn-agent - Spawn Claude Agent in tmux Session

Spawn a Claude Code agent in a separate tmux session with optional handover context.

## Usage

```bash
/spawn-agent "implement user authentication"
/spawn-agent "refactor the API layer" --with-handover
```

## Implementation

```bash
#!/bin/bash

# Parse arguments
TASK="$1"
WITH_HANDOVER=false

if [[ "$2" == "--with-handover" ]]; then
    WITH_HANDOVER=true
fi

if [ -z "$TASK" ]; then
    echo "❌ Task description required"
    echo "Usage: /spawn-agent \"task description\" [--with-handover]"
    exit 1
fi

# Generate session info
TASK_ID=$(date +%s)
SESSION="agent-${TASK_ID}"
WORK_DIR=$(pwd)

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
  "with_handover": $WITH_HANDOVER
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
