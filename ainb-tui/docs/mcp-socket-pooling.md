# MCP Socket Pooling

## Overview

MCP (Model Context Protocol) servers are external processes that provide tools to Claude Code (e.g., `context7` for documentation, `exa` for search). By default, **each Claude Code session spawns its own MCP server processes**, which wastes memory when running multiple sessions.

**MCP Socket Pooling** shares a single MCP server process across multiple Claude Code sessions via Unix domain sockets, achieving **85-90% memory savings**.

---

## The Problem: Memory Waste

Without pooling, each session starts its own MCP processes:

```
+---------------------------------------------------------------------+
|                     WITHOUT POOLING                                  |
+---------------------------------------------------------------------+
|                                                                      |
|  +---------------+     +---------------------------------------+     |
|  | Claude Code   |---->| npx @anthropic/mcp-server-context7    |     |
|  |  Session 1    |     |         Process A (150MB)             |     |
|  +---------------+     +---------------------------------------+     |
|                                                                      |
|  +---------------+     +---------------------------------------+     |
|  | Claude Code   |---->| npx @anthropic/mcp-server-context7    |     |
|  |  Session 2    |     |         Process B (150MB)             |     |
|  +---------------+     +---------------------------------------+     |
|                                                                      |
|  +---------------+     +---------------------------------------+     |
|  | Claude Code   |---->| npx @anthropic/mcp-server-context7    |     |
|  |  Session 3    |     |         Process C (150MB)             |     |
|  +---------------+     +---------------------------------------+     |
|                                                                      |
|  Total Memory: 450MB for context7 alone                              |
|  With 5 MCPs x 3 sessions = 15 processes!                            |
|                                                                      |
+---------------------------------------------------------------------+
```

---

## The Solution: Socket Pooling

With pooling, one MCP process serves all sessions:

```
+---------------------------------------------------------------------+
|                      WITH POOLING                                    |
+---------------------------------------------------------------------+
|                                                                      |
|  +---------------+                                                   |
|  | Claude Code   |---+                                               |
|  |  Session 1    |   |                                               |
|  +---------------+   |     +--------------+                          |
|                      |     |              |                          |
|  +---------------+   +---->| Socket Pool  |                          |
|  | Claude Code   |-------->|   Proxy      |                          |
|  |  Session 2    |   +---->|              |                          |
|  +---------------+   |     +------+-------+                          |
|                      |            |                                  |
|  +---------------+   |            v                                  |
|  | Claude Code   |---+     +---------------------------------------+ |
|  |  Session 3    |         | npx @anthropic/mcp-server-context7    | |
|  +---------------+         |      Single Process (150MB)           | |
|                            +---------------------------------------+ |
|                                                                      |
|  Total Memory: 150MB (saved 300MB = 67% reduction)                   |
|  With 5 MCPs x 3 sessions = still only 5 processes!                  |
|                                                                      |
+---------------------------------------------------------------------+
```

---

## MCP Configuration

### McpDefinition (TOML Format)

MCP servers are configured in `~/.config/agents-in-a-box/config.toml` using a simple TOML format:

