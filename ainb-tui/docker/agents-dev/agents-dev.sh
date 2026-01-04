#!/bin/bash
# ABOUTME: Wrapper script to run agents-dev container with proper mounting and authentication
# Optimized for agents-in-a-box with GITHUB_TOKEN support and gh CLI integration

# Parse command line arguments
NO_CACHE=""
FORCE_REBUILD=false
CONTINUE_FLAG=""
MEMORY_LIMIT=""
GPU_ACCESS=""
ARGS=()

show_help() {
    cat << EOF
Agents-in-a-Box Development Environment

USAGE:
    ./agents-dev.sh [OPTIONS] [COMMAND]

OPTIONS:
    --help          Show this help message
    --continue      Continue from last Claude session
    --rebuild       Force rebuild of Docker image
    --no-cache      Build Docker image without cache (use with --rebuild)
    --memory SIZE   Set memory limit (e.g., 4g, 2048m)
    --gpus TYPE     Enable GPU access (e.g., all, device=0)

AUTHENTICATION:
    Preferred: Set GITHUB_TOKEN environment variable or in .env file
    Fallback: SSH keys in ~/.agents-box/ssh/

EXAMPLES:
    ./agents-dev.sh                    # Start Claude CLI in container
    ./agents-dev.sh --continue         # Continue from last session
    ./agents-dev.sh --rebuild          # Rebuild and start
    ./agents-dev.sh --memory 4g        # Start with 4GB memory limit
    ./agents-dev.sh bash               # Run bash in container

For more information, see the README or documentation.
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --help|-h)
            show_help
            exit 0
            ;;
        --continue)
            CONTINUE_FLAG="--continue"
            shift
            ;;
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        --rebuild)
            FORCE_REBUILD=true
            shift
            ;;
        --memory)
            MEMORY_LIMIT="$2"
            shift 2
            ;;
        --gpus)
            GPU_ACCESS="$2"
            shift 2
            ;;
        *)
            ARGS+=("$1")
            shift
            ;;
    esac
done

# Get the absolute path of the current directory
CURRENT_DIR=$(pwd)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Check if .env exists in docker directory for building
ENV_FILE="$SCRIPT_DIR/.env"
if [ -f "$ENV_FILE" ]; then
    echo "✓ Found .env file with credentials"
    # Source .env to get configuration variables
    set -a
    source "$ENV_FILE" 2>/dev/null || true
    set +a
else
    echo "⚠️  No .env file found at $ENV_FILE"
    echo "   To enable MCP servers requiring API keys:"
    echo "   Copy .env.example to .env and add your credentials"
fi

# Use environment variables as defaults if command line args not provided
if [ -z "$MEMORY_LIMIT" ] && [ -n "$DOCKER_MEMORY_LIMIT" ]; then
    MEMORY_LIMIT="$DOCKER_MEMORY_LIMIT"
    echo "✓ Using memory limit from environment: $MEMORY_LIMIT"
fi

if [ -z "$GPU_ACCESS" ] && [ -n "$DOCKER_GPU_ACCESS" ]; then
    GPU_ACCESS="$DOCKER_GPU_ACCESS"
    echo "✓ Using GPU access from environment: $GPU_ACCESS"
fi

# Check if we need to rebuild the image
NEED_REBUILD=false
IMAGE_NAME="agents-box:agents-dev"

if ! docker images | grep -q "agents-box.*agents-dev"; then
    echo "Building agents-box:agents-dev image for first time..."
    NEED_REBUILD=true
fi

if [ "$FORCE_REBUILD" = true ]; then
    echo "Forcing rebuild of agents-box:agents-dev image..."
    NEED_REBUILD=true
fi

if [ "$NEED_REBUILD" = true ]; then
    # Build docker command with host UID/GID
    BUILD_ARGS="--build-arg HOST_UID=$(id -u) --build-arg HOST_GID=$(id -g)"

    # Add environment variables from .env if they exist
    if [ -n "$ANTHROPIC_API_KEY" ]; then
        BUILD_ARGS="$BUILD_ARGS --build-arg ANTHROPIC_API_KEY=\"$ANTHROPIC_API_KEY\""
    fi

    echo "Building image: $IMAGE_NAME"
    eval "docker build $NO_CACHE $BUILD_ARGS -t $IMAGE_NAME \"$SCRIPT_DIR\""
