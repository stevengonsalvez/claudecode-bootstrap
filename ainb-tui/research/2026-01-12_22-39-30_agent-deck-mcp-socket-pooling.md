# Research: Agent-Deck MCP Socket Pooling Architecture

**Date**: 2026-01-12T22:39:30+00:00
**Repository**: stevengonsalvez/agents-in-a-box
**Branch**: main
**Commit**: 7a7556697b2699531024bc0c094e22bf10e12d0d
**Research Type**: Web | GitHub Repository Analysis

## Research Question

How does agent-deck handle MCP using Unix sockets to have one set of MCP servers shared across multiple sessions?

## Executive Summary

Agent-deck implements **MCP Socket Pooling** - a pattern where a single MCP server process is shared across multiple Claude Code sessions via Unix domain sockets. Instead of each session spawning its own MCP servers, a socket proxy wraps each MCP process and exposes it at `/tmp/agentdeck-mcp-{name}.sock`. Sessions connect using `nc -U` (netcat) as a transparent bridge. This achieves **85-90% memory savings** for users running 10+ concurrent sessions.

## Key Findings

- **Unix socket proxy pattern**: Wrap stdio-based MCP processes, expose via Unix sockets, route JSON-RPC by request ID
- **Netcat bridge**: Generate Claude Code config using `nc -U /path/to/socket` as the command - Claude thinks it's stdio
- **Request ID routing**: Maintain `map[requestID]sessionID` to route responses to correct clients; broadcast notifications to all
- **Multi-instance coordination**: Discover and reuse sockets from other agent-deck instances via glob pattern matching
- **Health monitoring**: 10-second health checks with rate-limited restarts (5s minimum, 3 max/minute)

## Detailed Findings

### Source Repository

