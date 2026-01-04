#!/bin/bash
# ABOUTME: MCP server installation script for claude-dev container
# Reads installation commands from config/mcp-servers.txt file

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[mcp-install]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[mcp-install]${NC} $1"
}

error() {
    echo -e "${RED}[mcp-install]${NC} $1"
}

success() {
    echo -e "${GREEN}[mcp-install]${NC} $1"
}

# Function to substitute environment variables in a string
substitute_env_vars() {
    local input="$1"

    # Try envsubst first
    if command -v envsubst >/dev/null 2>&1; then
        echo "$input" | envsubst
    else
        # Fallback: simple shell-based substitution
        local result="$input"

        # Find all ${VAR} patterns and substitute them
        while [[ $result =~ \$\{([A-Z_][A-Z0-9_]*)\} ]]; do
            local var_name="${BASH_REMATCH[1]}"
            local var_value="${!var_name}"
            result="${result//\$\{$var_name\}/$var_value}"
        done

        echo "$result"
    fi
}

# Function to check if a command contains required environment variables
check_required_env_vars() {
    local command="$1"
    local missing_vars=()

    # Extract environment variable references from the command
    local env_vars=$(echo "$command" | grep -oE '\$\{[A-Z_][A-Z0-9_]*\}' | sed 's/${//g' | sed 's/}//g' | sort -u)

    for var in $env_vars; do
        if [ -z "${!var}" ]; then
            missing_vars+=("$var")
        fi
    done

    if [ ${#missing_vars[@]} -ne 0 ]; then
        warn "Missing environment variables: ${missing_vars[*]}"
        return 1
    fi

    return 0
}

# Function to execute MCP installation command
execute_mcp_command() {
    local command="$1"

    log "Executing: $command"

    # Check for required environment variables
    if ! check_required_env_vars "$command"; then
        warn "Skipping command due to missing environment variables"
        return 0
    fi

    # Substitute environment variables
    local substituted_command=$(substitute_env_vars "$command")

    # Execute the command (disable exit-on-error temporarily)
    set +e
    eval "$substituted_command"
    local exit_code=$?
    set -e

    if [ $exit_code -eq 0 ]; then
        success "Command executed successfully"
        return 0
    else
        error "Command failed with exit code $exit_code: $substituted_command"
        # Continue with other installations instead of failing completely
        return 1
    fi
}

# Main installation
log "Installing MCP servers from config/mcp-servers.txt..."

# Check if the config file exists
if [ ! -f /app/config/mcp-servers.txt ]; then
    error "MCP servers configuration file not found: /app/config/mcp-servers.txt"
    exit 1
fi

# Read and process each line from the config file
installed_count=0
skipped_count=0

while IFS= read -r line; do
    # Skip empty lines and comments
    if [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]]; then
        continue
    fi

    # Execute the MCP installation command (continue on failure)
    set +e
    execute_mcp_command "$line"
    cmd_result=$?
    set -e

    if [ $cmd_result -eq 0 ]; then
        ((installed_count++))
    else
        ((skipped_count++))
    fi

done < /app/config/mcp-servers.txt

log "Installation summary: $installed_count installed, $skipped_count skipped"
success "MCP server installation completed"

# Note: MCP configuration is now handled by Claude CLI itself
# The claude mcp add commands automatically register servers
log "MCP servers are registered with Claude CLI and available for use"

# Ensure script exits successfully
exit 0