fi

# Ensure the agents-box directories exist (following agents-docker pattern)
mkdir -p "$HOME/.agents-box/claude-home"
mkdir -p "$HOME/.agents-box/ssh"

# Sync authentication files to persistent directory only if needed
# Check if we need to sync authentication files
SYNC_NEEDED=false

# Check .claude.json first (primary authentication file)
if [ -f "$HOME/.claude.json" ]; then
    if [ ! -f "$HOME/.agents-box/claude-home/.claude.json" ] || [ "$HOME/.claude.json" -nt "$HOME/.agents-box/claude-home/.claude.json" ]; then
        SYNC_NEEDED=true
    fi
fi

# Check .claude directory contents
if [ -d "$HOME/.claude" ]; then
    # Only sync if persistent directory is empty or outdated
    if [ ! -d "$HOME/.agents-box/claude-home" ] || [ ! -f "$HOME/.agents-box/claude-home/.credentials.json" ]; then
        SYNC_NEEDED=true
    elif [ "$HOME/.claude" -nt "$HOME/.agents-box/claude-home" ]; then
        SYNC_NEEDED=true
    fi
fi

if [ "$SYNC_NEEDED" = true ]; then
    echo "✓ Syncing Claude configuration to persistent directory"

    # Sync .claude.json if it exists
    if [ -f "$HOME/.claude.json" ]; then
        cp "$HOME/.claude.json" "$HOME/.agents-box/claude-home/.claude.json"
    fi

    # Sync .claude directory contents if they exist
    if [ -d "$HOME/.claude" ]; then
        # Use rsync for efficient sync, or cp if rsync not available
        if command -v rsync >/dev/null 2>&1; then
            rsync -a "$HOME/.claude/" "$HOME/.agents-box/claude-home/"
        else
            cp -r "$HOME/.claude/." "$HOME/.agents-box/claude-home/" 2>/dev/null || true
        fi
    fi
fi

# Log information about persistent Claude home directory
echo ""
echo "📁 Claude persistent home directory: ~/.agents-box/claude-home/"
echo "   This directory contains Claude's settings and authentication"
echo "   Modify files here to customize Claude's behavior across all sessions"
echo ""

# Check Git authentication setup (GITHUB_TOKEN first, SSH fallback)
echo ""
echo "🔐 Git Authentication Setup"

if [ -n "$GITHUB_TOKEN" ]; then
    echo "✓ GITHUB_TOKEN found - will use token-based authentication"
    echo "   This enables:"
    echo "   • Git operations (clone, push, pull)"
    echo "   • GitHub CLI (gh) commands for issues/PRs"
    echo "   • No SSH key setup required"
    echo ""
else
    echo "⚠️  GITHUB_TOKEN not found"
    echo "   To enable full GitHub integration:"
    echo ""
    echo "   1. Create GitHub Personal Access Token:"
    echo "      https://github.com/settings/tokens/new"
    echo "      Required scopes: repo, read:org, workflow"
    echo ""
    echo "   2. Add to .env file:"
    echo "      GITHUB_TOKEN=ghp_your_token_here"
    echo ""
    echo "   3. Or export environment variable:"
    echo "      export GITHUB_TOKEN=ghp_your_token_here"
    echo ""

    # Check SSH keys as fallback
    SSH_KEY_PATH="$HOME/.agents-box/ssh/id_rsa"
    SSH_PUB_KEY_PATH="$HOME/.agents-box/ssh/id_rsa.pub"

    if [ -f "$SSH_KEY_PATH" ] && [ -f "$SSH_PUB_KEY_PATH" ]; then
        echo "✓ SSH keys found as fallback for git operations"

        # Create SSH config if it doesn't exist
        SSH_CONFIG_PATH="$HOME/.agents-box/ssh/config"
        if [ ! -f "$SSH_CONFIG_PATH" ]; then
            cat > "$SSH_CONFIG_PATH" << 'EOF'
