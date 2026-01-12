# Research: MCP Socket Pooling Implementation Plan for ainb-tui

**Date**: 2026-01-12 (Updated after Distinguished Engineer Review)
**Repository**: stevengonsalvez/ainb-tui
**Branch**: feat/mcp-socket-pooling
**Research Type**: Codebase Analysis | Architecture Planning
**Reference**: [agent-deck MCP Socket Pooling Research](./2026-01-12_22-39-30_agent-deck-mcp-socket-pooling.md)
**Review**: Distinguished Engineer critique applied - see [Critical Fixes](#critical-fixes-from-review) section

---

## Executive Summary

This document outlines a comprehensive plan to implement MCP socket pooling in ainb-tui, porting the pattern from [agent-deck](https://github.com/asheshgoplani/agent-deck). The goal is to share MCP server processes across multiple Claude Code sessions via Unix domain sockets, achieving **85-90% memory savings** for concurrent sessions.

**Key insight**: ainb-tui already has mature MCP support (`src/config/mcp.rs`, `src/config/mcp_init.rs`). The socket pooling layer will sit **between** the existing MCP configuration and session creation, requiring minimal changes to the current architecture.

---

## Critical Fixes from Review

The following critical issues were identified by Distinguished Engineer review and are **MANDATORY** for production readiness:

| Issue | Severity | Root Cause | Solution |
|-------|----------|------------|----------|
| Request ID Collision | **CRITICAL** | JSON-RPC IDs are session-local, not globally unique | Request ID rewriting with proxy-level UUIDs |
| No Backpressure | **HIGH** | Unbounded queues cause OOM under load | Bounded channels + circuit breaker |
| FD Leak on Disconnect | **HIGH** | No client liveness detection | Keepalive + timeout + background reaper |
| Insecure Socket Location | **HIGH** | `/tmp` is world-readable | Use `~/.agents-in-a-box/sockets/` |
| Socket Discovery Race | **MEDIUM** | TOCTOU between check and use | Lock files with PID validation |
| Zombie Processes | **MEDIUM** | No SIGCHLD handling | Proper tokio process supervision |
| Notification Storm | **MEDIUM** | Broadcast to all without backpressure | Per-client queues with coalescing |
| Docker Socket Mounting | **MEDIUM** | Breaks container isolation | TCP relay for containers |

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
┌──────────────────────────────────────────────────────────────────────────────┐
│                        ainb-tui with MCP Socket Pool                          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  ┌───────────────────┐ ┌───────────────────┐ ┌─────────────────────────────┐ │
│  │  Interactive #1   │ │  Interactive #2   │ │  Docker Session #N          │ │
│  │  tmux session     │ │  tmux session     │ │  container                  │ │
│  │                   │ │                   │ │                             │ │
│  │  nc -U socket.sock│ │  nc -U socket.sock│ │  socat TCP:host:port STDIO  │ │
│  └─────────┬─────────┘ └─────────┬─────────┘ └─────────────┬───────────────┘ │
│            │                     │                         │                 │
│            └──────────────────┬──┴─────────────────────────┘                 │
│                               │                                               │
│                               ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                      MCP Socket Pool                                     │ │
│  │  Socket Dir: ~/.agents-in-a-box/sockets/ (mode 0700)                    │ │
│  │                                                                          │ │
│  │  ┌────────────────────┐ ┌────────────────────┐ ┌──────────────────────┐ │ │
│  │  │  SocketProxy:      │ │  SocketProxy:      │ │  SocketProxy:        │ │ │
│  │  │  context7          │ │  memory            │ │  exa                 │ │ │
│  │  │                    │ │                    │ │                      │ │ │
│  │  │  ┌──────────────┐  │ │  ┌──────────────┐  │ │  ┌──────────────┐   │ │ │
│  │  │  │ ID Rewriter  │  │ │  │ ID Rewriter  │  │ │  │ ID Rewriter  │   │ │ │
│  │  │  │ UUID → orig  │  │ │  │ UUID → orig  │  │ │  │ UUID → orig  │   │ │ │
│  │  │  └──────────────┘  │ │  └──────────────┘  │ │  └──────────────┘   │ │ │
│  │  │                    │ │                    │ │                      │ │ │
│  │  │  ┌──────────────┐  │ │  ┌──────────────┐  │ │  ┌──────────────┐   │ │ │
│  │  │  │ Backpressure │  │ │  │ Backpressure │  │ │  │ Backpressure │   │ │ │
│  │  │  │ 100 req/client│ │ │  │ 100 req/client│ │ │  │ 100 req/client│  │ │ │
│  │  │  └──────────────┘  │ │  └──────────────┘  │ │  └──────────────┘   │ │ │
│  │  │                    │ │                    │ │                      │ │ │
│  │  │  MCP Process:      │ │  MCP Process:      │ │  MCP Process:        │ │ │
│  │  │  npx context7      │ │  npx memory        │ │  npx exa-mcp-server  │ │ │
│  │  └────────────────────┘ └────────────────────┘ └──────────────────────┘ │ │
│  │                                                                          │ │
│  │  ┌─────────────────────────────────────────────────────────────────────┐│ │
│  │  │ Subsystems:                                                          ││ │
│  │  │ • Client Liveness Monitor (keepalive every 30s, reap after 60s)     ││ │
│  │  │ • Health Monitor (check every 10s, circuit breaker on 3 failures)   ││ │
│  │  │ • Process Supervisor (tokio::process with proper SIGCHLD handling)  ││ │
│  │  │ • Metrics Collector (requests/sec, latency, errors)                 ││ │
│  │  └─────────────────────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Request ID Rewriting (CRITICAL)

The **agent-deck pattern is BROKEN** - it stores `request_id → session_id` directly. This fails when two sessions send `{"id": 1, ...}` simultaneously.

**Correct pattern - Proxy-level ID rewriting:**

```
Session A                  Socket Proxy                    MCP Process
    │                           │                               │
    │  {"id": 1, "method":     │                               │
    │   "tools/call", ...}     │                               │
    │ ─────────────────────────>                               │
    │                           │                               │
    │              Generate proxyId: "uuid-abc-123"            │
    │              Store: "uuid-abc-123" → (A, 1)              │
    │              Rewrite request ID                          │
    │                           │                               │
    │                           │  {"id": "uuid-abc-123", ...} │
    │                           │ ─────────────────────────────>
    │                           │                               │
    │                           │  {"id": "uuid-abc-123",      │
    │                           │   "result": ...}             │
    │                           │ <─────────────────────────────
    │                           │                               │
    │              Lookup: "uuid-abc-123" → (A, 1)             │
    │              Restore original ID                         │
    │              Delete mapping                              │
    │                           │                               │
    │   {"id": 1, "result": ...}│                               │
    │ <─────────────────────────                               │
```

**Data structures for ID rewriting:**

```rust
/// Globally unique proxy-level request ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyRequestId(Uuid);

impl ProxyRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Original request ID from client (can collide across sessions)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OriginalRequestId {
    Number(i64),
    String(String),
    Null,  // Some clients use null ID
}

/// Mapping entry for request tracking
pub struct RequestMapping {
    pub session_id: String,
    pub original_id: OriginalRequestId,
    pub created_at: Instant,
    pub timeout: Duration,
}

/// Thread-safe request router with TTL cleanup
pub struct RequestRouter {
    mappings: Arc<RwLock<HashMap<ProxyRequestId, RequestMapping>>>,
    cleanup_task: JoinHandle<()>,
}

impl RequestRouter {
    /// Rewrite incoming request, return proxy ID for tracking
    pub fn rewrite_request(
        &self,
        session_id: &str,
        request: &mut serde_json::Value,
    ) -> Option<ProxyRequestId> {
        let original_id = request.get("id").cloned();
        if original_id.is_none() {
            return None; // Notification, no ID to track
        }

        let proxy_id = ProxyRequestId::new();

        // Rewrite the ID in the request
        request["id"] = json!(proxy_id.0.to_string());

        // Store the mapping
        let mapping = RequestMapping {
            session_id: session_id.to_string(),
            original_id: OriginalRequestId::from_json(&original_id.unwrap()),
            created_at: Instant::now(),
            timeout: Duration::from_secs(300), // 5 min timeout
        };

        self.mappings.write().unwrap().insert(proxy_id.clone(), mapping);

        Some(proxy_id)
    }

    /// Restore original request ID in response, return target session
    pub fn restore_response(
        &self,
        response: &mut serde_json::Value,
    ) -> Option<(String, OriginalRequestId)> {
        let proxy_id_str = response.get("id")?.as_str()?;
        let proxy_id = ProxyRequestId(Uuid::parse_str(proxy_id_str).ok()?);

        let mapping = self.mappings.write().unwrap().remove(&proxy_id)?;

        // Restore original ID
        response["id"] = mapping.original_id.to_json();

        Some((mapping.session_id, mapping.original_id))
    }
}
```

---

## Proposed Module Structure

```
src/mcp_pool/
├── mod.rs              # Module exports, public API
├── pool.rs             # McpSocketPool - manages multiple proxies
├── socket_proxy.rs     # SocketProxy - wraps single MCP process
├── request_router.rs   # Request ID rewriting & routing (CRITICAL)
├── backpressure.rs     # Bounded channels, circuit breaker
├── client_manager.rs   # Client liveness detection, keepalive
├── process_supervisor.rs  # tokio::process with proper reaping
├── health.rs           # Health monitoring & restart logic
├── config.rs           # PoolConfig, socket paths, timeouts
├── discovery.rs        # Socket discovery with lock files
├── metrics.rs          # Observability: counters, latency histograms
└── tcp_relay.rs        # TCP relay for Docker container access
```

### Core Data Structures

```rust
// src/mcp_pool/pool.rs
pub struct McpSocketPool {
    proxies: HashMap<String, Arc<RwLock<SocketProxy>>>,
    config: PoolConfig,
    health_task: Option<JoinHandle<()>>,
    client_reaper_task: Option<JoinHandle<()>>,
    metrics: Arc<PoolMetrics>,
    shutdown_tx: broadcast::Sender<()>,
}

impl McpSocketPool {
    pub async fn new(config: PoolConfig) -> Result<Self>;
    pub async fn start(&mut self, mcp_configs: &[McpServerConfig]) -> Result<()>;
    pub async fn get_socket_path(&self, name: &str) -> Option<PathBuf>;
    pub async fn is_running(&self, name: &str) -> bool;

    /// Graceful shutdown: drain in-flight requests, then stop
    pub async fn shutdown(&mut self, timeout: Duration) -> Result<()>;
}

// src/mcp_pool/socket_proxy.rs
pub struct SocketProxy {
    name: String,
    config: McpServerConfig,

    // Process management (with proper reaping)
    process_supervisor: ProcessSupervisor,

    // Socket
    socket_path: PathBuf,
    lock_path: PathBuf,  // ~/.agents-in-a-box/sockets/{name}.lock
    listener: UnixListener,

    // Client management with liveness detection
    client_manager: ClientManager,

    // Request routing with ID rewriting
    request_router: RequestRouter,

    // Backpressure
    circuit_breaker: CircuitBreaker,

    // Status
    status: Arc<RwLock<ServerStatus>>,
    metrics: Arc<ProxyMetrics>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Failed { reason: String, consecutive_failures: u32 },
    CircuitOpen { until: Instant },  // Circuit breaker tripped
    ShuttingDown,
}

// src/mcp_pool/client_manager.rs
pub struct ClientManager {
    clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
    keepalive_interval: Duration,  // 30s
    idle_timeout: Duration,        // 60s
}

pub struct ClientConnection {
    stream: UnixStream,
    last_activity: Instant,
    pending_requests: BoundedChannel<PendingRequest>,  // Backpressure!
    notification_queue: BoundedChannel<Value>,         // Coalescing queue
}

impl ClientManager {
    /// Background task: send keepalive, reap dead clients
    pub async fn run_liveness_monitor(&self, shutdown: broadcast::Receiver<()>);

    /// Handle EPIPE/BrokenPipe by removing client
    pub fn handle_write_error(&self, session_id: &str, error: &io::Error);
}

// src/mcp_pool/backpressure.rs
pub struct BoundedChannel<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,
    capacity: usize,
}

impl<T> BoundedChannel<T> {
    pub fn new(capacity: usize) -> Self;

    /// Try to send, return error if channel full (backpressure)
    pub fn try_send(&self, item: T) -> Result<(), BackpressureError>;
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,      // 3 failures
    reset_timeout: Duration,     // 30s
    half_open_requests: u32,     // 1 test request when half-open
}

#[derive(Debug, Clone)]
pub enum CircuitState {
    Closed,                              // Normal operation
    Open { until: Instant },             // Rejecting requests
    HalfOpen { test_requests: u32 },     // Testing recovery
}

// src/mcp_pool/process_supervisor.rs
pub struct ProcessSupervisor {
    child: Option<Child>,  // tokio::process::Child
    restart_count: u32,
    last_restart: Option<Instant>,
    backoff: ExponentialBackoff,
    max_restarts: u32,  // After this, mark as permanently failed
}

impl ProcessSupervisor {
    /// Spawn MCP process, handle SIGCHLD properly
    pub async fn spawn(&mut self, config: &McpServerConfig) -> Result<()>;

    /// Check if process is alive, reap zombie if dead
    pub async fn check_and_reap(&mut self) -> ProcessState;

    /// Restart with exponential backoff
    pub async fn restart(&mut self, config: &McpServerConfig) -> Result<()>;
}

#[derive(Debug)]
pub enum ProcessState {
    Running,
    Exited { code: i32 },
    Signaled { signal: i32 },
    Zombie,  // Needs reaping
}

// src/mcp_pool/config.rs
pub struct PoolConfig {
    pub enabled: bool,
    pub pool_all: bool,
    pub exclude_mcps: Vec<String>,
    pub include_mcps: Vec<String>,

    // Socket location - SECURE: user-private directory
    pub socket_dir: PathBuf,        // ~/.agents-in-a-box/sockets/
    pub socket_prefix: String,      // "mcp-"

    // Timeouts
    pub socket_wait_timeout: Duration,      // 5s
    pub request_timeout: Duration,          // 300s (5 min)
    pub keepalive_interval: Duration,       // 30s
    pub idle_client_timeout: Duration,      // 60s

    // Health & restart
    pub health_check_interval: Duration,    // 10s
    pub max_restarts: u32,                  // 10 total
    pub restart_backoff_base: Duration,     // 1s
    pub restart_backoff_max: Duration,      // 60s

    // Backpressure
    pub max_pending_requests_per_client: usize,  // 100
    pub max_clients_per_mcp: usize,              // 50
    pub circuit_breaker_threshold: u32,          // 3 failures
    pub circuit_breaker_reset: Duration,         // 30s

    // Docker integration
    pub tcp_relay_enabled: bool,            // For container access
    pub tcp_relay_port_range: (u16, u16),   // 19000-19999

    pub fallback_to_stdio: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            enabled: true,
            pool_all: true,
            exclude_mcps: vec![],
            include_mcps: vec![],

            // SECURE: User-private directory
            socket_dir: home.join(".agents-in-a-box/sockets"),
            socket_prefix: "mcp-".to_string(),

            socket_wait_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(300),
            keepalive_interval: Duration::from_secs(30),
            idle_client_timeout: Duration::from_secs(60),

            health_check_interval: Duration::from_secs(10),
            max_restarts: 10,
            restart_backoff_base: Duration::from_secs(1),
            restart_backoff_max: Duration::from_secs(60),

            max_pending_requests_per_client: 100,
            max_clients_per_mcp: 50,
            circuit_breaker_threshold: 3,
            circuit_breaker_reset: Duration::from_secs(30),

            tcp_relay_enabled: true,
            tcp_relay_port_range: (19000, 19999),

            fallback_to_stdio: true,
        }
    }
}

// src/mcp_pool/discovery.rs
pub struct SocketDiscovery {
    socket_dir: PathBuf,
}

impl SocketDiscovery {
    /// Discover existing sockets with proper TOCTOU protection
    pub async fn discover(&self) -> Vec<DiscoveredSocket>;

    /// Check if socket is alive using lock file PID validation
    fn is_socket_alive(&self, socket_path: &Path) -> bool {
        let lock_path = socket_path.with_extension("lock");

        // Read PID from lock file
        let pid = match std::fs::read_to_string(&lock_path) {
            Ok(content) => content.trim().parse::<i32>().ok(),
            Err(_) => return false,
        };

        // Check if PID is alive
        if let Some(pid) = pid {
            // On Unix, kill(pid, 0) checks if process exists
            unsafe {
                libc::kill(pid, 0) == 0
            }
        } else {
            false
        }
    }

    /// Clean up stale sockets where we hold the lock
    pub fn cleanup_stale(&self) -> Result<Vec<PathBuf>>;
}

pub struct DiscoveredSocket {
    pub name: String,
    pub socket_path: PathBuf,
    pub owner_pid: i32,
    pub is_ours: bool,  // We created it vs another instance
}

// src/mcp_pool/tcp_relay.rs
/// TCP relay for Docker container access (avoids socket mounting)
pub struct TcpRelay {
    port: u16,
    socket_path: PathBuf,
    listener: TcpListener,
    shutdown: broadcast::Receiver<()>,
}

impl TcpRelay {
    /// Start TCP relay that forwards to Unix socket
    pub async fn start(socket_path: PathBuf, port_range: (u16, u16)) -> Result<Self>;

    /// Run the relay loop
    pub async fn run(&self);
}

// src/mcp_pool/metrics.rs
pub struct PoolMetrics {
    pub total_requests: AtomicU64,
    pub total_errors: AtomicU64,
    pub active_clients: AtomicU32,
    pub active_mcps: AtomicU32,
}

pub struct ProxyMetrics {
    pub requests_in_flight: AtomicU32,
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
    pub latency_histogram: Histogram,  // Consider using metrics crate
    pub circuit_breaker_trips: AtomicU64,
}
```

---

## Security Model

### Socket Location & Permissions

**NEVER use `/tmp`** - it's world-readable. Use user-private directory:

```rust
// Socket directory setup
fn ensure_socket_dir(config: &PoolConfig) -> Result<PathBuf> {
    let socket_dir = &config.socket_dir;  // ~/.agents-in-a-box/sockets/

    // Create with restrictive permissions
    std::fs::create_dir_all(socket_dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);  // rwx------
        std::fs::set_permissions(socket_dir, perms)?;
    }

    Ok(socket_dir.clone())
}
```

### Lock File Protocol

Prevent TOCTOU races and enable stale socket cleanup:

```rust
struct LockFile {
    path: PathBuf,
    file: File,
}

impl LockFile {
    fn acquire(socket_name: &str, socket_dir: &Path) -> Result<Self> {
        let lock_path = socket_dir.join(format!("{}.lock", socket_name));

        // Open with exclusive lock
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)?;

        // Try to acquire exclusive lock (non-blocking)
        if !file.try_lock_exclusive()? {
            return Err(Error::SocketInUse);
        }

        // Write our PID
        writeln!(&file, "{}", std::process::id())?;
        file.sync_all()?;

        Ok(Self { path: lock_path, file })
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // Release lock and remove file
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}
```

---

## Notification Handling

**Problem**: Broadcasting to all clients without backpressure causes storms.

**Solution**: Per-client notification queue with coalescing and backpressure.

```rust
impl ClientConnection {
    /// Queue notification for this client
    pub fn queue_notification(&mut self, notification: Value) -> Result<(), BackpressureError> {
        // Coalesce: if same notification type already queued, replace it
        let method = notification.get("method").and_then(|m| m.as_str());

        if let Some(method) = method {
            self.notification_queue.coalesce_by_key(method, notification)
        } else {
            self.notification_queue.try_send(notification)
        }
    }
}

impl NotificationQueue {
    /// Coalesce notifications by key (keep latest)
    pub fn coalesce_by_key(&mut self, key: &str, value: Value) -> Result<(), BackpressureError> {
        // Remove existing notification with same key
        self.queue.retain(|n| {
            n.get("method").and_then(|m| m.as_str()) != Some(key)
        });

        // Add new notification (with backpressure check)
        if self.queue.len() >= self.capacity {
            return Err(BackpressureError::QueueFull);
        }

        self.queue.push_back(value);
        Ok(())
    }
}
```

---

## Docker Integration (Secure)

**DO NOT mount Unix sockets into containers** - breaks isolation.

Instead, use TCP relay:

```rust
impl SocketProxy {
    /// Start TCP relay for container access
    pub async fn start_tcp_relay(&self) -> Result<u16> {
        let relay = TcpRelay::start(
            self.socket_path.clone(),
            self.config.tcp_relay_port_range,
        ).await?;

        let port = relay.port;

        // Spawn relay task
        tokio::spawn(async move {
            relay.run().await;
        });

        Ok(port)
    }
}

// In Docker session creation:
impl SessionLifecycleManager {
    async fn configure_mcp_for_container(
        &self,
        pool: &McpSocketPool,
    ) -> Result<serde_json::Value> {
        let mut mcp_servers = serde_json::Map::new();

        for (name, proxy) in &pool.proxies {
            // Get TCP relay port instead of socket path
            let port = proxy.read().await.get_tcp_relay_port().await?;

            mcp_servers.insert(name.clone(), json!({
                "command": "socat",
                "args": [
                    &format!("TCP:host.docker.internal:{}", port),
                    "STDIO"
                ]
            }));
        }

        Ok(json!({ "mcpServers": mcp_servers }))
    }
}
```

---

## Graceful Shutdown

```rust
impl McpSocketPool {
    pub async fn shutdown(&mut self, timeout: Duration) -> Result<()> {
        tracing::info!("Starting graceful shutdown, timeout: {:?}", timeout);

        // Signal all tasks to stop
        let _ = self.shutdown_tx.send(());

        // Wait for in-flight requests to complete
        let deadline = Instant::now() + timeout;

        for (name, proxy) in &self.proxies {
            let proxy = proxy.read().await;

            while proxy.request_router.has_pending_requests() {
                if Instant::now() > deadline {
                    tracing::warn!(
                        "Shutdown timeout, {} pending requests dropped for {}",
                        proxy.request_router.pending_count(),
                        name
                    );
                    break;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Stop all proxies
        for (name, proxy) in self.proxies.drain() {
            let mut proxy = proxy.write().await;
            if let Err(e) = proxy.stop().await {
                tracing::error!("Failed to stop proxy {}: {}", name, e);
            }
        }

        // Cancel background tasks
        if let Some(task) = self.health_task.take() {
            task.abort();
        }
        if let Some(task) = self.client_reaper_task.take() {
            task.abort();
        }

        tracing::info!("Graceful shutdown complete");
        Ok(())
    }
}
```

---

## Integration Points

### 1. App Initialization (`src/app/state.rs`)

```rust
pub struct App {
    // ... existing fields ...
    mcp_pool: Option<Arc<RwLock<McpSocketPool>>>,
}

impl App {
    pub async fn new() -> Result<Self> {
        // ... existing initialization ...

        // Initialize MCP socket pool
        let pool_config = PoolConfig::from_app_config(&app_config);
        let mcp_pool = if pool_config.enabled && cfg!(unix) {
            match McpSocketPool::new(pool_config).await {
                Ok(mut pool) => {
                    if let Err(e) = pool.start(&app_config.get_mcp_servers()).await {
                        tracing::error!("Failed to start MCP pool: {}", e);
                        None
                    } else {
                        Some(Arc::new(RwLock::new(pool)))
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create MCP pool: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            // ... existing fields ...
            mcp_pool,
        })
    }

    /// Called on app exit
    pub async fn cleanup(&mut self) {
        if let Some(pool) = self.mcp_pool.take() {
            let mut pool = pool.write().await;
            if let Err(e) = pool.shutdown(Duration::from_secs(5)).await {
                tracing::error!("Pool shutdown error: {}", e);
            }
        }
    }
}
```

### 2. Configuration File (`~/.agents-in-a-box/config.toml`)

```toml
[mcp_pool]
enabled = true                         # Master switch for socket pooling
pool_all = true                        # Pool ALL defined MCPs
exclude_mcps = ["chrome"]              # MCPs that need per-session isolation

# Timeouts
socket_wait_timeout = 5                # Seconds to wait for socket ready
request_timeout = 300                  # 5 min max request duration
keepalive_interval = 30                # Client keepalive ping
idle_client_timeout = 60               # Remove idle clients after this

# Health & restart
health_check_interval = 10             # Seconds between health checks
max_restarts = 10                      # After this, mark permanently failed
restart_backoff_base = 1               # Initial restart delay (seconds)
restart_backoff_max = 60               # Max restart delay (seconds)

# Backpressure
max_pending_requests_per_client = 100  # Per-client queue limit
max_clients_per_mcp = 50               # Max concurrent sessions
circuit_breaker_threshold = 3          # Failures before circuit opens
circuit_breaker_reset = 30             # Seconds before circuit half-opens

# Docker integration
tcp_relay_enabled = true               # Enable TCP relay for containers
tcp_relay_port_range = [19000, 19999]  # Port range for relays

fallback_to_stdio = true               # Fall back if socket fails

[mcps.context7]
command = "npx"
args = ["-y", "context7"]
description = "Library documentation and code examples"
pool = true

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

### Phase 0: Infrastructure Setup (1 day)

**Prerequisite work before any code:**

1. Create `~/.agents-in-a-box/sockets/` directory structure
2. Set up module skeleton with all files
3. Add logging infrastructure with tracing
4. Create test harness for concurrent client simulation

**Deliverables**:
- Module structure in place
- Directory creation with proper permissions
- Test fixtures ready

### Phase 1: Request ID Rewriting (2 days) - CRITICAL PATH

**This is the most important component. Get it right first.**

1. Implement `RequestRouter` with ID rewriting
2. Implement `ProxyRequestId` generation (UUID-based)
3. Implement `OriginalRequestId` parsing and restoration
4. Add TTL cleanup for abandoned requests
5. Comprehensive unit tests for concurrent ID rewriting

**Deliverables**:
- Request ID rewriting works correctly
- Fuzz tests pass with random concurrent requests
- No ID collisions possible by design

### Phase 2: Backpressure & Circuit Breaker (2 days)

1. Implement `BoundedChannel` with capacity limits
2. Implement `CircuitBreaker` state machine
3. Add per-client pending request limits
4. Add JSON-RPC error responses for overload
5. Test under simulated slow MCP

**Deliverables**:
- System degrades gracefully under load
- No OOM possible from queue buildup
- Circuit breaker prevents cascade failures

### Phase 3: Client Lifecycle Management (2 days)

1. Implement `ClientManager` with liveness detection
2. Add keepalive ping/pong (30s interval)
3. Add idle client timeout (60s)
4. Handle EPIPE/BrokenPipe on write
5. Background reaper task
6. Test client disconnect scenarios

**Deliverables**:
- Dead clients are detected and removed
- No FD leaks from crashed sessions
- Proper error handling on write failures

### Phase 4: Process Supervision (1-2 days)

1. Implement `ProcessSupervisor` with tokio::process
2. Add proper SIGCHLD handling (automatic with tokio)
3. Implement exponential backoff for restarts
4. Add max restart limit with permanent failure state
5. Test MCP crash and recovery scenarios

**Deliverables**:
- No zombie processes
- Automatic recovery from MCP crashes
- Gives up after too many failures

### Phase 5: Socket Proxy Core (2 days)

1. Implement `SocketProxy` combining all components
2. Wire up request routing, backpressure, client management
3. Implement notification handling with coalescing
4. Add lock file protocol
5. Integration test with real MCP server

**Deliverables**:
- Single MCP works end-to-end
- Handles concurrent requests correctly
- Proper cleanup on shutdown

### Phase 6: Pool Management (1-2 days)

1. Implement `McpSocketPool` managing multiple proxies
2. Add health monitoring background task
3. Implement graceful shutdown with request draining
4. Add socket discovery for multi-instance
5. Clean up stale sockets on startup

**Deliverables**:
- Multiple MCPs managed correctly
- Clean startup/shutdown
- Multi-instance coordination works

### Phase 7: Docker Integration (1 day)

1. Implement `TcpRelay` for container access
2. Update session lifecycle to use TCP relay
3. Generate container-friendly MCP config with socat
4. Test with Docker session

**Deliverables**:
- Docker containers can access pool
- No socket mounting required
- Container isolation preserved

### Phase 8: Integration & Polish (2 days)

1. Integrate pool into `App::new()`
2. Integrate into `SessionLifecycleManager`
3. Integrate into `InteractiveSessionManager`
4. Update configuration parsing
5. Add metrics collection
6. Comprehensive integration tests

**Deliverables**:
- Full integration complete
- Sessions use pooled MCP
- Fallback works when pool unavailable

### Phase 9: Testing & Documentation (1-2 days)

1. Load testing with 20+ concurrent sessions
2. Chaos testing (kill processes, disconnect clients)
3. Memory leak testing with valgrind/heaptrack
4. Update CLAUDE.md
5. Add troubleshooting guide

**Deliverables**:
- Tested under realistic load
- No memory/FD leaks
- Documentation complete

---

## Platform Considerations

| Platform | Socket Pooling | TCP Relay | Notes |
|----------|----------------|-----------|-------|
| macOS | Full support | Full | Primary development platform |
| Linux | Full support | Full | Native Unix sockets |
| WSL2 | Full support | Full | Unix sockets work |
| WSL1 | Auto-disabled | N/A | Falls back to stdio |
| Windows | Auto-disabled | N/A | Falls back to stdio |

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

## Error Handling Summary

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| Client sends when queue full | Return JSON-RPC error `-32000` | Client can retry |
| Circuit breaker open | Return JSON-RPC error `-32001` | Wait for half-open |
| Request timeout (5 min) | Return JSON-RPC error `-32002` | Remove from router |
| Client disconnects | Remove from client map | Clean up pending requests |
| MCP crashes | Mark failed, attempt restart | Exponential backoff |
| Too many MCP restarts | Mark permanently failed | Fall back to stdio |
| Socket dir not writable | Skip pool, log error | Use stdio mode |
| Lock file contention | Skip that MCP | Another instance owns it |

---

## Observability

### Logging Levels

```rust
// ERROR: Things that need immediate attention
tracing::error!("MCP {} permanently failed after {} restarts", name, count);

// WARN: Degraded operation
tracing::warn!("Client {} hit backpressure limit, request rejected", session_id);
tracing::warn!("Circuit breaker opened for MCP {}", name);

// INFO: Normal operation milestones
tracing::info!("MCP pool started with {} servers", count);
tracing::info!("MCP {} restarted successfully", name);

// DEBUG: Detailed operation
tracing::debug!("Request {} routed to session {}", proxy_id, session_id);
tracing::debug!("Client {} passed keepalive check", session_id);

// TRACE: Very detailed
tracing::trace!("Raw message from MCP {}: {:?}", name, message);
```

### Metrics to Track

```rust
pub struct Metrics {
    // Counters
    requests_total: Counter,
    requests_success: Counter,
    requests_error: Counter,
    requests_timeout: Counter,
    backpressure_rejections: Counter,
    circuit_breaker_trips: Counter,
    client_disconnects: Counter,
    mcp_restarts: Counter,

    // Gauges
    active_clients: Gauge,
    pending_requests: Gauge,
    active_mcps: Gauge,

    // Histograms
    request_latency_ms: Histogram,
    queue_depth: Histogram,
}
```

---

## Dependencies

### Existing (No Changes)

```toml
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
```

### Potentially Useful

```toml
# For file locking (optional, can use fcntl directly)
fs2 = "0.4"

# For metrics (optional, can use counters directly)
metrics = "0.21"
```

---

## References

- [agent-deck MCP Socket Pooling Research](./2026-01-12_22-39-30_agent-deck-mcp-socket-pooling.md)
- [GitHub - asheshgoplani/agent-deck](https://github.com/asheshgoplani/agent-deck)
- [MCP Transports Specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [Rust std::os::unix::net](https://doc.rust-lang.org/std/os/unix/net/index.html)
- [tokio::process](https://docs.rs/tokio/latest/tokio/process/index.html)
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)

---

## Estimated Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 0: Infrastructure | 1 day | None |
| Phase 1: Request ID Rewriting | 2 days | Phase 0 |
| Phase 2: Backpressure | 2 days | Phase 0 |
| Phase 3: Client Lifecycle | 2 days | Phase 0 |
| Phase 4: Process Supervision | 1-2 days | Phase 0 |
| Phase 5: Socket Proxy Core | 2 days | Phases 1-4 |
| Phase 6: Pool Management | 1-2 days | Phase 5 |
| Phase 7: Docker Integration | 1 day | Phase 6 |
| Phase 8: Integration | 2 days | Phase 6-7 |
| Phase 9: Testing & Docs | 1-2 days | Phase 8 |

**Total: 15-18 days of focused work**

---

## Success Criteria

1. **No data corruption**: Request IDs never collide, responses always reach correct session
2. **No resource exhaustion**: Memory bounded, FDs bounded, no zombies
3. **Graceful degradation**: Overload → backpressure, not crash
4. **Self-healing**: MCP crash → automatic recovery with backoff
5. **Observable**: Clear logging, metrics for debugging
6. **Secure**: Sockets in private directory, no container escape
7. **Production-ready**: Tested under load, chaos tested

---

## Next Steps

1. Create GitHub issue for MCP socket pooling feature with this plan
2. Begin Phase 0: Create module skeleton and test infrastructure
3. Implement Phase 1 (Request ID Rewriting) - this is the critical path
4. Iterate through remaining phases
5. Load test with 20+ concurrent sessions before merge
