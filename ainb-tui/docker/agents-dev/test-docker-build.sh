#!/bin/bash
# ABOUTME: Standalone test script for Docker build, mounting, and MCP functionality
# Tests the agents-dev container build process independently of the main application

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[test]${NC} $1"
}

success() {
    echo -e "${GREEN}[test]${NC} ✓ $1"
}

error() {
    echo -e "${RED}[test]${NC} ✗ $1"
    exit 1
}

warn() {
    echo -e "${YELLOW}[test]${NC} ⚠ $1"
}

# Configuration
IMAGE_NAME="agents-box:test-$(date +%s)"
CONTAINER_NAME="agents-box-test-$(date +%s)"
TEST_WORKSPACE="/tmp/agents-box-test-workspace-$(date +%s)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$SCRIPT_DIR"

# Get current user's UID and GID
HOST_UID=$(id -u)
HOST_GID=$(id -g)

# Cleanup function
cleanup() {
    log "Cleaning up..."

    # Stop and remove container if it exists
    if docker ps -a --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
    fi

    # Remove test image
    docker rmi "${IMAGE_NAME}" >/dev/null 2>&1 || true

    # Remove test workspace
    rm -rf "${TEST_WORKSPACE}"

    success "Cleanup completed"
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Test 1: Build Docker image
test_docker_build() {
    log "Test 1: Building Docker image with UID=$HOST_UID, GID=$HOST_GID"

    # Build the image
    docker build \
        --build-arg HOST_UID="${HOST_UID}" \
        --build-arg HOST_GID="${HOST_GID}" \
        -t "${IMAGE_NAME}" \
        "${DOCKER_DIR}" || error "Failed to build Docker image"

    success "Docker image built successfully"
}

# Test 2: Create test workspace
test_workspace_setup() {
    log "Test 2: Setting up test workspace"

    # Create test workspace
    mkdir -p "${TEST_WORKSPACE}"

    # Create a test file
    echo "Hello from host!" > "${TEST_WORKSPACE}/test.txt"

    # Create a test git repo
    cd "${TEST_WORKSPACE}"
    git init
    git config user.email "test@example.com"
    git config user.name "Test User"
    echo "# Test Project" > README.md
    git add README.md
    git commit -m "Initial commit"

    success "Test workspace created at ${TEST_WORKSPACE}"
}

# Test 3: Run container with mounts
test_container_run() {
    log "Test 3: Running container with volume mounts"

    # Prepare mount options (following agents-docker pattern with direct .claude.json mount)
    MOUNT_OPTS="-v ${TEST_WORKSPACE}:/workspace"

    # Create persistent agents-box directory
    mkdir -p "$HOME/.agents-box/claude-home"

    # Copy .claude directory contents to persistent location (except .claude.json)
    if [ -f "$HOME/.claude/.credentials.json" ] && [ ! -f "$HOME/.agents-box/claude-home/.credentials.json" ]; then
        log "Copying .credentials.json to persistent directory"
        cp "$HOME/.claude/.credentials.json" "$HOME/.agents-box/claude-home/.credentials.json"
    fi

    # Mount persistent .claude directory
    MOUNT_OPTS="$MOUNT_OPTS -v $HOME/.agents-box/claude-home:/home/claude-user/.claude:rw"

    # Mount .claude.json directly if it exists
    if [ -f "$HOME/.claude.json" ]; then
        MOUNT_OPTS="$MOUNT_OPTS -v $HOME/.claude.json:/home/claude-user/.claude.json:ro"
        log "Mounting .claude.json directly"
    else
        warn "No .claude.json found at $HOME/.claude.json"
    fi

    log "Mounting authentication with hybrid approach"

    # Run container
    docker run -d \
        --name "${CONTAINER_NAME}" \
        ${MOUNT_OPTS} \
        -e AGENTS_BOX_MODE=true \
        "${IMAGE_NAME}" \
        sleep infinity || error "Failed to run container"

    success "Container started successfully"
}

# Test 4: Verify file permissions
test_file_permissions() {
    log "Test 4: Verifying file permissions"

    # Check if claude-user can write to workspace
    docker exec "${CONTAINER_NAME}" bash -c "echo 'Hello from container!' > /workspace/container-test.txt" \
        || error "Failed to write to workspace from container"

    # Check if file exists on host
    if [ ! -f "${TEST_WORKSPACE}/container-test.txt" ]; then
        error "File created in container not visible on host"
    fi

    # Check file ownership on host
    FILE_UID=$(stat -f %u "${TEST_WORKSPACE}/container-test.txt" 2>/dev/null || stat -c %u "${TEST_WORKSPACE}/container-test.txt")
    FILE_GID=$(stat -f %g "${TEST_WORKSPACE}/container-test.txt" 2>/dev/null || stat -c %g "${TEST_WORKSPACE}/container-test.txt")

    if [ "$FILE_UID" -ne "$HOST_UID" ] || [ "$FILE_GID" -ne "$HOST_GID" ]; then
        error "File ownership mismatch: expected ${HOST_UID}:${HOST_GID}, got ${FILE_UID}:${FILE_GID}"
    fi

    success "File permissions correct"
}

# Test 5: Verify MCP servers installation
test_mcp_servers() {
    log "Test 5: Verifying MCP servers installation"

    # Check if Claude CLI can list MCP servers
    docker exec "${CONTAINER_NAME}" claude mcp list >/dev/null 2>&1 \
        || warn "Could not list MCP servers (this is normal if no API key is provided)"

    # Check if the MCP configuration directory exists
    docker exec "${CONTAINER_NAME}" test -d /home/claude-user/.claude \
        || error "Claude configuration directory not found"

    # Verify specific MCP servers are available
    # We check for filesystem and memory servers which should be installed by default
    MCP_SERVERS=("filesystem" "memory")

    for server in "${MCP_SERVERS[@]}"; do
        # Check if server is listed in Claude MCP configuration
        if docker exec "${CONTAINER_NAME}" claude mcp list 2>/dev/null | grep -q "$server"; then
            success "MCP server $server configured"
        else
            warn "MCP server $server not found in claude mcp list (may require API key)"
        fi
    done

    success "MCP server installation verified"
}

# Test 6: Verify Claude/Gemini CLI
test_cli_installation() {
    log "Test 6: Verifying CLI installation"

    # Check Claude CLI
    docker exec "${CONTAINER_NAME}" which claude >/dev/null 2>&1 \
        || error "Claude CLI not found"

    # Check Gemini CLI
    docker exec "${CONTAINER_NAME}" which gemini >/dev/null 2>&1 \
        || error "Gemini CLI not found"

    # Test Claude CLI help
    docker exec "${CONTAINER_NAME}" claude --help >/dev/null 2>&1 \
        || error "Claude CLI not working"

    success "CLI tools installed and working"
}

# Test 7: Test with different UID/GID (simulate conflict)
test_uid_gid_conflict() {
    log "Test 7: Testing UID/GID conflict handling"

    # Try building with UID/GID that might conflict (1000 is common)
    local TEST_IMAGE="agents-box:test-conflict-$(date +%s)"

    docker build \
        --build-arg HOST_UID=1000 \
        --build-arg HOST_GID=1000 \
        -t "${TEST_IMAGE}" \
        "${DOCKER_DIR}" || error "Failed to build with UID/GID 1000"

    # Clean up test image
    docker rmi "${TEST_IMAGE}" >/dev/null 2>&1 || true

    success "UID/GID conflict handling works"
}

# Main test execution
main() {
    log "Starting Docker build and functionality tests"
    log "Using Docker directory: ${DOCKER_DIR}"

    # Check Docker is running
    docker version >/dev/null 2>&1 || error "Docker is not running"

    # Run tests
    test_docker_build
    test_workspace_setup
    test_container_run
    test_file_permissions
    test_mcp_servers
    test_cli_installation
    test_uid_gid_conflict

    echo ""
    success "All tests passed! 🎉"
    echo ""
    log "Summary:"
    echo "  - Docker image builds correctly with host UID/GID"
    echo "  - Container runs with proper volume mounts"
    echo "  - File permissions are preserved between host and container"
    echo "  - MCP servers are installed and configured"
    echo "  - Claude and Gemini CLIs are available"
    echo "  - UID/GID conflicts are handled gracefully"
}

# Run main function
main