```toml
# Basic MCP server definition
[mcps.context7]
command = "npx"
args = ["-y", "@anthropic/mcp-server-context7"]
description = "Library documentation and code examples"
enabled = true    # Whether to use this MCP (default: true)
pool = true       # Include in socket pool (default: true)

# MCP with environment variables
[mcps.exa]
command = "npx"
args = ["-y", "exa-mcp-server"]
description = "Web search via Exa AI"
enabled = true
pool = true
required_env = ["EXA_API_KEY"]  # Warns if missing, skips MCP

[mcps.exa.env]
EXA_API_KEY = "${EXA_API_KEY}"

# MCP that should NOT be pooled (needs per-session state)
[mcps.puppeteer]
command = "npx"
args = ["-y", "@anthropic/mcp-server-puppeteer"]
description = "Browser automation"
enabled = true
pool = false  # Each session gets its own browser instance
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `command` | string | required | Command to run (e.g., `npx`, `uvx`, `node`) |
| `args` | array | `[]` | Command arguments |
| `env` | table | `{}` | Environment variables |
| `description` | string | `""` | Human-readable description |
| `enabled` | bool | `true` | Whether this MCP is active |
| `pool` | bool | `true` | Include in socket pool |
| `required_env` | array | `[]` | Required env vars (warns if missing) |

### Built-in Defaults

If no configuration exists, these MCPs are available by default:

| MCP | Description | Pooled |
|-----|-------------|--------|
| `context7` | Library documentation and code examples | Yes |
| `memory` | Persistent memory across sessions | Yes |
| `filesystem` | File system operations | Yes |
| `brave-search` | Web search via Brave (requires `BRAVE_API_KEY`) | Yes |
| `exa` | Web search via Exa AI (requires `EXA_API_KEY`) | Yes |
| `github` | GitHub operations (requires `GITHUB_PERSONAL_ACCESS_TOKEN`) | Yes |
| `puppeteer` | Browser automation | **No** (per-session state) |

---

## Auto-Import from Claude Configurations

### McpImporter

The TUI automatically imports MCP configurations from existing Claude installations:

```
+-----------------------------------------------------------------------+
|                        AUTO-IMPORT SOURCES                             |
+-----------------------------------------------------------------------+
|                                                                        |
|  Priority 1: Claude Desktop Config                                     |
|  +----------------------------------------------------------------+   |
|  | macOS:  ~/Library/Application Support/Claude/                  |   |
|  |         claude_desktop_config.json                             |   |
|  |                                                                 |   |
|  | Linux:  ~/.config/Claude/claude_desktop_config.json            |   |
|  +----------------------------------------------------------------+   |
|                                                                        |
|  Priority 2: Claude Code Settings                                      |
|  +----------------------------------------------------------------+   |
|  | ~/.claude/settings.json                                        |   |
|  | ~/.claude/settings.local.json (overrides)                      |   |
|  +----------------------------------------------------------------+   |
|                                                                        |
|  Priority 3: Project MCP Config                                        |
|  +----------------------------------------------------------------+   |
|  | <project>/.mcp.json                                            |   |
|  +----------------------------------------------------------------+   |
|                                                                        |
|  Result: All sources merged, later sources override earlier ones       |
|                                                                        |
+-----------------------------------------------------------------------+
```

### Import Process

1. **On TUI Startup**: Scans all known locations for MCP configs
2. **Parses JSON**: Extracts `mcpServers` entries from each config
3. **Converts to McpDefinition**: Maps command/args/env to internal format
4. **Merges**: Combines with TOML config (TOML takes precedence)
5. **Validates**: Checks `required_env` vars, warns for missing ones

### Example Imported Config

If your `~/.claude/settings.json` contains:

```json
{
  "mcpServers": {
    "my-custom-mcp": {
      "command": "node",
      "args": ["/path/to/my-mcp/index.js"],
      "env": {
        "API_KEY": "${MY_API_KEY}"
      }
    }
  }
}
```

It is automatically imported as:

```toml
# Auto-generated equivalent
[mcps.my-custom-mcp]
command = "node"
args = ["/path/to/my-mcp/index.js"]
description = "Imported: my-custom-mcp"
enabled = true
pool = true
required_env = ["MY_API_KEY"]

