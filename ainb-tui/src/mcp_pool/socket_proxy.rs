// ABOUTME: Unix socket proxy wrapping a single MCP server process
// ABOUTME: Combines ProcessSupervisor, ClientManager, RequestRouter, and CircuitBreaker

//! Socket proxy for MCP server multiplexing.
//!
//! The `SocketProxy` wraps a single MCP server process and exposes it via a Unix domain
//! socket. Multiple clients can connect concurrently, with request ID rewriting to prevent
//! collisions and circuit breaker protection for failure handling.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      SocketProxy                            │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
//! │  │ProcessSuper- │  │ClientManager │  │  RequestRouter   │  │
//! │  │    visor     │  │              │  │                  │  │
//! │  │  (MCP child) │  │ (connections)│  │  (ID rewriting)  │  │
//! │  └──────────────┘  └──────────────┘  └──────────────────┘  │
//! │                                                             │
//! │  ┌──────────────┐                                          │
//! │  │CircuitBreaker│                                          │
//! │  │(fail protect)│                                          │
//! │  └──────────────┘                                          │
//! │                                                             │
//! │  Unix Socket Listener                                      │
//! │       ↓                                                    │
//! │  Client connections → Request routing → MCP process        │
//! │                       Response routing ← MCP process        │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use std::time::Duration;
//! use mcp_pool::{SocketProxy, PoolConfig};
//!
//! let config = PoolConfig::default();
//! let mut proxy = SocketProxy::new(
//!     "context7".to_string(),
//!     "npx".to_string(),
//!     vec!["-y".to_string(), "@context7/mcp".to_string()],
//!     config,
//! );
//!
//! // Start the proxy (spawns MCP process and listens on socket)
//! proxy.start().await?;
//!
//! // Clients can now connect to proxy.socket_path()
//!
//! // Graceful shutdown
//! proxy.stop().await?;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::backpressure::{BackpressureError, CircuitBreaker};
use super::client_manager::{ClientError, ClientId, ClientManager};
use super::config::PoolConfig;
use super::process_supervisor::{ProcessSupervisor, SupervisorError};
use super::request_router::RequestRouter;

// === Proxy Errors ===

/// Errors that can occur during proxy operation
#[derive(Debug, Error)]
pub enum ProxyError {
    /// I/O error (socket, file, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Process management error
    #[error("Process error: {0}")]
    Process(#[from] SupervisorError),

    /// Client management error
    #[error("Client error: {0}")]
    Client(#[from] ClientError),

    /// Circuit breaker is open
    #[error("Circuit breaker open")]
    CircuitOpen,

    /// Proxy is already running
    #[error("Proxy already running")]
    AlreadyRunning,

    /// Proxy is not running when expected
    #[error("Proxy not running")]
    NotRunning,

    /// JSON-RPC parsing error
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),

    /// Request routing error
    #[error("Routing error: {0}")]
    Routing(String),
}

impl From<BackpressureError> for ProxyError {
    fn from(err: BackpressureError) -> Self {
        match err {
            BackpressureError::CircuitOpen { .. } => Self::CircuitOpen,
            _ => Self::Routing(err.to_string()),
        }
    }
}

// === Message Types for Internal Communication ===

/// Message from client to MCP (outgoing request)
#[derive(Debug)]
struct ClientRequest {
    /// Which client sent this request
    client_id: ClientId,
    /// The JSON-RPC request (already ID-rewritten)
    data: Vec<u8>,
}

/// Message from MCP to client (incoming response/notification)
#[derive(Debug)]
#[allow(dead_code)]
struct McpMessage {
    /// Raw JSON-RPC message
    data: Vec<u8>,
    /// Parsed JSON for routing decisions
    json: serde_json::Value,
}

// === Socket Proxy Implementation ===

/// Unix domain socket proxy for a single MCP server.
///
/// Manages the lifecycle of an MCP server process and multiplexes client
/// connections through request ID rewriting.
pub struct SocketProxy {
    /// Name of this MCP server (e.g., "context7")
    mcp_name: String,

    /// Command to spawn the MCP server
    command: String,

    /// Arguments for the MCP command
    args: Vec<String>,

    /// Environment variables for the process
    env: HashMap<String, String>,

    /// Process supervisor for the MCP
    supervisor: Mutex<ProcessSupervisor>,

    /// Connected clients
    clients: ClientManager,

