# Research: MCP Socket Pooling Implementation Plan for ainb-tui

**Date**: 2026-01-12
**Repository**: stevengonsalvez/ainb-tui
**Branch**: feat/mcp-socket-pooling
**Research Type**: Codebase Analysis | Architecture Planning
**Reference**: [agent-deck MCP Socket Pooling Research](./2026-01-12_22-39-30_agent-deck-mcp-socket-pooling.md)

---

## Executive Summary

This document outlines a comprehensive plan to implement MCP socket pooling in ainb-tui, porting the pattern from [agent-deck](https://github.com/asheshgoplani/agent-deck). The goal is to share MCP server processes across multiple Claude Code sessions via Unix domain sockets, achieving **85-90% memory savings** for concurrent sessions.

**Key insight**: ainb-tui already has mature MCP support (`src/config/mcp.rs`, `src/config/mcp_init.rs`). The socket pooling layer will sit **between** the existing MCP configuration and session creation, requiring minimal changes to the current architecture.

---

## Current ainb-tui Architecture

### Session Management - Two Parallel Systems

```
┌─────────────────────────────────────────────────────────────────┐
│                        ainb-tui                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌───────────────────────┐     ┌───────────────────────────┐    │
│  │  InteractiveSession   │     │  SessionLifecycle         │    │
│  │      Manager          │     │      Manager              │    │
│  │  (Host-based)         │     │  (Docker-based)           │    │
│  │                       │     │                           │    │
│  │  • tmux sessions      │     │  • Docker containers      │    │
│  │  • Git worktrees      │     │  • MCP initialization     │    │
│  │  • Direct host exec   │     │  • Volume mounts          │    │
│  └───────────────────────┘     └───────────────────────────┘    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  MCP Subsystem (Existing)                 │   │
│  │  • McpServerConfig - Server definitions                   │   │
│  │  • McpInitializer - Per-container initialization          │   │
│  │  • McpWidget - UI rendering for MCP tool calls            │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Source Files

| File | Purpose | Relevance to Socket Pooling |
|------|---------|----------------------------|
| `src/config/mcp.rs` | MCP server definitions (command, args, env) | **Core** - Source of server configs |
| `src/config/mcp_init.rs` | MCP initialization strategies | **Extend** - Add socket pooling strategy |
| `src/docker/session_lifecycle.rs` | Docker session creation | **Integrate** - Use pool instead of per-container |
| `src/interactive/session_manager.rs` | Host tmux sessions | **Integrate** - Use pool for native sessions |
| `src/tmux/session.rs` | Tmux session management | Reference pattern for process lifecycle |
| `src/widgets/mcp_widget.rs` | MCP tool call rendering | No changes needed |
| `src/app/state.rs` | App initialization & state | **Initialize** - Start pool at app startup |

### Existing MCP Configuration Model

```rust
// From src/config/mcp.rs
pub struct McpServerConfig {
    pub name: String,
    pub description: String,
    pub installation: McpInstallation,
    pub definition: McpServerDefinition,
    pub required_env: Vec<String>,
    pub enabled_by_default: bool,
}

pub enum McpServerDefinition {
    Command {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Json { config: serde_json::Value },
}

pub enum McpInitStrategy {
    PerContainer,                    // Current: spawn in each container
    CentralMount { host_path: String },
    Hybrid { config_path: String, merge_configs: bool },
    // NEW: SocketPool { ... }      // To be added
}
```

---

## Proposed Socket Pooling Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        ainb-tui with MCP Socket Pool                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌───────────────────┐ ┌───────────────────┐ ┌───────────────────────┐  │
│  │  Interactive #1   │ │  Interactive #2   │ │  Docker Session #N    │  │
│  │  tmux session     │ │  tmux session     │ │  container            │  │
│  │                   │ │                   │ │                       │  │
│  │  nc -U socket.sock│ │  nc -U socket.sock│ │  nc -U socket.sock    │  │
│  └─────────┬─────────┘ └─────────┬─────────┘ └───────────┬───────────┘  │
│            │                     │                       │              │
│            └──────────────────┬──┴───────────────────────┘              │
│                               │                                          │
│                               ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                      MCP Socket Pool                               │  │
│  │                                                                    │  │
│  │  ┌──────────────────┐ ┌──────────────────┐ ┌─────────────────────┐│  │
│  │  │  SocketProxy:    │ │  SocketProxy:    │ │  SocketProxy:       ││  │
│  │  │  context7        │ │  memory          │ │  exa                ││  │
│  │  │                  │ │                  │ │                     ││  │
│  │  │  Unix Socket:    │ │  Unix Socket:    │ │  Unix Socket:       ││  │
│  │  │  /tmp/ainb-mcp-  │ │  /tmp/ainb-mcp-  │ │  /tmp/ainb-mcp-     ││  │
│  │  │  context7.sock   │ │  memory.sock     │ │  exa.sock           ││  │
│  │  │                  │ │                  │ │                     ││  │
│  │  │  MCP Process:    │ │  MCP Process:    │ │  MCP Process:       ││  │
│  │  │  npx context7    │ │  npx memory      │ │  npx exa-mcp-server ││  │
│  │  └──────────────────┘ └──────────────────┘ └─────────────────────┘│  │
│  │                                                                    │  │
│  │  Request Map: { requestID → sessionID }                           │  │
│  │  Health Monitor: 10-second checks, rate-limited restarts          │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### JSON-RPC Request Routing

The core multiplexing mechanism (from agent-deck):

```
Session A                  Socket Proxy                MCP Process
    │                           │                           │
    │  {"id": 42, "method":    │                           │
    │   "tools/call", ...}     │                           │
    │ ─────────────────────────>                           │
    │                           │                           │
    │                   Store: 42 → A                      │
    │                           │   {"id": 42, ...}        │
    │                           │ ─────────────────────────>
    │                           │                           │
    │                           │   {"id": 42, result: ...}│
    │                           │ <─────────────────────────
    │                           │                           │
    │                   Lookup: 42 → A                     │
    │                   Delete: 42                          │
    │   {"id": 42, result: ...}│                           │
    │ <─────────────────────────                           │
    │                           │                           │
```

**For notifications (no `id`)**: Broadcast to ALL connected sessions.

---

## Proposed Module Structure

```
src/mcp_pool/
├── mod.rs              # Module exports, public API
├── pool.rs             # McpSocketPool - manages multiple proxies
├── socket_proxy.rs     # SocketProxy - wraps single MCP process
├── request_router.rs   # JSON-RPC ID tracking & routing
├── health.rs           # Health monitoring & restart logic
├── config.rs           # PoolConfig, socket paths, timeouts
└── discovery.rs        # Discover existing sockets from other instances
```

### Core Data Structures

```rust
// src/mcp_pool/pool.rs
pub struct McpSocketPool {
    proxies: HashMap<String, SocketProxy>,  // MCP name → proxy
    config: PoolConfig,
    health_task: Option<JoinHandle<()>>,
}

impl McpSocketPool {
    pub async fn new(config: PoolConfig) -> Result<Self>;
    pub async fn start(&mut self, mcp_configs: &[McpServerConfig]) -> Result<()>;
    pub async fn get_socket_path(&self, name: &str) -> Option<PathBuf>;
    pub async fn is_running(&self, name: &str) -> bool;
    pub async fn shutdown(&mut self) -> Result<()>;
}

// src/mcp_pool/socket_proxy.rs
pub struct SocketProxy {
    name: String,
    mcp_process: Child,
    mcp_stdin: ChildStdin,
    mcp_stdout: ChildStdout,
    socket_path: PathBuf,
    listener: UnixListener,
    clients: HashMap<String, UnixStream>,       // session_id → stream
    request_map: HashMap<RequestId, String>,    // request_id → session_id
    status: ServerStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Failed(String),
}

// src/mcp_pool/request_router.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl RequestId {
    pub fn from_json(value: &serde_json::Value) -> Option<Self>;
}

// src/mcp_pool/config.rs
pub struct PoolConfig {
    pub enabled: bool,
    pub pool_all: bool,                     // Pool ALL MCPs by default
    pub exclude_mcps: Vec<String>,          // Exclude specific MCPs
    pub include_mcps: Vec<String>,          // Only pool these MCPs
    pub socket_dir: PathBuf,                // Default: /tmp
    pub socket_prefix: String,              // Default: "ainb-mcp-"
    pub socket_wait_timeout: Duration,      // Default: 5s
    pub health_check_interval: Duration,    // Default: 10s
    pub max_restarts_per_minute: u32,       // Default: 3
    pub min_restart_interval: Duration,     // Default: 5s
    pub fallback_to_stdio: bool,            // Default: true
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pool_all: true,
            exclude_mcps: vec![],
            include_mcps: vec![],
            socket_dir: PathBuf::from("/tmp"),
            socket_prefix: "ainb-mcp-".to_string(),
            socket_wait_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(10),
            max_restarts_per_minute: 3,
            min_restart_interval: Duration::from_secs(5),
            fallback_to_stdio: true,
        }
    }
}
```

---

## Integration Points

### 1. App Initialization (`src/app/state.rs`)

```rust
// Add to App struct
pub struct App {
    // ... existing fields ...
    mcp_pool: Option<Arc<tokio::sync::RwLock<McpSocketPool>>>,
}

