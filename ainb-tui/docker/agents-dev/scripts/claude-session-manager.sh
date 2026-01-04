#!/bin/bash
# ABOUTME: Robust Claude session manager that handles all edge cases
# Provides start, stop, restart, status, and attach functionality

SESSION_NAME="claude-session"
LOG_DIR="/workspace/.agents-box/logs"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Helper functions
log() { echo -e "${BLUE}[claude]${NC} $1"; }
success() { echo -e "${GREEN}[claude]${NC} $1"; }
warn() { echo -e "${YELLOW}[claude]${NC} $1"; }
error() { echo -e "${RED}[claude]${NC} $1"; }

# Check if we have valid authentication
check_auth() {
    if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
        return 0
    elif [ -n "${ANTHROPIC_API_KEY}" ]; then
        return 0
    elif [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
        return 0
    else
        return 1
    fi
}

# Create log directory
ensure_log_dir() {
    mkdir -p "$LOG_DIR"
}

# Get latest log file
get_log_file() {
    echo "$LOG_DIR/claude-$(date +%Y%m%d-%H%M%S).log"
}

# Check if claude session exists
session_exists() {
    tmux has-session -t "$SESSION_NAME" 2>/dev/null
}

# Start Claude session
start_session() {
    if session_exists; then
        warn "Claude session already exists. Use 'attach' to connect or 'restart' to restart."
        return 1
    fi

    if ! check_auth; then
        error "No authentication found. Please set up authentication first:"
        error "  • Set ANTHROPIC_API_KEY environment variable"
        error "  • Or mount ~/.claude.json for OAuth authentication"
        return 1
    fi

    ensure_log_dir
    local log_file=$(get_log_file)

    log "Starting new Claude session..."
    log "Logs will be written to: $log_file"

    # Create the session
    tmux new-session -d -s "$SESSION_NAME" \
        "echo 'Claude session starting...' | tee '$log_file'; \
         echo 'Authentication detected, initializing Claude CLI...' | tee -a '$log_file'; \
         /app/scripts/start-claude-interactive.sh 2>&1 | tee -a '$log_file'; \
         echo 'Claude CLI exited. Press Enter to restart or Ctrl+C to exit.' | tee -a '$log_file'; \
         read; exec bash"

    if session_exists; then
        success "Claude session started successfully!"
        success "Use 'claude-start attach' to connect to it"
        return 0
    else
        error "Failed to start Claude session"
        return 1
    fi
}

# Attach to Claude session
attach_session() {
    if ! session_exists; then
        warn "No Claude session found. Starting a new one..."
        start_session || return 1
    fi

    log "Attaching to Claude session..."
    log "Press Ctrl-b then d to detach (Claude keeps running)"

    if [ -n "$TMUX" ]; then
        # Inside tmux, use switch-client
        tmux switch-client -t "$SESSION_NAME"
    else
        # Outside tmux, use attach
        exec tmux attach-session -t "$SESSION_NAME"
    fi
}

# Stop Claude session
stop_session() {
    if ! session_exists; then
        warn "No Claude session to stop"
        return 0
    fi

    log "Stopping Claude session..."
    tmux kill-session -t "$SESSION_NAME"
    success "Claude session stopped"
}

# Restart Claude session
restart_session() {
    log "Restarting Claude session..."
    stop_session
    sleep 1
    start_session
}

# Show session status
show_status() {
    if session_exists; then
        success "✅ Claude session is running"
        echo ""
        echo "Session details:"
        tmux list-sessions | grep "$SESSION_NAME"
        echo ""
        echo "Recent activity:"
        tmux capture-pane -t "$SESSION_NAME" -p | tail -10
        echo ""
        log "Use 'claude-start attach' to connect to the session"
    else
        warn "❌ Claude session is not running"
        log "Use 'claude-start' to start a new session"
    fi
}

# Show logs
show_logs() {
    local latest_log=$(ls -t "$LOG_DIR"/claude-*.log 2>/dev/null | head -n1)
    if [ -n "$latest_log" ]; then
        log "Viewing: $latest_log"
        log "Press Ctrl-C to stop following logs"
        tail -f "$latest_log"
    else
        warn "No Claude logs found yet"
    fi
}

# Main command router
case "${1:-attach}" in
    start)
        start_session
        ;;
    attach|"")
        attach_session
        ;;
    stop|kill)
        stop_session
        ;;
    restart)
        restart_session
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs
        ;;
    *)
        echo "Usage: $0 {start|attach|stop|restart|status|logs}"
        echo ""
        echo "Commands:"
        echo "  start    - Start a new Claude session"
        echo "  attach   - Attach to existing session (default)"
        echo "  stop     - Stop the Claude session"
        echo "  restart  - Restart the Claude session"
        echo "  status   - Show session status"
        echo "  logs     - Follow Claude logs"
        exit 1
        ;;
esac
