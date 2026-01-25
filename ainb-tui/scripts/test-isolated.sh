#!/bin/bash
# scripts/test-isolated.sh
# Test ainb-tui with isolated HOME to avoid affecting live config

set -e

# Create isolated environment
TEST_HOME=$(mktemp -d)
# Don't auto-cleanup on exit so we can inspect the results
echo "NOTE: Test directory will be preserved for inspection"
echo "      Clean up manually with: rm -rf $TEST_HOME"
echo ""

echo "========================================"
echo "  Isolated Testing Environment"
echo "========================================"
echo ""
echo "Test HOME: $TEST_HOME"
echo ""

# Setup directory structure
mkdir -p "$TEST_HOME/.agents-in-a-box/config"
mkdir -p "$TEST_HOME/.agents-in-a-box/sockets"  # For MCP pool sockets
mkdir -p "$TEST_HOME/.config/agents-in-a-box"
mkdir -p "$TEST_HOME/.claude"

# Create test config with MCP definitions
cat > "$TEST_HOME/.agents-in-a-box/config/config.toml" << 'EOF'
[authentication]
default_model = "sonnet"

[session]
default_session_type = "tmux"
interactive_mode = false

[mcp_pool]
enabled = true
pool_all = true
max_clients = 10

[mcps.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp"]
description = "Context7 documentation lookup"
enabled = true
pool = true

[mcps.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
description = "Memory/knowledge graph MCP"
enabled = true
pool = true
EOF

echo "Test config created:"
echo "----------------------------------------"
cat "$TEST_HOME/.agents-in-a-box/config/config.toml"
echo "----------------------------------------"
echo ""

# Also create a mock Claude settings to test auto-import
cat > "$TEST_HOME/.claude/settings.json" << 'EOF'
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "env": {}
    }
  }
}
EOF

echo "Mock Claude settings created at $TEST_HOME/.claude/settings.json"
echo ""

echo "========================================"
echo "  What to Test"
echo "========================================"
echo ""
echo "1. TUI Startup:"
echo "   - Config > MCP Pool shows pool settings"
echo "   - MCP servers list shows context7, memory, filesystem"
echo ""
echo "2. Session Creation:"
echo "   - Create a new Interactive session"
echo "   - Check the worktree for .mcp.json file"
echo "   - Verify it has 'nc -U /path/to/socket' for pooled MCPs"
echo ""
echo "3. Claude Usage (in the session):"
echo "   - Run 'cat .mcp.json' to see the generated config"
echo "   - Claude should use pooled MCPs via socket"
echo ""
echo "4. After testing, inspect:"
echo "   ls -la $TEST_HOME/.agents-in-a-box/sockets/"
echo "   (Should show .sock files for pooled MCPs)"
echo ""
echo "========================================"
echo "  Running ainb-tui with isolated HOME"
echo "========================================"
echo ""
echo "Press Ctrl+C to exit when done testing"
echo ""

# Run with isolated home
HOME="$TEST_HOME" cargo run

echo ""
echo "========================================"
echo "  Test completed"
echo "========================================"
echo ""
echo "Inspect test artifacts:"
echo "  ls -la $TEST_HOME/.agents-in-a-box/sockets/"
echo "  cat $TEST_HOME/.agents-in-a-box/config/config.toml"
echo ""
echo "Clean up with:"
echo "  rm -rf $TEST_HOME"