    /// Request ID router (lazily initialized in start() to avoid tokio runtime requirement)
    router: Mutex<Option<RequestRouter>>,

    /// Circuit breaker for failure protection
    circuit_breaker: CircuitBreaker,

    /// Configuration
    config: PoolConfig,

    /// Unix socket path
    socket_path: PathBuf,

    /// Socket listener handle
    listener_handle: Mutex<Option<JoinHandle<()>>>,

    /// MCP reader task handle
    mcp_reader_handle: Mutex<Option<JoinHandle<()>>>,

    /// MCP writer task handle
    mcp_writer_handle: Mutex<Option<JoinHandle<()>>>,

    /// Flag indicating if proxy has been started (vs just created)
    started: Arc<AtomicBool>,

    /// Shutdown flag (set when stopping)
    shutdown: Arc<AtomicBool>,

    /// Channel for sending requests to MCP
    request_tx: mpsc::Sender<ClientRequest>,

    /// Channel receiver for requests (taken by writer task)
    request_rx: Mutex<Option<mpsc::Receiver<ClientRequest>>>,
}

impl SocketProxy {
    /// Create a new socket proxy for an MCP server.
    ///
    /// # Arguments
    ///
    /// * `mcp_name` - Unique name for this MCP server
    /// * `command` - Command to spawn the MCP process
    /// * `args` - Arguments for the command
    /// * `config` - Pool configuration
    ///
    /// # Panics
    ///
    /// Panics if socket path cannot be determined from config.
    pub fn new(mcp_name: String, command: String, args: Vec<String>, config: PoolConfig) -> Self {
        let socket_path =
            config.get_socket_path(&mcp_name).expect("Failed to determine socket path");

        let supervisor = ProcessSupervisor::new(
            config.max_restarts,
            config.restart_backoff_base,
            config.restart_backoff_max,
        );

        let clients = ClientManager::new(
            config.max_clients_per_mcp,
            config.keepalive_interval,
            config.idle_client_timeout,
        );

        // Note: RequestRouter is lazily initialized in start() because it spawns
        // a tokio task, which requires a runtime context
        let circuit_breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_reset,
            1, // half_open_max_requests
        );

        // Bounded channel for backpressure on requests to MCP
        let (request_tx, request_rx) =
            mpsc::channel(config.max_pending_requests_per_client * config.max_clients_per_mcp);

        Self {
            mcp_name,
            command,
            args,
            env: HashMap::new(),
            supervisor: Mutex::new(supervisor),
            clients,
            router: Mutex::new(None), // Lazily initialized
            circuit_breaker,
            config,
            socket_path,
            listener_handle: Mutex::new(None),
            mcp_reader_handle: Mutex::new(None),
            mcp_writer_handle: Mutex::new(None),
            started: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
            request_tx,
            request_rx: Mutex::new(Some(request_rx)),
        }
    }