impl App {
    pub async fn new() -> Result<Self> {
        // ... existing initialization ...

        // Initialize MCP socket pool
        let pool_config = PoolConfig::from_app_config(&app_config);
        let mcp_pool = if pool_config.enabled && cfg!(unix) {
            let mut pool = McpSocketPool::new(pool_config)?;
            pool.start(&app_config.get_mcp_servers()).await?;
            Some(Arc::new(tokio::sync::RwLock::new(pool)))
        } else {
            None
        };

        Ok(Self {
            // ... existing fields ...
            mcp_pool,
        })
    }
}
```

### 2. MCP Init Strategy Extension (`src/config/mcp_init.rs`)

```rust
pub enum McpInitStrategy {
    PerContainer,
    CentralMount { host_path: String },
    Hybrid { config_path: String, merge_configs: bool },

    // NEW: Use socket pooling
    SocketPool {
        pool: Arc<tokio::sync::RwLock<McpSocketPool>>,
        fallback: Box<McpInitStrategy>,  // Fallback for excluded MCPs
    },
}

impl McpInitializer {
    async fn init_socket_pool(
        &self,
        pool: &McpSocketPool,
        fallback: &McpInitStrategy,
        container_config: &mut ContainerConfig,
    ) -> Result<McpInitResult> {
        let mut mcp_servers = serde_json::Map::new();

        for server in &self.servers {
            if pool.is_running(&server.name).await {
                // Use netcat bridge to socket
                let socket_path = pool.get_socket_path(&server.name).await.unwrap();
                mcp_servers.insert(server.name.clone(), json!({
                    "command": "nc",
                    "args": ["-U", socket_path.to_str().unwrap()]
                }));
            } else if self.should_pool(&server.name) {
                // Pool should be running but isn't - log warning
                tracing::warn!("MCP {} should be pooled but pool not running", server.name);
                // Use fallback
                mcp_servers.insert(server.name.clone(), server.to_mcp_config());
            } else {
                // Explicitly excluded from pool
                mcp_servers.insert(server.name.clone(), server.to_mcp_config());
            }
        }

        Ok(McpInitResult {
            config: json!({ "mcpServers": mcp_servers }),
            volumes: vec![],
            environment: HashMap::new(),
            post_scripts: vec![],
        })
    }
}
```

### 3. Docker Session Lifecycle (`src/docker/session_lifecycle.rs`)

```rust
impl SessionLifecycleManager {
    async fn initialize_mcp_servers(
        &self,
        config: &mut ContainerConfig,
        request: &SessionRequest,
        _project_config: &Option<ProjectConfig>,
        progress_sender: &Option<mpsc::Sender<SessionProgress>>,
    ) -> Result<McpInitResult, SessionLifecycleError> {
        // Check if socket pool is available
        if let Some(ref pool) = self.mcp_pool {
            let pool_guard = pool.read().await;
            let strategy = McpInitStrategy::SocketPool {
                pool: Arc::clone(pool),
                fallback: Box::new(McpInitStrategy::PerContainer),
            };
            // ... use socket pool strategy
        } else {
            // ... existing per-container logic
        }
    }
}
```

### 4. Interactive Session Manager (`src/interactive/session_manager.rs`)

```rust
impl InteractiveSessionManager {
    async fn configure_mcp_for_tmux(
        &self,
        session_name: &str,
        worktree_path: &Path,
    ) -> Result<()> {
        // Write .mcp.json to worktree with socket-based config
        if let Some(ref pool) = self.mcp_pool {
            let pool_guard = pool.read().await;
            let mcp_config = self.generate_socket_mcp_config(&pool_guard).await?;

            let mcp_path = worktree_path.join(".mcp.json");
            std::fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_config)?)?;

