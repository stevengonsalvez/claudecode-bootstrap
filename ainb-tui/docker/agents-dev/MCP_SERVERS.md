# MCP Server Management

This document explains how to add and manage MCP (Model Context Protocol) servers in the claude-dev container.

## Quick Start

To add a new MCP server:

1. Edit `config/mcp-servers.txt`
2. Add your MCP server installation command
3. Rebuild the Docker image

## File Structure

- `config/mcp-servers.txt` - List of MCP server installation commands
- `scripts/install-mcp-servers.sh` - Script that processes and installs MCP servers
- `.env` - Environment variables (for MCP servers that need API keys)

## Adding MCP Servers

### Simple MCP Servers (No Environment Variables)

⚠️ **IMPORTANT**: Always use `-s user` flag to make MCPs available across all projects!

Add a line like this to `config/mcp-servers.txt`:

```bash
claude mcp add -s user <name> -- <command> <args>
```

Example:

```bash
claude mcp add -s user filesystem -- npx -y @modelcontextprotocol/server-filesystem
```

**Without `-s user`**: MCP will only be available in the Docker build directory (`/app`)
**With `-s user`**: MCP will be available in any project directory (`/workspace`, etc.)

### MCP Servers with Environment Variables

For servers that need API keys or configuration:

```bash
claude mcp add-json <name> -s user '{"command":"...","args":[...],"env":{"KEY":"${ENV_VAR}"}}'
```

Example:

```bash
claude mcp add-json github -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"${GITHUB_TOKEN}"}}'
```

## Environment Variables

1. Add required variables to `.env`:

```env
GITHUB_TOKEN=your_token_here
ANTHROPIC_API_KEY=your_key_here
```

2. Reference them in `config/mcp-servers.txt` using `${VAR_NAME}` syntax

3. The install script will:
   - Skip servers with missing required env vars
   - Log which variables are missing
   - Continue installing other servers

## Default MCP Servers

The following servers are enabled by default:

- **Filesystem** - Read/write files in the workspace
- **Memory** - Persistent memory across conversations

The following servers are available but commented out (uncomment to enable):

- **GitHub** - GitHub integration (requires GITHUB_TOKEN)
- **Browser** - Browser automation for web scraping
- **PostgreSQL** - Database integration (requires DATABASE_URL)
- **Twilio** - SMS messaging (requires TWILIO_* env vars)

## Examples of Popular MCP Servers

```bash
# Filesystem access
claude mcp add -s user filesystem -- npx -y @modelcontextprotocol/server-filesystem

# GitHub integration
claude mcp add-json github -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"${GITHUB_TOKEN}"}}'

# Browser automation
claude mcp add -s user browser -- npx -y @modelcontextprotocol/server-browser

# Memory/knowledge base
claude mcp add -s user memory -- npx -y @modelcontextprotocol/server-memory

# PostgreSQL database
claude mcp add-json postgres -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-postgres"],"env":{"POSTGRES_URL":"${DATABASE_URL}"}}'

# Slack integration
claude mcp add-json slack -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-slack"],"env":{"SLACK_BOT_TOKEN":"${SLACK_BOT_TOKEN}"}}'

# Google Drive
claude mcp add -s user gdrive -- npx -y @modelcontextprotocol/server-gdrive

# SQLite database
claude mcp add -s user sqlite -- npx -y @modelcontextprotocol/server-sqlite
```

## Troubleshooting

### MCP Server Not Installing

- Check if required environment variables are set in `.env`
- Run the docker build to rebuild with latest changes
- Check Docker build logs for error messages

### Finding MCP Server Commands

Most MCP servers provide installation instructions on their GitHub pages. Look for:

- `claude mcp add` commands
- `npx` commands that can be wrapped in `claude mcp add`
- JSON configurations for servers with environment variables

### Debugging

The install script logs:

- Which servers are being installed
- Missing environment variables
- Success/failure for each installation
- Continues even if one server fails

## Environment Variable Examples

Common environment variables you might need:

```env
# Claude API
ANTHROPIC_API_KEY=sk-ant-...

# GitHub integration
GITHUB_TOKEN=ghp_...

# PostgreSQL database
DATABASE_URL=postgresql://user:pass@host:5432/db

# Twilio SMS
TWILIO_AUTH_TOKEN=...
TWILIO_ACCOUNT_SID=...
TWILIO_FROM_PHONE=+1234567890

# Slack integration
SLACK_BOT_TOKEN=xoxb-...

# OpenAI (for some MCP servers)
OPENAI_API_KEY=sk-...
```
