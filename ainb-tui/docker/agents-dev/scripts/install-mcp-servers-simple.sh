#!/bin/bash
# Simple MCP server installation for debugging

echo "Starting simple MCP installation..."

# Try to add filesystem server
echo "Adding filesystem MCP server..."
claude mcp add -s user filesystem -- npx -y @modelcontextprotocol/server-filesystem

echo "Installation completed successfully"
exit 0