            tracing::info!(
                "Wrote socket-based .mcp.json for session {}",
                session_name
            );
        }

        Ok(())
    }
}
```

### 5. Configuration File (`~/.agents-in-a-box/config.toml`)

```toml
[mcp_pool]
enabled = true                    # Master switch for socket pooling
pool_all = true                   # Pool ALL defined MCPs
exclude_mcps = ["chrome"]         # MCPs that need per-session isolation
socket_wait_timeout = 5           # Seconds to wait for socket ready
health_check_interval = 10        # Seconds between health checks
fallback_to_stdio = true          # Fall back if socket fails

[mcps.context7]
command = "npx"
args = ["-y", "context7"]
description = "Library documentation and code examples"
pool = true                       # Explicitly enable pooling

[mcps.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]
description = "Persistent memory"
pool = true

[mcps.exa]
command = "npx"
args = ["-y", "exa-mcp-server"]
env = { EXA_API_KEY = "${EXA_API_KEY}" }
description = "Web search via Exa AI"
pool = true
```

---

## Implementation Phases

### Phase 1: Core Socket Proxy (2-3 days)

1. Create `src/mcp_pool/mod.rs` - Module structure
2. Implement `SocketProxy` - Single MCP process wrapper
3. Implement `RequestRouter` - JSON-RPC ID tracking
4. Add unit tests for request routing

**Deliverables**:
- Basic socket proxy that can multiplex requests
- Works with a single MCP server
- Unit tests pass

### Phase 2: Pool Management (2-3 days)

1. Implement `McpSocketPool` - Multiple proxy management
2. Add health monitoring with tokio background task
3. Implement rate-limited restart logic
4. Add socket discovery for multi-instance coordination

**Deliverables**:
- Pool manages multiple MCP servers
- Health checks with auto-restart
- Other ainb-tui instances can discover/reuse sockets

### Phase 3: Integration (2-3 days)

1. Extend `McpInitStrategy` with `SocketPool` variant
2. Integrate pool into `SessionLifecycleManager`
3. Integrate pool into `InteractiveSessionManager`
4. Add pool initialization to `App::new()`
5. Update configuration parsing

**Deliverables**:
- Sessions use pooled MCP servers
- Fallback to stdio when pool unavailable
- Configuration via `config.toml`

### Phase 4: Platform Support & Polish (1-2 days)

1. Add `#[cfg(unix)]` guards for socket code
2. Implement Windows/WSL1 detection and auto-disable
3. Add graceful shutdown on app exit
4. Add telemetry/logging for pool operations