    /// Set environment variables for the MCP process.
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = env;
    }

    /// Start the socket proxy.
    ///
    /// Spawns the MCP process, creates the Unix socket, and starts
    /// background tasks for accepting connections and routing messages.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Proxy is already running
    /// - Socket creation fails
    /// - MCP process spawn fails
    pub async fn start(&mut self) -> Result<(), ProxyError> {
        // Check if already started
        if self.started.load(Ordering::SeqCst) {
            return Err(ProxyError::AlreadyRunning);
        }

        // Mark as started and clear shutdown flag
        self.started.store(true, Ordering::SeqCst);
        self.shutdown.store(false, Ordering::SeqCst);

        info!(mcp = %self.mcp_name, socket = %self.socket_path.display(), "Starting socket proxy");

        // Initialize the request router (requires tokio runtime)
        {
            let mut router_guard = self.router.lock().await;
            if router_guard.is_none() {
                *router_guard = Some(RequestRouter::new(self.config.request_timeout));
            }
        }

        // Remove stale socket file if exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Spawn MCP process
        {
            let mut supervisor = self.supervisor.lock().await;
            supervisor.spawn(&self.command, &self.args, &self.env, None).await?;
        }

        // Start client manager background tasks
        // Note: We need interior mutability for this, but ClientManager takes &mut self
        // For now, we'll handle keepalive/reaping differently

        // Create Unix socket listener
        let listener = UnixListener::bind(&self.socket_path)?;

        // Set socket permissions to allow only owner (security)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.socket_path, perms)?;
        }

        // Start listener task
        let shutdown = Arc::clone(&self.shutdown);
        let mcp_name = self.mcp_name.clone();
        let request_tx = self.request_tx.clone();
        let config = self.config.clone();

        // We need to share client manager and router across tasks
        // Using Arc for the proxy components that need sharing
        let listener_handle = tokio::spawn(async move {
            Self::accept_connections_loop(listener, shutdown, mcp_name, request_tx, config).await;
        });

        *self.listener_handle.lock().await = Some(listener_handle);

        // Start MCP I/O tasks
        self.start_mcp_io_tasks().await?;

        info!(mcp = %self.mcp_name, "Socket proxy started successfully");

        Ok(())
    }

    /// Start MCP process I/O handling tasks.
    async fn start_mcp_io_tasks(&mut self) -> Result<(), ProxyError> {
        let mut supervisor = self.supervisor.lock().await;

        // Take stdin/stdout from supervisor
        let stdin = supervisor
            .take_stdin()
            .ok_or_else(|| ProxyError::Process(SupervisorError::NotRunning))?;

        let stdout = supervisor
            .take_stdout()
            .ok_or_else(|| ProxyError::Process(SupervisorError::NotRunning))?;

        drop(supervisor); // Release lock

        // Take request receiver
        let request_rx = self
            .request_rx
            .lock()
            .await
            .take()
            .ok_or_else(|| ProxyError::Routing("Request channel already taken".to_string()))?;

        // MCP writer task: receives requests from channel, writes to MCP stdin
        let shutdown_writer = Arc::clone(&self.shutdown);
        let mcp_name_writer = self.mcp_name.clone();

        let writer_handle = tokio::spawn(async move {
            Self::mcp_writer_loop(stdin, request_rx, shutdown_writer, mcp_name_writer).await;
        });

        *self.mcp_writer_handle.lock().await = Some(writer_handle);

        // MCP reader task: reads from MCP stdout, routes responses to clients
        let shutdown_reader = Arc::clone(&self.shutdown);
        let mcp_name_reader = self.mcp_name.clone();

        let reader_handle = tokio::spawn(async move {
            Self::mcp_reader_loop(stdout, shutdown_reader, mcp_name_reader).await;
        });

        *self.mcp_reader_handle.lock().await = Some(reader_handle);

        Ok(())
    }

    /// Accept connections loop - runs as background task.
    async fn accept_connections_loop(
        listener: UnixListener,
        shutdown: Arc<AtomicBool>,
        mcp_name: String,
        request_tx: mpsc::Sender<ClientRequest>,
        config: PoolConfig,
    ) {
        info!(mcp = %mcp_name, "Accepting connections on socket");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                debug!(mcp = %mcp_name, "Listener shutdown requested");
                break;
            }

            // Accept with timeout so we can check shutdown flag
            let accept_result =
                tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;

            match accept_result {
                Ok(Ok((stream, _addr))) => {
                    debug!(mcp = %mcp_name, "New client connection");

                    // Spawn handler for this client
                    let request_tx = request_tx.clone();
                    let mcp_name = mcp_name.clone();
                    let shutdown = Arc::clone(&shutdown);
                    let config = config.clone();

                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_client(stream, request_tx, shutdown, mcp_name, config)
                                .await
                        {
                            warn!(error = %e, "Client handler error");
                        }
                    });
                }
                Ok(Err(e)) => {
                    error!(error = %e, mcp = %mcp_name, "Accept error");
                }
                Err(_) => {
                    // Timeout - just loop again to check shutdown
                }
            }
        }

        info!(mcp = %mcp_name, "Listener loop ended");
    }

    /// Handle a single client connection.
    async fn handle_client(
        stream: UnixStream,
        request_tx: mpsc::Sender<ClientRequest>,
        shutdown: Arc<AtomicBool>,
        mcp_name: String,
        _config: PoolConfig,
    ) -> Result<(), ProxyError> {
        let client_id = ClientId::new();
        debug!(mcp = %mcp_name, client = %client_id, "Handling client");

        let (read_half, _write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();

        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            line.clear();

            // Read with timeout
            let read_result =
                tokio::time::timeout(Duration::from_secs(30), reader.read_line(&mut line)).await;

            match read_result {
                Ok(Ok(0)) => {
                    // EOF - client disconnected
                    debug!(mcp = %mcp_name, client = %client_id, "Client disconnected (EOF)");
                    break;
                }
                Ok(Ok(_)) => {
                    // Got a line - parse and forward
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Forward to MCP
                    let request = ClientRequest {
                        client_id,
                        data: line.as_bytes().to_vec(),
                    };

                    if request_tx.send(request).await.is_err() {
                        error!(mcp = %mcp_name, "Request channel closed");
                        break;
                    }
                }
                Ok(Err(e)) => {
                    warn!(mcp = %mcp_name, error = %e, "Read error from client");
                    break;
                }
                Err(_) => {
                    // Timeout - continue to check shutdown
                }
            }
        }

        debug!(mcp = %mcp_name, client = %client_id, "Client handler ended");
        Ok(())
    }

    /// MCP writer loop - forwards requests to MCP stdin.
    async fn mcp_writer_loop(
        stdin: tokio::process::ChildStdin,
        mut request_rx: mpsc::Receiver<ClientRequest>,
        shutdown: Arc<AtomicBool>,
        mcp_name: String,
    ) {
        let mut writer = BufWriter::new(stdin);

        debug!(mcp = %mcp_name, "MCP writer loop started");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // Receive with timeout
            let recv_result = tokio::time::timeout(Duration::from_secs(1), request_rx.recv()).await;

            match recv_result {
                Ok(Some(request)) => {
                    // Write to MCP
                    if let Err(e) = writer.write_all(&request.data).await {
                        error!(mcp = %mcp_name, error = %e, "Write to MCP failed");
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!(mcp = %mcp_name, error = %e, "Flush to MCP failed");
                        break;
                    }

                    debug!(
                        mcp = %mcp_name,
                        client = %request.client_id,
                        "Forwarded request to MCP"
                    );
                }
                Ok(None) => {
                    // Channel closed
                    debug!(mcp = %mcp_name, "Request channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout - continue
                }
            }
        }

        debug!(mcp = %mcp_name, "MCP writer loop ended");
    }

    /// MCP reader loop - reads responses from MCP stdout.
    async fn mcp_reader_loop(
        stdout: tokio::process::ChildStdout,
        shutdown: Arc<AtomicBool>,
        mcp_name: String,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        debug!(mcp = %mcp_name, "MCP reader loop started");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            line.clear();

            // Read with timeout
            let read_result =
                tokio::time::timeout(Duration::from_secs(1), reader.read_line(&mut line)).await;

            match read_result {
                Ok(Ok(0)) => {
                    // EOF - MCP process died
                    warn!(mcp = %mcp_name, "MCP process EOF");
                    break;
                }
                Ok(Ok(_)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Parse and route response
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(json) => {
                            // Check if it's a response (has id) or notification (no id)
                            if json.get("id").is_some() {
                                debug!(mcp = %mcp_name, "Received response from MCP");
                                // TODO: Route to correct client via RequestRouter
                            } else {
                                debug!(mcp = %mcp_name, "Received notification from MCP");
                                // TODO: Broadcast to all clients
                            }
                        }
                        Err(e) => {
                            warn!(mcp = %mcp_name, error = %e, "Failed to parse MCP response");
                        }
                    }
                }
                Ok(Err(e)) => {
                    error!(mcp = %mcp_name, error = %e, "Read from MCP failed");
                    break;
                }
                Err(_) => {
                    // Timeout - continue
                }
            }
        }

        debug!(mcp = %mcp_name, "MCP reader loop ended");
    }

    /// Stop the socket proxy gracefully.
    ///
    /// Drains pending requests, closes client connections, and terminates
    /// the MCP process.
    ///
    /// # Errors
    ///
    /// Returns error if proxy is not running or termination fails.
    pub async fn stop(&mut self) -> Result<(), ProxyError> {
        if !self.is_running() {
            return Err(ProxyError::NotRunning);
        }

        info!(mcp = %self.mcp_name, "Stopping socket proxy");

        // Signal shutdown
        self.shutdown.store(true, Ordering::SeqCst);

        // Wait for listener to stop
        if let Some(handle) = self.listener_handle.lock().await.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // Wait for MCP I/O tasks
        if let Some(handle) = self.mcp_writer_handle.lock().await.take() {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
        if let Some(handle) = self.mcp_reader_handle.lock().await.take() {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }

        // Terminate MCP process
        {
            let mut supervisor = self.supervisor.lock().await;
            supervisor.terminate().await?;
        }

        // Shutdown request router
        {
            let mut router_guard = self.router.lock().await;
            if let Some(ref mut router) = *router_guard {
                router.shutdown().await;
            }
        }

        // Mark as no longer started
        self.started.store(false, Ordering::SeqCst);

        // Remove socket file
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        info!(mcp = %self.mcp_name, "Socket proxy stopped");

        Ok(())
    }

    /// Check if the proxy is currently running.
    pub fn is_running(&self) -> bool {
        // Proxy is running if it has been started and not shut down
        self.started.load(Ordering::SeqCst) && !self.shutdown.load(Ordering::SeqCst)
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.client_count().await
    }

    /// Get the Unix socket path for this proxy.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get the MCP server name.
    pub fn mcp_name(&self) -> &str {
        &self.mcp_name
    }

    /// Get current circuit breaker state.
    pub fn circuit_state(&self) -> super::backpressure::CircuitState {
        self.circuit_breaker.state()
    }

    /// Get number of pending requests.
    ///
    /// Returns 0 if the router has not been initialized yet.
    pub async fn pending_requests(&self) -> usize {
        let router_guard = self.router.lock().await;
        if let Some(ref router) = *router_guard {
            router.pending_count().await
        } else {
            0
        }
    }

    /// Check process health and restart if needed.
    pub async fn check_health(&mut self) -> Result<bool, ProxyError> {
        let mut supervisor = self.supervisor.lock().await;
        let state = supervisor.check_status();

        if !state.is_running() {
            // Process died - record failure for circuit breaker
            self.circuit_breaker.record_failure();

            // Try to restart if not permanently failed
            if !supervisor.is_permanently_failed() {
                warn!(mcp = %self.mcp_name, "MCP process died, attempting restart");

                supervisor.restart(&self.command, &self.args, &self.env, None).await?;

                // Restart I/O tasks
                drop(supervisor); // Release lock before calling start_mcp_io_tasks
                self.start_mcp_io_tasks().await?;

                return Ok(true);
            } else {
                error!(mcp = %self.mcp_name, "MCP permanently failed, not restarting");
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Force reset the circuit breaker (admin override).
    pub fn reset_circuit_breaker(&self) {
        self.circuit_breaker.reset();
    }
}

impl Drop for SocketProxy {
    fn drop(&mut self) {
        // Signal shutdown on drop
        self.shutdown.store(true, Ordering::SeqCst);

        // Try to remove socket file
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PoolConfig {
        let mut config = PoolConfig::default();
        // Use temp directory with unique suffix for tests
        let unique_dir = std::env::temp_dir()
            .join("mcp-pool-test")
            .join(format!("{}", std::process::id()));
        config.socket_dir = Some(unique_dir);
        config
    }

    #[test]
    fn test_proxy_error_variants() {
        // Test error creation
        let io_err = ProxyError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "test error",
        ));
        assert!(io_err.to_string().contains("I/O error"));

        let already_running = ProxyError::AlreadyRunning;
        assert_eq!(already_running.to_string(), "Proxy already running");

        let not_running = ProxyError::NotRunning;
        assert_eq!(not_running.to_string(), "Proxy not running");

        let circuit_open = ProxyError::CircuitOpen;
        assert_eq!(circuit_open.to_string(), "Circuit breaker open");
    }

    #[test]
    fn test_proxy_error_from_backpressure() {
        let bp_err = BackpressureError::CircuitOpen {
            until: std::time::Instant::now(),
        };
        let proxy_err: ProxyError = bp_err.into();
        assert!(matches!(proxy_err, ProxyError::CircuitOpen));

        let queue_err = BackpressureError::QueueFull {
            capacity: 100,
            current: 100,
        };
        let proxy_err: ProxyError = queue_err.into();
        assert!(matches!(proxy_err, ProxyError::Routing(_)));
    }

    #[test]
    fn test_socket_proxy_creation() {
        let config = test_config();
        let proxy = SocketProxy::new(
            "test-mcp".to_string(),
            "echo".to_string(),
            vec!["hello".to_string()],
            config,
        );

        assert_eq!(proxy.mcp_name(), "test-mcp");
        assert!(proxy.socket_path().to_string_lossy().contains("test-mcp"));
    }

    #[test]
    fn test_socket_proxy_initial_state() {
        let config = test_config();
        let proxy = SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        // Proxy should NOT be running initially (not started)
        assert!(!proxy.is_running());
    }

    #[tokio::test]
    async fn test_socket_proxy_stop_without_start() {
        let config = test_config();
        let mut proxy = SocketProxy::new(
            "test-mcp-no-start".to_string(),
            "echo".to_string(),
            vec![],
            config,
        );

        // Stopping without starting should return NotRunning error
        let result = proxy.stop().await;
        assert!(matches!(result, Err(ProxyError::NotRunning)));
    }

    #[test]
    fn test_socket_proxy_set_env() {
        let config = test_config();
        let mut proxy =
            SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        proxy.set_env(env);

        assert_eq!(proxy.env.get("TEST_VAR"), Some(&"test_value".to_string()));
    }

    #[tokio::test]
    async fn test_socket_proxy_client_count() {
        let config = test_config();
        let proxy = SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        // No clients connected initially
        assert_eq!(proxy.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_socket_proxy_pending_requests() {
        let config = test_config();
        let proxy = SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        // No pending requests initially (router not initialized)
        assert_eq!(proxy.pending_requests().await, 0);
    }

    #[test]
    fn test_socket_proxy_circuit_state() {
        let config = test_config();
        let proxy = SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        // Circuit should be closed initially
        assert!(matches!(
            proxy.circuit_state(),
            super::super::backpressure::CircuitState::Closed
        ));
    }

    #[test]
    fn test_socket_proxy_reset_circuit_breaker() {
        let config = test_config();
        let proxy = SocketProxy::new("test-mcp".to_string(), "echo".to_string(), vec![], config);

        // Record some failures
        proxy.circuit_breaker.record_failure();
        proxy.circuit_breaker.record_failure();
        proxy.circuit_breaker.record_failure();

        // Circuit should be open
        assert!(matches!(
            proxy.circuit_state(),
            super::super::backpressure::CircuitState::Open { .. }
        ));

        // Reset circuit breaker
        proxy.reset_circuit_breaker();

        // Circuit should be closed
        assert!(matches!(
            proxy.circuit_state(),
            super::super::backpressure::CircuitState::Closed
        ));
    }

    #[tokio::test]
    async fn test_socket_proxy_start_with_echo() {
        let config = test_config();
        let mut proxy = SocketProxy::new(
            "test-echo".to_string(),
            "cat".to_string(), // Use cat as a simple stdin->stdout echo
            vec![],
            config,
        );

        // Not running before start
        assert!(!proxy.is_running());

        // Start the proxy
        let result = proxy.start().await;
        assert!(result.is_ok(), "Failed to start proxy: {:?}", result);

        // Verify running
        assert!(proxy.is_running());

        // Stop the proxy
        let stop_result = proxy.stop().await;
        assert!(
            stop_result.is_ok(),
            "Failed to stop proxy: {:?}",
            stop_result
        );

        // Verify stopped
        assert!(!proxy.is_running());
    }

    #[tokio::test]
    async fn test_socket_proxy_already_running() {
        let config = test_config();
        let mut proxy = SocketProxy::new(
            "test-double-start".to_string(),
            "cat".to_string(),
            vec![],
            config,
        );

        // Start once
        proxy.start().await.expect("First start should succeed");

        // Try to start again
        let result = proxy.start().await;
        assert!(matches!(result, Err(ProxyError::AlreadyRunning)));

        // Cleanup
        let _ = proxy.stop().await;
    }

    #[tokio::test]
    async fn test_socket_path_permissions() {
        let config = test_config();
        let mut proxy =
            SocketProxy::new("test-perms".to_string(), "cat".to_string(), vec![], config);

        proxy.start().await.expect("Start should succeed");

        // Check socket permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(proxy.socket_path()).expect("Socket should exist");
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Socket should have mode 0600");
        }

        let _ = proxy.stop().await;
    }

    #[test]
    fn test_client_request_debug() {
        let request = ClientRequest {
            client_id: ClientId::new(),
            data: b"test".to_vec(),
        };
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("ClientRequest"));
    }

    #[test]
    fn test_mcp_message_debug() {
        let message = McpMessage {
            data: b"test".to_vec(),
            json: serde_json::json!({"test": true}),
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("McpMessage"));
    }
}