[mcps.my-custom-mcp.env]
API_KEY = "${MY_API_KEY}"
```

---

## Architecture with Netcat Bridge

### How Sessions Connect to Pooled MCPs

Claude Code sessions connect to pooled MCPs via a **netcat (nc) bridge**:

```
+--------------------------------------------------------------------------+
|                    MCP SOCKET POOL ARCHITECTURE                           |
+--------------------------------------------------------------------------+
|                                                                           |
|   Claude Code Session                     Host System                     |
|   ===================                     ===========                     |
|                                                                           |
|   +------------------+                    +-----------------------------+ |
|   | .mcp.json says:  |                    |      MCP Socket Pool        | |
|   |                  |                    |                             | |
|   | "context7": {    |                    |  +------------------------+ | |
|   |   "command":"nc" |   Unix Socket      |  |   SocketProxy          | | |
|   |   "args": [      |   ~~~~~~~~~~~~     |  |                        | | |
|   |     "-U",        |=================>  |  | +--------------------+ | | |
|   |     "/path/to/   |                    |  | | Request Router     | | | |
|   |      socket"     |                    |  | | (ID rewriting)     | | | |
|   |   ]              |                    |  | +--------------------+ | | |
|   | }                |                    |  |          |            | | |
|   +------------------+                    |  |          v            | | |
|                                           |  | +--------------------+ | | |
|   When Claude Code                        |  | | MCP Server Process | | | |
|   starts, it runs:                        |  | | (single instance)  | | | |
|                                           |  | |                    | | | |
|   nc -U ~/.agents-in-a-box/               |  | | stdin <-- JSON --> | | | |
|        sockets/mcp-context7.sock          |  | | stdout             | | | |
|                                           |  | +--------------------+ | | |
|   This opens a persistent                 |  +------------------------+ | |
|   connection to the pool                  +-----------------------------+ |
|                                                                           |
+--------------------------------------------------------------------------+
```

### Session Integration Flow

```
+--------------------------------------------------------------------------+
|                      SESSION CREATION FLOW                                |
+--------------------------------------------------------------------------+
|                                                                           |
|  1. TUI Startup                                                           |
|     +------------------------------------------------------------------+  |
|     | - Load MCPs from config.toml                                     |  |
|     | - Auto-import from Claude configs                                |  |
|     | - Merge and validate all MCPs                                    |  |
|     | - Start socket proxy for each enabled+pooled MCP                 |  |
|     +------------------------------------------------------------------+  |
|                              |                                            |
|                              v                                            |
|  2. Session Creation                                                      |
|     +------------------------------------------------------------------+  |
|     | McpCatalog.write_session_config(project_path, mcps, socket_dir)  |  |
|     +------------------------------------------------------------------+  |
|                              |                                            |
|                              v                                            |
|  3. Generate .mcp.json                                                    |
|     +------------------------------------------------------------------+  |
|     | For each MCP:                                                    |  |
|     |   - If pool=true AND socket exists:                              |  |
|     |       {"command": "nc", "args": ["-U", "/path/to/socket"]}       |  |
|     |   - If pool=false OR socket missing:                             |  |
|     |       {"command": "npx", "args": [...original args...]}          |  |
|     +------------------------------------------------------------------+  |
|                              |                                            |
|                              v                                            |
|  4. Write to Project                                                      |
|     +------------------------------------------------------------------+  |
|     | <project>/.mcp.json                                              |  |
|     | {                                                                |  |
|     |   "mcpServers": {                                                |  |
|     |     "context7": {"command":"nc","args":["-U","/path/..."]},      |  |
|     |     "puppeteer": {"command":"npx","args":["-y","@anthropic/..."]}|  |
|     |   }                                                              |  |
|     | }                                                                |  |
|     +------------------------------------------------------------------+  |
|                              |                                            |
|                              v                                            |
|  5. Claude Code Starts                                                    |
|     +------------------------------------------------------------------+  |
|     | - Reads .mcp.json                                                |  |
|     | - For pooled MCPs: runs `nc -U /path/to/socket`                  |  |
|     | - For non-pooled: spawns MCP process directly                    |  |
|     +------------------------------------------------------------------+  |
|                                                                           |
+--------------------------------------------------------------------------+
```

### Generated .mcp.json Example

When a session is created, `McpCatalog` generates:

```json
{
  "mcpServers": {
    "context7": {
      "command": "nc",
      "args": ["-U", "/Users/you/.agents-in-a-box/sockets/mcp-context7.sock"]
    },
    "memory": {
      "command": "nc",
      "args": ["-U", "/Users/you/.agents-in-a-box/sockets/mcp-memory.sock"]
    },
    "puppeteer": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-puppeteer"]
    }
  }
}
```

Note: `puppeteer` uses direct stdio because `pool = false` in its config.

---

## The Critical Problem: Request ID Collision

MCP uses JSON-RPC, where each request has an `id`. Multiple sessions might use the same ID:

```
Session 1 sends: {"jsonrpc": "2.0", "id": 1, "method": "tools/call", ...}
Session 2 sends: {"jsonrpc": "2.0", "id": 1, "method": "tools/call", ...}  <-- Same ID!

MCP responds:    {"jsonrpc": "2.0", "id": 1, "result": ...}