Host github.com
    HostName github.com
    User git
    IdentityFile ~/.ssh/id_rsa
    IdentitiesOnly yes

Host gitlab.com
    HostName gitlab.com
    User git
    IdentityFile ~/.ssh/id_rsa
    IdentitiesOnly yes
EOF
            echo "✓ SSH config created"
        fi
    else
        echo "   Alternative: Generate SSH keys:"
        echo "      ssh-keygen -t rsa -b 4096 -f ~/.agents-box/ssh/id_rsa -N ''"
        echo "      Then add public key to GitHub/GitLab"
        echo ""
        echo "   Note: GITHUB_TOKEN is recommended for better integration"
    fi
fi

# Prepare Docker run arguments
DOCKER_OPTS=""
ENV_ARGS=""

# Add memory limit if specified
if [ -n "$MEMORY_LIMIT" ]; then
    echo "✓ Setting memory limit: $MEMORY_LIMIT"
    DOCKER_OPTS="$DOCKER_OPTS --memory $MEMORY_LIMIT"
fi

# Add GPU access if specified
if [ -n "$GPU_ACCESS" ]; then
    if docker info 2>/dev/null | grep -q nvidia || which nvidia-docker >/dev/null 2>&1; then
        echo "✓ Enabling GPU access: $GPU_ACCESS"
        DOCKER_OPTS="$DOCKER_OPTS --gpus $GPU_ACCESS"
    else
        echo "⚠️  GPU access requested but NVIDIA Docker runtime not found"
        echo "   Continuing without GPU access..."
    fi
fi

# Add environment variables from .env
if [ -n "$ANTHROPIC_API_KEY" ]; then
    ENV_ARGS="$ENV_ARGS -e ANTHROPIC_API_KEY=\"$ANTHROPIC_API_KEY\""
fi

if [ -n "$GITHUB_TOKEN" ]; then
    ENV_ARGS="$ENV_ARGS -e GITHUB_TOKEN=\"$GITHUB_TOKEN\""
fi

if [ -n "$TWILIO_AUTH_TOKEN" ]; then
    ENV_ARGS="$ENV_ARGS -e TWILIO_AUTH_TOKEN=\"$TWILIO_AUTH_TOKEN\""
fi

if [ -n "$TWILIO_ACCOUNT_SID" ]; then
    ENV_ARGS="$ENV_ARGS -e TWILIO_ACCOUNT_SID=\"$TWILIO_ACCOUNT_SID\""
fi

if [ -n "$TWILIO_FROM_PHONE" ]; then
    ENV_ARGS="$ENV_ARGS -e TWILIO_FROM_PHONE=\"$TWILIO_FROM_PHONE\""
fi

echo "Starting Claude CLI in agents-box container..."
echo "Container: $IMAGE_NAME"
echo "Workspace: $CURRENT_DIR"
echo ""

# Run the container with proper mounts
# All authentication files are now in persistent directory with read-write access

# Mount .claude.json separately if it exists in the persistent directory
CLAUDE_JSON_MOUNT=""
if [ -f "$HOME/.agents-box/claude-home/.claude.json" ]; then
    CLAUDE_JSON_MOUNT="-v $HOME/.agents-box/claude-home/.claude.json:/home/claude-user/.claude.json:rw"
    echo "✓ Mounting .claude.json for authentication"
fi

docker run -it --rm \
    $DOCKER_OPTS \
    -v "$CURRENT_DIR:/workspace" \
    -v "$HOME/.agents-box/claude-home:/home/claude-user/.claude:rw" \
    -v "$HOME/.agents-box/ssh:/home/claude-user/.ssh:rw" \
    $CLAUDE_JSON_MOUNT \
    $ENV_ARGS \
    -e AGENTS_BOX_MODE=true \
    -e CLAUDE_CONTINUE_FLAG="$CONTINUE_FLAG" \
    --workdir /workspace \
    --name "agents-box-$(basename "$CURRENT_DIR")-$$" \
    "$IMAGE_NAME" "${ARGS[@]}"
