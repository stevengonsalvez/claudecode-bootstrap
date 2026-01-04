#!/bin/bash
# Custom bashrc for Agents-in-a-Box sessions

# Source the default bashrc if it exists
if [ -f /etc/bash.bashrc ]; then
    . /etc/bash.bashrc
fi

# Function to check tmux session status
check_claude_session() {
    if tmux has-session -t "claude-session" 2>/dev/null; then
        echo "Active"
    else
        echo "None"
    fi
}

# Only show welcome message in interactive shells
case "$-" in
    *i*)
        # Interactive shell - show welcome message
        clear
        ;;
    *)
        # Non-interactive shell - skip welcome message
        return
        ;;
esac
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║              Welcome to Agents-in-a-Box Session                  ║"
echo "╠══════════════════════════════════════════════════════════════════╣"
echo "║                                                                  ║"
echo "║  🚀 Claude CLI is ready to use!                                 ║"
echo "║                                                                  ║"
echo "║  Quick Commands:                                                 ║"
echo "║  • claude-ask \"question\" - Ask Claude (logged to TUI)           ║"
echo "║  • claude-start        - Start interactive Claude session       ║"
echo "║  • claude-help         - Show all Claude commands               ║"
echo "║  • claude-logs         - View live Claude output                ║"
echo "║  • claude-status       - Check Claude session status            ║"
echo "║  • exit                - Exit shell (Claude keeps running)      ║"
echo "║                                                                  ║"
echo "║  Tmux Controls (when attached to Claude):                       ║"
echo "║  • Ctrl-b then d       - Detach (Claude keeps running)          ║"
echo "║  • Ctrl-b then [       - Scroll mode (q to exit scroll)         ║"
echo "║                                                                  ║"
echo "║  Session Status:                                                 ║"
echo "║  • Claude Session: $(check_claude_session)                       ║"
echo "║  • Working Directory: $(pwd)                                     ║"
echo "║                                                                  ║"
echo "║  💡 Tip: Just type 'claude' to start chatting!                  ║"
echo "║                                                                  ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo

# Set a custom prompt to indicate we're in an Agents-in-a-Box session
export PS1="\[\033[01;32m\][agents-box]\[\033[00m\] \[\033[01;34m\]\w\[\033[00m\] $ "

# Add helpful aliases
alias cls='clear'
alias ll='ls -la'
alias status='docker ps'

# Source Claude logging commands if available
if [ -f /app/scripts/claude-commands.sh ]; then
    source /app/scripts/claude-commands.sh
fi

# Claude session management functions
claude-start() {
    /app/scripts/claude-session-manager.sh attach
}

claude-logs() {
    /app/scripts/claude-session-manager.sh logs
}

claude-restart() {
    /app/scripts/claude-session-manager.sh restart
}

claude-status() {
    /app/scripts/claude-session-manager.sh status
}

claude-stop() {
    /app/scripts/claude-session-manager.sh stop
}

# Create a wrapper function for claude that respects CLAUDE_CONTINUE_FLAG
claude() {
    if [ "$1" = "--help" ] || [ "$1" = "--version" ] || [ "$1" = "config" ] || [ "$1" = "auth" ] || [ "$1" = "mcp" ]; then
        # For configuration commands, call claude directly without flags
        /home/claude-user/.npm-global/bin/claude "$@"
    else
        # For interactive/chat commands, use the continue flag
        if [ -n "$CLAUDE_CONTINUE_FLAG" ]; then
            eval "/home/claude-user/.npm-global/bin/claude $CLAUDE_CONTINUE_FLAG \"\$@\""
        else
            /home/claude-user/.npm-global/bin/claude "$@"
        fi
    fi
}

# Alias for interactive session management (restored functionality)
alias claude-interactive='claude-start'

# Export functions so they're available in the shell
export -f claude
export -f claude-start
export -f claude-logs
export -f claude-restart
export -f claude-status
export -f claude-stop
export -f check_claude_session