Which session gets this response? DATA CORRUPTION!
```

### Solution: Request ID Rewriting

The proxy rewrites request IDs to unique UUIDs:

```
+---------------------------------------------------------------------+
|                    REQUEST ID REWRITING                              |
+---------------------------------------------------------------------+
|                                                                      |
|  Session 1                    Proxy                     MCP Server   |
|  ---------                    -----                     ----------   |
|                                                                      |
|  {"id": 1, ...} --------> {"id": "a1b2c3...", ...} -------->         |
|                                    |                                 |
|                           +--------+--------+                        |
|                           | Mapping Table   |                        |
|                           | --------------- |                        |
|                           | a1b2c3 -> Sess1:1|                        |
|                           | d4e5f6 -> Sess2:1|                        |
|                           +--------+--------+                        |
|                                    |                                 |
|  {"id": 1, ...} <-------- {"id": "a1b2c3...", ...} <--------         |
|                                                                      |
|  Session 2                                                           |
|  ---------                                                           |
|                                                                      |
|  {"id": 1, ...} --------> {"id": "d4e5f6...", ...} -------->         |
|                                                                      |
|  {"id": 1, ...} <-------- {"id": "d4e5f6...", ...} <--------         |
|                                                                      |
|  [OK] Each session gets ONLY its own responses                       |
|                                                                      |
+---------------------------------------------------------------------+
```

---

## Component Overview

```
src/mcp_pool/
+-- mod.rs                 # Module exports
+-- config.rs              # PoolConfig - all tunable parameters
+-- request_router.rs      # UUID-based request ID rewriting (CRITICAL)
+-- backpressure.rs        # Circuit breaker + bounded queues
+-- client_manager.rs      # Session lifecycle (connect/disconnect/keepalive)
+-- process_supervisor.rs  # MCP process management with auto-restart
+-- socket_proxy.rs        # Core proxy combining all components
+-- discovery.rs           # Find existing sockets, handle multi-instance
+-- pool.rs                # Manage multiple MCP proxies
+-- health.rs              # Background health monitoring
+-- metrics.rs             # Statistics collection
+-- tcp_relay.rs           # TCP bridge for Docker containers

src/config/
+-- mcp.rs                 # McpDefinition, McpImporter, McpCatalog
+-- mcp_init.rs            # Container MCP initialization strategies
```

### Key Components

| Component | Responsibility |
|-----------|---------------|
| **McpDefinition** | TOML config struct for MCP servers |
| **McpImporter** | Auto-import from Claude Desktop/Code configs |
| **McpCatalog** | Generate session `.mcp.json` with nc -U commands |
| **RequestRouter** | Rewrites `id` fields to prevent collisions |
| **ClientManager** | Tracks connected sessions, handles disconnect |
| **ProcessSupervisor** | Starts MCP process, restarts on crash |
| **CircuitBreaker** | Prevents cascade failures when MCP is unhealthy |
| **SocketProxy** | Combines all above, listens on Unix socket |
| **McpSocketPool** | Manages proxies for multiple MCP servers |

---

## Data Flow

```
+---------------------------------------------------------------------+
|                         REQUEST FLOW                                 |
+---------------------------------------------------------------------+
|                                                                      |
|  1. Claude Code connects to Unix socket                              |
|     ~/.agents-in-a-box/sockets/mcp-context7.sock                     |
|                                                                      |
|  2. Claude Code sends JSON-RPC request                               |
|     {"jsonrpc": "2.0", "id": 42, "method": "tools/list"}             |
|                                                                      |
|  3. RequestRouter rewrites ID                                        |
|     {"jsonrpc": "2.0", "id": "f47ac10b-58cc...", "method": ...}      |
|     Stores mapping: f47ac10b -> (client_id, original_id: 42)         |
|                                                                      |
|  4. Proxy writes to MCP stdin                                        |
|                                                                      |
|  5. MCP responds on stdout                                           |
|     {"jsonrpc": "2.0", "id": "f47ac10b-58cc...", "result": [...]}    |
|                                                                      |
|  6. RequestRouter restores original ID                               |
|     {"jsonrpc": "2.0", "id": 42, "result": [...]}                    |
|                                                                      |
|  7. Proxy sends response to correct client socket                    |
|                                                                      |
+---------------------------------------------------------------------+
```

---

## Failure Handling

### Circuit Breaker

Prevents overloading a failing MCP server:

```
+------------------------------------------------------------------+
|                    CIRCUIT BREAKER STATES                         |
+------------------------------------------------------------------+
|                                                                   |
|     +--------+    3 failures    +--------+                        |
|     | CLOSED | ---------------->|  OPEN  |                        |
|     |        |                  |        |                        |
|     | Normal |                  | Reject |                        |
|     | operation                 | all    |                        |
|     +----+---+                  +---+----+                        |
|          |                          |                             |
|          |    success               | 30 sec timeout              |
|          |                          |                             |
|          |                          v                             |
|          |                    +-----------+                       |
|          |                    | HALF-OPEN |                       |
|          |<-------------------|           |                       |
|               success         | Test one  |                       |
|                               | request   |                       |
|                               +-----------+                       |
|                                     |                             |
|                                     | failure                     |
|                                     v                             |
|                               Back to OPEN                        |
|                                                                   |
+------------------------------------------------------------------+
```

### Process Supervision

Auto-restart with exponential backoff:

```
MCP crashes -> Wait 1s -> Restart -> Crashes again -> Wait 2s -> Restart -> ...
                                                                   |
                                               Max backoff: 60s    |
                                               Max restarts: 10 ---+
