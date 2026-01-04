#!/bin/bash
# ABOUTME: Manages tmux sessions for Agents-in-a-Box
# Attaches to existing Claude session or creates a new one with proper nesting handling

SESSION_NAME="claude-session"

# Function to attach to session based on context
attach_to_session() {
    if [ -n "$TMUX" ]; then
        # We're already inside tmux, use switch-client instead of attach
        echo "Switching to Claude session..."
        echo "Press Ctrl-b then d to return to previous session"
        tmux switch-client -t "$SESSION_NAME"
    else
        # Not in tmux, regular attach
        echo "Attaching to existing Claude session..."
        echo "Press Ctrl-b then d to detach without stopping Claude"
        exec tmux attach-session -t "$SESSION_NAME"
    fi
}

# Check if tmux session already exists
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    attach_to_session
else
    echo "No existing Claude session found. Starting new session..."
    echo "Press Ctrl-b then d to detach without stopping Claude"

    # Create log directory if it doesn't exist
    mkdir -p /workspace/.agents-box/logs

    LOG_FILE="/workspace/.agents-box/logs/claude-$(date +%Y%m%d-%H%M%S).log"

    # Create new tmux session
    if [ -n "$TMUX" ]; then
        # We're already in tmux, create detached session then switch
        tmux new-session -d -s "$SESSION_NAME" \
            "echo 'Claude CLI starting...' | tee '$LOG_FILE'; /app/scripts/start-claude-interactive.sh 2>&1 | tee -a '$LOG_FILE'; echo 'Claude CLI exited. Press Enter to restart or Ctrl+C to exit.'; read; exec bash"
        tmux switch-client -t "$SESSION_NAME"
    else
        # Not in tmux, create and attach normally
        exec tmux new-session -s "$SESSION_NAME" \
            "echo 'Claude CLI starting...' | tee '$LOG_FILE'; /app/scripts/start-claude-interactive.sh 2>&1 | tee -a '$LOG_FILE'; echo 'Claude CLI exited. Press Enter to restart or Ctrl+C to exit.'; read; exec bash"
    fi
fi
