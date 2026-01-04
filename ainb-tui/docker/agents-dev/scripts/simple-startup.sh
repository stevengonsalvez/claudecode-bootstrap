#!/bin/bash
# ABOUTME: Simplified startup script that prepares the environment without auto-starting Claude
# This avoids complex tmux nesting issues while providing excellent UX

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[agents-box]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[agents-box]${NC} $1"
}

error() {
    echo -e "${RED}[agents-box]${NC} $1"
}

success() {
    echo -e "${GREEN}[agents-box]${NC} $1"
}

# Load environment variables from .env if it exists
if [ -f /app/.env ]; then
    log "Loading environment variables from .env"
    set -a
    source /app/.env
    set +a
fi

# Check for existing authentication (multiple sources)
AUTH_OK=false
AUTH_SOURCES=()

# Check for mounted .claude.json first (OAuth tokens from Claude Max)
if [ -f /home/claude-user/.claude.json ] && [ -s /home/claude-user/.claude.json ]; then
    AUTH_SOURCES+=(".claude.json (OAuth tokens)")
    AUTH_OK=true
    log "Using mounted .claude.json with OAuth authentication"
elif [ -n "${ANTHROPIC_API_KEY}" ]; then
    AUTH_SOURCES+=("ANTHROPIC_API_KEY environment variable")
    AUTH_OK=true
    log "Using ANTHROPIC_API_KEY environment variable for authentication"
fi

# Check for .claude directory with credentials (if no auth found yet)
if [ "${AUTH_OK}" = "false" ] && [ -f /home/claude-user/.claude/.credentials.json ] && [ -s /home/claude-user/.claude/.credentials.json ]; then
    AUTH_SOURCES+=(".claude/.credentials.json (agents-in-a-box)")
    AUTH_OK=true
fi

if [ "${AUTH_OK}" = "true" ]; then
    success "Found Claude authentication via: ${AUTH_SOURCES[*]}"
else
    warn "No Claude authentication found!"
    warn "Please ensure one of:"
    warn "  1. Set ANTHROPIC_API_KEY environment variable"
    warn "  2. Mount ~/.claude.json to /home/claude-user/.claude.json"
    warn "  3. Have a valid .credentials.json file"
fi

# Create .claude directory if it doesn't exist
if [ ! -d /home/claude-user/.claude ]; then
    mkdir -p /home/claude-user/.claude
fi

# Configure GitHub CLI if GITHUB_TOKEN is provided
if [ -n "${GITHUB_TOKEN}" ]; then
    log "Configuring GitHub CLI with token authentication"
    gh auth login --with-token <<< "${GITHUB_TOKEN}"

    # Configure git to use the token for authentication
    git config --global credential.helper store
    echo "https://oauth:${GITHUB_TOKEN}@github.com" > /home/claude-user/.git-credentials

    # Test gh CLI connection
    if gh auth status > /dev/null 2>&1; then
        success "GitHub CLI authenticated successfully"
    else
        warn "GitHub CLI authentication failed"
    fi
fi

# Copy CLAUDE.md template if it doesn't exist in workspace
if [ ! -f /workspace/CLAUDE.md ] && [ -f /app/config/CLAUDE.md.template ]; then
    log "Creating CLAUDE.md from template"
    cp /app/config/CLAUDE.md.template /workspace/CLAUDE.md
fi

# Set trust dialog to accepted
claude config set hasTrustDialogAccepted true >/dev/null 2>&1 || true

# Create log directory
mkdir -p /workspace/.agents-box/logs

# Prepare the claude session (but don't start it)
log "Environment prepared. Claude CLI is ready to use!"
if [ "${AUTH_OK}" = "true" ]; then
    success "✅ Authentication detected - Claude will work immediately"
    success "📝 Run 'claude-start' to begin chatting with Claude"
else
    warn "⚠️  No authentication detected - Claude won't work until you provide credentials"
    warn "📝 Set ANTHROPIC_API_KEY or mount authentication files"
fi

# If no command specified, start an interactive shell
if [ $# -eq 0 ]; then
    success "Starting interactive shell..."
    success "Type 'claude-start' to begin using Claude!"
    exec bash
else
    # Run the specified command
    log "Running command: $*"
    exec "$@"
fi