```

---

## Pool Configuration

Add to `~/.config/agents-in-a-box/config.toml`:

```toml
[mcp_pool]
# Master switch
enabled = true

# Pool all MCP servers by default
pool_all = true

# Exclude specific MCPs that need per-session isolation
exclude_mcps = ["chrome", "puppeteer"]

# Or explicitly include only specific MCPs
# include_mcps = ["context7", "exa"]

# Timeouts (seconds)
request_timeout = 300      # 5 minutes max per request
keepalive_interval = 30    # Ping clients every 30s
idle_client_timeout = 60   # Disconnect idle clients after 60s

# Limits
max_clients_per_mcp = 50   # Max sessions per MCP server
max_pending_requests_per_client = 100

# Circuit breaker
circuit_breaker_threshold = 3   # Open after 3 failures
circuit_breaker_reset = 30      # Try again after 30s

# Docker support
tcp_relay_enabled = true
tcp_relay_port_range = [19000, 19999]

# Fallback
fallback_to_stdio = true   # Use stdio if socket fails
```

---

## Socket Location

Sockets are stored in a **secure, user-private directory**:

```
~/.agents-in-a-box/sockets/
+-- mcp-context7.sock      # Unix domain socket
+-- mcp-context7.lock      # Lock file with owner PID
+-- mcp-exa.sock
+-- mcp-exa.lock
+-- ...
```

- Directory permissions: `0700` (only owner can access)
- Lock files use `fcntl` advisory locking
- Stale sockets are automatically cleaned up

---

## Docker Container Access

Containers cannot use Unix sockets on the host. The TCP relay bridges this:

```
+---------------------------------------------------------------------+
|                    DOCKER TCP RELAY                                  |
+---------------------------------------------------------------------+
|                                                                      |
|   Docker Container                Host                               |
|   ================               ====                                |
|                                                                      |
|   +--------------+              +--------------+                     |
|   | Claude Code  |              |  TCP Relay   |                     |
|   |              |--TCP:19001-->|              |                     |
|   |              |              | localhost:   |                     |
|   +--------------+              |    19001     |                     |
|                                 +------+-------+                     |
|                                        |                             |
|                                        v                             |
|                                 +--------------+                     |
|                                 | Unix Socket  |                     |
|                                 | mcp-ctx7.sock|                     |
|                                 +------+-------+                     |
|                                        |                             |
|                                        v                             |
|                                 +--------------+                     |
|                                 | MCP Server   |                     |
|                                 |   Process    |                     |
|                                 +--------------+                     |
|                                                                      |
+---------------------------------------------------------------------+
```

---

## Memory Savings Example

| Scenario | Without Pooling | With Pooling | Savings |
|----------|-----------------|--------------|---------|
| 1 session, 5 MCPs | 750 MB | 750 MB | 0% |
| 3 sessions, 5 MCPs | 2,250 MB | 750 MB | **67%** |
| 10 sessions, 5 MCPs | 7,500 MB | 750 MB | **90%** |
| 20 sessions, 5 MCPs | 15,000 MB | 750 MB | **95%** |

*Assumes ~150MB per MCP server process*

---

## Troubleshooting

### Check if pooling is active

```bash
# List active sockets
ls -la ~/.agents-in-a-box/sockets/

# Check socket owner
cat ~/.agents-in-a-box/sockets/mcp-context7.lock
```

### MCP not responding

1. Check circuit breaker state in logs
2. Verify MCP process is running: `ps aux | grep mcp`
3. Check for stale sockets and clean up

### Session not receiving responses

1. Ensure request IDs are unique within your session
2. Check client connection state
3. Review proxy logs for routing errors

### Auto-import not finding MCPs

1. Check that source files exist:
   ```bash
   ls ~/.claude/settings.json
   ls ~/Library/Application\ Support/Claude/claude_desktop_config.json  # macOS
   ls ~/.config/Claude/claude_desktop_config.json  # Linux
   ```
2. Verify JSON structure has `mcpServers` key
3. Check TUI logs for import messages

### Netcat connection issues

1. Verify socket exists: `ls -la ~/.agents-in-a-box/sockets/`
2. Test connection manually: `echo '{"jsonrpc":"2.0","id":1,"method":"ping"}' | nc -U /path/to/socket`
3. Check socket permissions match current user

---

## Implementation Stats

| Metric | Value |
|--------|-------|
| Lines of Code | ~4,500 |
| Unit Tests | 212 |
| Modules | 11 |
| Test Coverage | Core paths covered |
