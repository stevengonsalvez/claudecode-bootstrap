#!/bin/bash
# ABOUTME: Startup script for agents-dev container
# Handles environment setup, authentication, and CLI initialization

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

# Check agents-box session mode
if [ "${AGENTS_BOX_MODE}" = "boss" ]; then
    log "Running in agents-box boss mode"
elif [ "${AGENTS_BOX_MODE}" = "interactive" ]; then
    log "Running in agents-box interactive mode"
elif [ "${AGENTS_BOX_MODE}" = "true" ]; then
    # Legacy support
    log "Running in agents-box mode (legacy)"
fi

# Check for existing authentication (multiple sources)
AUTH_OK=false
AUTH_SOURCES=()

# Handle authentication for parallel sessions
# Priority: 1. Mounted .claude.json (OAuth tokens), 2. Environment variable, 3. Credentials file

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
    log "Found Claude authentication via: ${AUTH_SOURCES[*]}"
else
    warn "No Claude authentication found!"
    warn "Please ensure one of:"
    warn "  1. Run 'agents-box auth' to set up authentication"
    warn "  2. Have ~/.agents-in-a-box/auth/.credentials.json (mounted to /home/claude-user/.claude/.credentials.json)"
    warn "  3. Set ANTHROPIC_API_KEY in environment"
fi

# Create .claude directory if it doesn't exist (unless it's already mounted)
if [ ! -d /home/claude-user/.claude ]; then
    mkdir -p /home/claude-user/.claude
fi

# Configure GitHub CLI if GITHUB_TOKEN is provided
if [ -n "${GITHUB_TOKEN}" ]; then
    log "Configuring GitHub CLI with token authentication"
    echo "${GITHUB_TOKEN}" | gh auth login --with-token

    # Configure git to use the token for authentication
    git config --global credential.helper store
    echo "https://oauth:${GITHUB_TOKEN}@github.com" > /home/claude-user/.git-credentials

    # Test gh CLI connection
    if gh auth status > /dev/null 2>&1; then
        success "GitHub CLI authenticated successfully"
        log "Available commands: gh issue list, gh pr list, gh repo view, etc."
    else
        warn "GitHub CLI authentication failed"
    fi
else
    warn "GITHUB_TOKEN not found - GitHub CLI and token-based git auth unavailable"
    log "SSH keys will be used for git operations if available"
fi

# Copy CLAUDE.md template if it doesn't exist in workspace
if [ ! -f /workspace/CLAUDE.md ] && [ -f /app/config/CLAUDE.md.template ]; then
    log "Creating CLAUDE.md from template"
    cp /app/config/CLAUDE.md.template /workspace/CLAUDE.md
fi

# Set up Claude CLI logging commands
log "Setting up Claude CLI logging commands"
if [ -f /app/scripts/claude-commands.sh ]; then
    source /app/scripts/claude-commands.sh
    log "✅ Claude logging commands available: claude-ask, claude-print, claude-script"
else
    warn "Claude logging commands not found"
fi

# Set trust dialog to accepted to avoid prompts when using --dangerously-skip-permissions
if [[ "$CLAUDE_CONTINUE_FLAG" == *"--dangerously-skip-permissions"* ]]; then
    log "Setting trust dialog acceptance to avoid permission prompts (skip permissions enabled)"
    # Use direct binary to avoid triggering our wrapper's --dangerously-skip-permissions flag
    /home/claude-user/.npm-global/bin/claude config set hasTrustDialogAccepted true >/dev/null 2>&1 || warn "Failed to set trust dialog config"
else
    log "Trust dialog will be shown as needed (permissions enabled)"
fi

# Determine which CLI to use (adapted from claude-docker startup.sh)
CLI_CMD="claude"
CLI_ARGS="$CLAUDE_CONTINUE_FLAG"

log "Using Claude CLI with args: $CLI_ARGS"

# Handle boss mode execution
if [ "${AGENTS_BOX_MODE}" = "boss" ] && [ -n "${AGENTS_BOX_PROMPT}" ]; then
    # Create log directory
    mkdir -p /workspace/.agents-box/logs

    success "Container environment ready!"
    if [ "${AUTH_OK}" = "true" ]; then
        success "✅ Authentication detected - Claude will work immediately"
        log "🤖 Executing boss mode prompt..."
        log "Prompt: ${AGENTS_BOX_PROMPT}"

        # Boss mode prompt text to append
        BOSS_MODE_PROMPT="Ultrathink and understand our project rules, particularly around testing. You must go test first, and you must work in a way that allows for small known-good increments. You must commit when the code is in a working state, and commit early and often. When committing: - Use conventional commit format (feat:, fix:, refactor:, test:, docs:) - Commit after each logical increment (test passes, feature complete, refactor done) - Generate descriptive commit messages that explain the 'what' and 'why' - Never leave code in a broken state between commits"

        # Append boss mode prompt to user prompt
        ENHANCED_PROMPT="${AGENTS_BOX_PROMPT} ${BOSS_MODE_PROMPT}"

        # Execute Claude with the enhanced prompt and stream-json output
        log "Running: claude --print --output-format stream-json --verbose \"${ENHANCED_PROMPT}\""
        exec claude --print --output-format stream-json --verbose "${ENHANCED_PROMPT}" $CLI_ARGS
    else
        error "❌ Boss mode requires authentication!"
        error "Please ensure one of:"
        error "  1. Run 'agents-box auth' to set up authentication"
        error "  2. Have ~/.agents-in-a-box/auth/.credentials.json mounted"
        error "  3. Set ANTHROPIC_API_KEY in environment"
        exit 1
    fi
elif [ "${AGENTS_BOX_MODE}" = "boss" ]; then
    error "❌ Boss mode requires a prompt!"
    error "AGENTS_BOX_PROMPT environment variable is missing or empty"
    exit 1
fi

# If no command specified, run interactive shell
if [ $# -eq 0 ]; then
    # Create log directory
    mkdir -p /workspace/.agents-box/logs

    success "Container environment ready!"
    if [ "${AUTH_OK}" = "true" ]; then
        success "✅ Authentication detected - Claude will work immediately"
        success "📝 Available Claude commands:"
        success "   • claude-ask \"question\" - Ask Claude with logged response"
        success "   • claude-start - Interactive Claude CLI"
        success "   • claude-help - Show all available commands"
        success "   💡 Use claude-ask to see responses in TUI logs!"
    else
        warn "⚠️  No authentication detected"
        warn "📝 Set ANTHROPIC_API_KEY or mount authentication files"
    fi

    log "Starting interactive shell..."
    # Use sleep infinity to keep container running when not attached to TTY
    if [ -t 0 ]; then
        exec bash
    else
        log "No TTY detected, keeping container alive..."
        exec sleep infinity
    fi
else
    # Run the specified command
    log "Running command: $*"
    exec "$@"
fi