**GitHub**: [asheshgoplani/agent-deck](https://github.com/asheshgoplani/agent-deck)
**Language**: Go 1.24 with Bubble Tea TUI framework

### Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Claude Session 1├────►│                 │     │                 │
│  nc -U socket   │     │   SocketProxy   │     │   MCP Process   │
└─────────────────┘     │                 │────►│   (npx ...)     │
                        │  Unix Socket:   │     │                 │
┌─────────────────┐     │  /tmp/agent..   │     │   stdin/stdout  │
│ Claude Session 2├────►│                 │◄────│                 │
│  nc -U socket   │     │  Request ID     │     │                 │
└─────────────────┘     │  routing table  │     └─────────────────┘
                        │                 │
┌─────────────────┐     │  requestMap:    │
│ Claude Session N├────►│  ID -> session  │
│  nc -U socket   │     │                 │
└─────────────────┘     └─────────────────┘
```

### Key Source Files

| File | Purpose |
|------|---------|
| `internal/mcppool/pool_simple.go` | Pool management, server lifecycle, health monitoring |
| `internal/mcppool/socket_proxy.go` | Unix socket proxy with JSON-RPC multiplexing |
| `internal/mcppool/types.go` | ServerStatus enum (stopped/starting/running/failed) |
| `internal/session/pool_manager.go` | Global pool singleton, platform detection |
| `internal/session/mcp_catalog.go` | Config generation for Claude Code |
| `internal/session/userconfig.go` | TOML config parsing |

### Core Data Structures

**Pool** (`pool_simple.go`):
```go
type Pool struct {
    proxies map[string]*SocketProxy  // MCP name -> socket proxy
    config  *PoolConfig              // Pool settings
}
```

**SocketProxy** (`socket_proxy.go`):
```go
type SocketProxy struct {
    mcpProcess *exec.Cmd              // Single MCP process
    mcpStdin   io.WriteCloser         // Forward requests here
    mcpStdout  io.ReadCloser          // Read responses here

    clients    map[string]net.Conn    // Multiple Claude sessions
    requestMap map[interface{}]string // REQUEST ID -> SESSION ID
}
```

### JSON-RPC Request Routing

The multiplexing mechanism:

1. **On client request**: Store `requestID -> sessionID` mapping, forward to MCP stdin
   ```go
   if req.ID != nil {
       p.requestMap[req.ID] = sessionID
   }
   p.mcpStdin.Write(line)
   ```

2. **On MCP response**: Look up session by request ID, route to correct client
   ```go
   sessionID := p.requestMap[responseID]
   delete(p.requestMap, responseID)  // Cleanup after use
   conn := p.clients[sessionID]
   conn.Write(line)
   ```

3. **Notifications** (no ID): Broadcast to all connected clients

### Claude Code Integration via Netcat

The elegant bridge to Claude Code's stdio-based MCP:

**When socket pooling is active**:
```go
mcpConfig.MCPServers[name] = MCPServerConfig{
    Command: "nc",
    Args:    []string{"-U", "/tmp/agentdeck-mcp-memory.sock"},
}
```

**Fallback to stdio**:
```go
mcpConfig.MCPServers[name] = MCPServerConfig{
    Type:    "stdio",
    Command: def.Command,
    Args:    args,
    Env:     env,
}
```

### Configuration Schema

**TOML config** (`~/.agent-deck/config.toml`):

```toml
[mcp_pool]
enabled = true              # Master switch for socket pooling
pool_all = true             # Pool ALL defined MCPs
exclude_mcps = ["chrome"]   # Exclude specific MCPs from pooling
fallback_to_stdio = true    # Fall back if socket fails
socket_wait_timeout = 5     # Seconds to wait for socket ready

[mcps.exa]
command = "npx"
args = ["-y", "exa-mcp-server"]
env = { EXA_API_KEY = "your-key" }
description = "Web search via Exa AI"

[mcps.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
description = "Persistent memory"
```

### Health Monitoring

- **10-second interval** background health checks
- **Socket validation**: Check file existence + connection acceptance (500ms timeout)
- **Rate-limited restarts**: 5-second minimum between restarts, max 3 per minute
- **Stale socket cleanup**: Remove socket files before restart attempts
- **Status enum**: stopped → starting → running ↔ failed

### Multi-Instance Coordination

Multiple agent-deck instances can share the same pool:

```go
func DiscoverExistingSockets() {
    matches, _ := filepath.Glob("/tmp/agentdeck-mcp-*.sock")
    for _, socketPath := range matches {
        if isSocketAlive(socketPath) {
            RegisterExternalSocket(name, socketPath)  // Reuse without spawning
        }
    }
}
```

External sockets are marked read-only and not terminated when the registering instance shuts down.

### Platform Support

| Platform | Socket Pooling | Notes |
|----------|----------------|-------|
| macOS | ✅ Full support | Primary development platform |
| Linux | ✅ Full support | |
| WSL2 | ✅ Full support | |
| WSL1 | ❌ Auto-disabled | Falls back to stdio |
| Windows | ❌ Auto-disabled | Falls back to stdio |

### Initialization Flow

```
1. TUI Startup (main.go)
   └── ui.NewHomeWithProfile(profile)

2. Home.loadSessions()
   ├── storage.LoadWithGroups()
   └── session.InitializeGlobalPool(ctx, userConfig, instances)
       ├── Check platform compatibility
       ├── pool.DiscoverExistingSockets()  // Reuse from other instances
       └── For each MCP where ShouldPool() == true:
           └── pool.Start(name, command, args, env)
               └── NewSocketProxy() -> Creates /tmp/agentdeck-mcp-{name}.sock

3. Session Start/Restart (instance.go)
   └── WriteMCPJsonFromConfig(projectPath, mcpNames)
       └── For each MCP:
           ├── If pool.IsRunning(name):
           │   └── Write: { "command": "nc", "args": ["-U", socketPath] }
           └── Else (fallback):
               └── Write: { "command": "npx", "args": [...], "env": {...} }
```

## MCP Transport Context

### Official MCP Transports

Per the [MCP Specification (2025-06-18)](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports):

- **stdio**: Client launches MCP server as subprocess, communicates via stdin/stdout
- **Streamable HTTP**: HTTP POST/GET with optional SSE for streaming

**Unix sockets are NOT official** but are explicitly supported as custom transports.

### Alternative MCP Proxy Projects

| Project | Language | Purpose |
|---------|----------|---------|
| [sparfenyuk/mcp-proxy](https://github.com/sparfenyuk/mcp-proxy) | Python | stdio ↔ HTTP/SSE bridge |
| [TBXark/mcp-proxy](https://github.com/TBXark/mcp-proxy) | Go | Aggregate multiple MCPs into one HTTP endpoint |
| [ECF/MCPTransports](https://github.com/ECF/MCPTransports) | Java | Unix Domain Socket transport for Java SDK |
| [Martian-Engineering/rust-mcp](https://github.com/Martian-Engineering/rust-mcp) | Rust | Native Unix transport support |

## Implementation Recommendations for ainb-tui

### Core Pattern

1. **Socket Proxy in Rust**:
   ```rust
   struct SocketProxy {
       mcp_process: Child,
       mcp_stdin: ChildStdin,
       mcp_stdout: ChildStdout,
       clients: HashMap<String, UnixStream>,
       request_map: HashMap<serde_json::Value, String>, // ID -> session
   }
   ```

2. **Request ID Tracking**:
   - Parse JSON-RPC, extract `id` field
   - Store `id -> client_id` before forwarding to MCP
   - On response, route to correct client, then delete mapping
   - Notifications (no `id`): broadcast to all clients

3. **Netcat Bridge**:
   - Generate `.mcp.json` with `nc -U /tmp/ainb-mcp-{name}.sock`
   - Or use Rust's `std::os::unix::net::UnixStream` directly if building custom client

4. **Health Monitoring**:
   - Background task checking socket liveness every 10s
   - Rate-limit restarts to prevent thrashing
   - Graceful degradation to stdio on failure

5. **Platform Detection**:
   - `#[cfg(unix)]` for socket support
   - Auto-disable on Windows, provide stdio fallback

### Suggested File Structure

```
src/mcp_pool/
├── mod.rs           # Pool management, public API
├── socket_proxy.rs  # Unix socket proxy with JSON-RPC routing
├── health.rs        # Health monitoring and restart logic
├── config.rs        # Pool configuration from TOML
└── catalog.rs       # Generate .mcp.json for sessions
```

### Benefits

- **85-90% memory savings** for 10+ concurrent sessions
- **Faster session startup** - MCP servers already running
- **Consistent state** - memory MCP shared across sessions
- **Multi-instance support** - multiple TUI instances share pool

## Open Questions

1. **Session isolation**: Should memory MCP be shared, or is per-session state desired?
2. **Socket permissions**: What file permissions for `/tmp/ainb-mcp-*.sock`?
3. **Cleanup on crash**: How to handle stale sockets from crashed processes?
4. **WSL1 detection**: How to reliably detect WSL1 vs WSL2?

## References

### Primary Sources
- [GitHub - asheshgoplani/agent-deck](https://github.com/asheshgoplani/agent-deck)
- [MCP Transports Specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [MCP Architecture](https://modelcontextprotocol.io/specification/2025-06-18/architecture)

### Related Projects
- [sparfenyuk/mcp-proxy](https://github.com/sparfenyuk/mcp-proxy)
- [TBXark/mcp-proxy](https://github.com/TBXark/mcp-proxy)
- [ECF/MCPTransports](https://github.com/ECF/MCPTransports)
- [Martian-Engineering/rust-mcp](https://github.com/Martian-Engineering/rust-mcp)
- [OpenAI Codex stdio-to-uds PR](https://github.com/openai/codex/pull/5350)

### Documentation
- [Anthropic MCP Introduction](https://www.anthropic.com/news/model-context-protocol)
- [MCP Blog - Future of Transports](http://blog.modelcontextprotocol.io/posts/2025-12-19-mcp-transport-future/)
- [Stainless - Custom MCP Transport Implementation](https://www.stainless.com/mcp/custom-mcp-transport-implementation)