**Deliverables**:
- Works on macOS and Linux
- Graceful degradation on Windows
- Comprehensive logging

### Phase 5: Testing & Documentation (1-2 days)

1. Integration tests with real MCP servers
2. Load testing with multiple concurrent sessions
3. Update CLAUDE.md with pool configuration
4. Add troubleshooting guide

**Deliverables**:
- Tested with 10+ concurrent sessions
- Documentation complete
- Performance benchmarks

---

## Platform Considerations

| Platform | Socket Pooling | Notes |
|----------|----------------|-------|
| macOS | Full support | Primary development platform |
| Linux | Full support | Native Unix sockets |
| WSL2 | Full support | Unix sockets work |
| WSL1 | Auto-disabled | Falls back to stdio |
| Windows | Auto-disabled | Falls back to stdio |

### Platform Detection

```rust
pub fn is_socket_pooling_supported() -> bool {
    #[cfg(not(unix))]
    return false;

    #[cfg(unix)]
    {
        // Check for WSL1 (doesn't support Unix sockets well)
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            if version.contains("Microsoft") && !version.contains("WSL2") {
                return false;
            }
        }
        true
    }
}
```

---

## Benefits & Trade-offs

### Benefits

| Benefit | Impact | Explanation |
|---------|--------|-------------|
| Memory savings | **85-90%** | Single MCP process instead of N per session |
| Faster startup | **2-3s** | MCP already running, no spawn delay |
| Shared state | **Useful** | Memory MCP shares context across sessions |
| Resource efficiency | **High** | Fewer node/npx processes |

### Trade-offs

| Trade-off | Mitigation |
|-----------|------------|
| Single point of failure | Health monitoring + auto-restart |
| Shared state (might be unwanted) | Per-MCP `pool=false` config |
| Additional complexity | Clean module separation |
| Unix-only | Graceful fallback to stdio |

---

## Open Questions

1. **Session isolation**: Should memory MCP be shared, or per-session?
   - *Recommendation*: Make configurable. Default to shared for efficiency.

2. **Socket permissions**: What file permissions for `/tmp/ainb-mcp-*.sock`?
   - *Recommendation*: `0600` (owner read/write only)

3. **Cleanup on crash**: How to handle stale sockets?
   - *Recommendation*: Check socket liveness before reuse; delete stale sockets

4. **Docker integration**: Should Docker containers connect to host sockets?
   - *Recommendation*: Mount socket directory into container for seamless access

---

## Dependencies

### New Crate Dependencies

```toml
[dependencies]
# Already present, no changes needed:
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

# Potentially useful but likely not needed (tokio already provides):
# nix = "0.29"  # Unix-specific APIs (optional)
```

### No New Dependencies Required

The implementation uses:
- `std::os::unix::net::{UnixListener, UnixStream}` - stdlib
- `tokio::process::Command` - already in use
- `tokio::sync::mpsc` - already in use
- `serde_json` - already in use

---

## References

- [agent-deck MCP Socket Pooling Research](./2026-01-12_22-39-30_agent-deck-mcp-socket-pooling.md)
- [GitHub - asheshgoplani/agent-deck](https://github.com/asheshgoplani/agent-deck)
- [MCP Transports Specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [Rust std::os::unix::net](https://doc.rust-lang.org/std/os/unix/net/index.html)

---

## Next Steps

1. Create GitHub issue for MCP socket pooling feature
2. Begin Phase 1 implementation: Core Socket Proxy
3. Set up integration tests with context7 MCP server
4. Document configuration options in CLAUDE.md
