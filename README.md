# AI Coder Rules

A monorepo for AI coding agent tooling and infrastructure.

## Structure

```
ai-coder-rules/
├── toolkit/              # AI agent skills, rules, and workflows
├── claude-in-a-box/      # (coming soon) Rust TUI for Claude Code containers
└── agent-bridge/         # (coming soon) Browser debug bridge for LLM agents
```

## Toolkit

The `toolkit/` directory contains a complete AI coding agent toolkit:

- **Skills**: Reusable capabilities (webapp-testing, crypto-research, tmux-monitor, etc.)
- **Agents**: Specialized AI agent definitions (backend-developer, frontend-developer, tech-lead-orchestrator, etc.)
- **Commands**: Slash commands for structured workflows (/plan, /implement, /validate, etc.)
- **Rules**: Multi-tool rules for various AI coding assistants (Claude Code, Cursor, Amazon Q, Gemini, etc.)

### Quick Start

```bash
cd toolkit
npm install
node create-rule.js --tool=claude-code
```

See [toolkit/README.md](toolkit/README.md) for detailed documentation.

## Apps (Coming Soon)

### claude-in-a-box

A Rust TUI application for managing Claude Code containers with Docker. Features:
- Container lifecycle management
- tmux session orchestration
- MCP server integration
- Toolkit bootstrap integration

### agent-bridge

A browser debug bridge enabling LLM agents to interact with web applications via WebSocket:
- Chrome DevTools Protocol integration
- Screenshot capture
- DOM inspection
- Console log access

## License

Apache-2.0
