# Claude Development Environment

This project is set up with an Agents-in-a-Box development environment.

## Available Tools

This environment includes the following MCP (Model Context Protocol) servers:

### Serena

An AI coding agent that can work alongside you to:

- Generate code and documentation
- Analyze codebases
- Suggest improvements
- Help with debugging

### Context7

Provides access to library documentation and code examples:

- Search for function signatures
- Get usage examples
- Access up-to-date documentation

### Additional Tools

- **Git integration**: Full git support with worktree isolation
- **Docker isolation**: Each session runs in its own container
- **Port forwarding**: Development servers are automatically accessible
- **Volume mounting**: Your workspace is mounted at `/workspace`

## Environment Information

- **Container**: agents-dev template
- **Base Image**: node:20-slim
- **User**: claude-user (UID/GID matched to host)
- **Working Directory**: /workspace
- **Node.js**: v20.x
- **Python**: 3.x
- **Git**: Available
- **Build Tools**: build-essential package

## Quick Start

1. Your project files are available at `/workspace`
2. Run development servers as usual - ports will be forwarded automatically
3. Git operations work normally within the container
4. Use Claude CLI with MCP servers for enhanced development experience

## Claude CLI Commands

The environment provides several ways to interact with Claude:

### Logged Commands (responses appear in TUI logs)

- `claude-ask "your question"` - Ask Claude a question with logged response
- `claude-print "query"` - Same as claude-ask
- `claude-script` - Read from stdin, useful for piping files

### Interactive Commands

- `claude-start` - Start persistent tmux session with Claude CLI
- `claude-interactive` - Alias for claude-start
- `claude` - Direct Claude CLI (respects permission settings)
- `claude-help` - Show all available commands

### Examples

```bash
# Ask Claude with logged response (visible in TUI)
claude-ask "How do I create a React component?"

# Pipe file contents to Claude
cat README.md | claude-script

# Ask about code
claude-ask "Explain this error: $(cat error.log)"
```

💡 **Tip**: Use `claude-ask` to see Claude's responses in the TUI live logs!

## Configuration

To customize this environment for your project:

1. Create `.agents-in-a-box/project.toml` in your project root
2. Specify container template, environment variables, and additional mounts
3. Restart your agents-box session to apply changes

Example project configuration:

```toml
container_template = "agents-dev"
mount_claude_config = true

[environment]
NODE_ENV = "development"
DEBUG = "myapp:*"

[[additional_mounts]]
host_path = "~/.ssh"
container_path = "/home/claude-user/.ssh"
read_only = true
```

Happy coding! 🚀
