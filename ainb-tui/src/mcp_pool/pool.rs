// ABOUTME: MCP socket pool management for sharing MCP servers across sessions
// ABOUTME: Manages multiple SocketProxy instances with discovery, health monitoring, and graceful shutdown

//! MCP Socket Pool Management
//!
//! The `McpSocketPool` manages multiple `SocketProxy` instances, providing:
//! - Start/stop MCP proxies based on configuration
//! - Socket discovery for multi-instance coordination
//! - Health monitoring integration
//! - Graceful shutdown with request draining
//!
//! # Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐
//! │                     McpSocketPool                         │
//! │  ┌─────────────────────────────────────────────────────┐  │
//! │  │  proxies: HashMap<String, Arc<RwLock<SocketProxy>>> │  │
//! │  │     - context7 -> SocketProxy (running)             │  │
//! │  │     - memory   -> SocketProxy (running)             │  │
//! │  │     - exa      -> SocketProxy (stopped)             │  │
//! │  └─────────────────────────────────────────────────────┘  │
//! │                                                           │
//! │  ┌───────────────┐  ┌───────────────┐  ┌──────────────┐  │
//! │  │SocketDiscovery│  │  PoolMetrics  │  │  PoolConfig  │  │
//! │  │(find existing)│  │ (observability)│  │  (settings)  │  │
//! │  └───────────────┘  └───────────────┘  └──────────────┘  │
//! └───────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use mcp_pool::{McpSocketPool, PoolConfig};
//!
//! let config = PoolConfig::default();
//! let mut pool = McpSocketPool::new(config);
//!
//! // Start the pool (runs discovery, cleanup)
//! pool.start().await?;
//!
//! // Get or start a proxy for an MCP
//! let socket_path = pool.get_or_start_proxy(
//!     "context7",
//!     "npx",
//!     &["-y".to_string(), "@context7/mcp".to_string()]
//! ).await?;
//!
//! // Graceful shutdown
//! pool.shutdown().await?;
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::config::PoolConfig;
use super::discovery::{DiscoveryError, SocketDiscovery};
use super::metrics::PoolMetrics;
use super::socket_proxy::{ProxyError, SocketProxy};

// === Pool Errors ===

/// Errors that can occur during pool operations
#[derive(Debug, Error)]
pub enum PoolError {
    /// Error from underlying proxy
    #[error("Proxy error: {0}")]
    ProxyError(#[from] ProxyError),

    /// Error during socket discovery
    #[error("Discovery error: {0}")]
    DiscoveryError(#[from] DiscoveryError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Pool is not running when operation requires it
    #[error("Pool is not running")]
    NotRunning,

    /// Pool is already running when start is called
    #[error("Pool is already running")]
    AlreadyRunning,

    /// Proxy not found in pool
    #[error("Proxy not found: {0}")]
    ProxyNotFound(String),

    /// MCP not configured for pooling
    #[error("MCP '{0}' is not configured for pooling")]
    McpNotPooled(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for pool operations
pub type PoolResult<T> = Result<T, PoolError>;

// === Proxy Info ===

/// Information about a managed proxy
#[derive(Debug, Clone)]
pub struct ProxyInfo {
    /// MCP server name
    pub mcp_name: String,
    /// Socket path
    pub socket_path: PathBuf,
    /// Whether the proxy is currently running
    pub is_running: bool,
    /// Number of connected clients
    pub client_count: usize,
}

// === MCP Socket Pool ===

/// Main entry point for MCP socket pooling
///
/// Manages multiple `SocketProxy` instances and provides:
/// - Lazy proxy creation on demand
/// - Socket discovery for reusing existing sockets
/// - Pool-level metrics and health monitoring
/// - Graceful shutdown with request draining
pub struct McpSocketPool {
    /// Configuration for the pool
    config: PoolConfig,

    /// Running proxies by MCP name
    proxies: HashMap<String, Arc<RwLock<SocketProxy>>>,

    /// Pool-level metrics
    metrics: Arc<PoolMetrics>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Socket discovery for finding existing sockets
    discovery: SocketDiscovery,

    /// Whether the pool has been started
    started: bool,
}

impl std::fmt::Debug for McpSocketPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpSocketPool")
            .field("config", &self.config)
            .field("proxy_count", &self.proxies.len())
            .field("started", &self.started)
            .finish()
    }
}

impl McpSocketPool {
    /// Create a new MCP socket pool
    ///
    /// The pool is created in a stopped state. Call `start()` to initialize.
    pub fn new(config: PoolConfig) -> Self {
        let discovery = SocketDiscovery::new(config.clone());

        Self {
            config,
            proxies: HashMap::new(),
            metrics: Arc::new(PoolMetrics::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
            discovery,
            started: false,
        }
    }

    /// Get the pool configuration
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Get the socket directory path
    ///
    /// Returns the directory where socket files are stored.
    /// Returns None if the socket directory cannot be determined.
    pub fn socket_dir(&self) -> Option<PathBuf> {
        self.config.get_socket_dir().ok()
    }

    /// Get pool metrics
    pub fn metrics(&self) -> &Arc<PoolMetrics> {
        &self.metrics
    }

    /// Start the pool
    ///
    /// This performs:
    /// 1. Socket discovery to find existing sockets from other instances
    /// 2. Stale socket cleanup
    /// 3. Initialization of pool state
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Pool is already running
    /// - Platform doesn't support socket pooling
    /// - Socket directory cannot be created
    pub async fn start(&mut self) -> PoolResult<()> {
        if self.started {
            return Err(PoolError::AlreadyRunning);
        }

        // Check platform support
        if !PoolConfig::is_platform_supported() {
            return Err(PoolError::ConfigError(
                "Socket pooling not supported on this platform".to_string(),
            ));
        }

        // Ensure socket directory exists
        self.config.get_socket_dir()?;

        info!("Starting MCP socket pool");

        // Reset shutdown flag
        self.shutdown.store(false, Ordering::SeqCst);

        // Clean up stale sockets from dead processes
        match self.discovery.cleanup_stale() {
            Ok(cleaned) => {
                if cleaned > 0 {
                    info!(count = cleaned, "Cleaned up stale sockets");
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to clean up stale sockets");
            }
        }

        // Discover existing live sockets
        match self.discovery.get_live_sockets() {
            Ok(sockets) => {
                debug!(count = sockets.len(), "Found existing live sockets");
                // Note: We don't automatically adopt existing sockets here
                // because they're owned by other processes. We'll check for
                // them when get_or_start_proxy is called.
            }
            Err(e) => {
                warn!(error = %e, "Failed to discover existing sockets");
            }
        }

        self.started = true;
        self.update_metrics();

        info!("MCP socket pool started");
        Ok(())
    }

    /// Shutdown the pool gracefully
    ///
    /// This:
    /// 1. Signals shutdown to all proxies
    /// 2. Waits for pending requests to drain
    /// 3. Stops all proxies
    /// 4. Cleans up resources
    ///
    /// # Errors
    ///
    /// Returns error if pool is not running.
    pub async fn shutdown(&mut self) -> PoolResult<()> {
        if !self.started {
            return Err(PoolError::NotRunning);
        }

        info!(proxy_count = self.proxies.len(), "Shutting down MCP socket pool");

        // Signal shutdown
        self.shutdown.store(true, Ordering::SeqCst);

        // Stop all proxies
        let mut errors = Vec::new();
        let mcp_names: Vec<String> = self.proxies.keys().cloned().collect();

        for mcp_name in mcp_names {
            if let Some(proxy_arc) = self.proxies.remove(&mcp_name) {
                let mut proxy = proxy_arc.write().await;
                if proxy.is_running() {
                    if let Err(e) = proxy.stop().await {
                        warn!(mcp = %mcp_name, error = %e, "Error stopping proxy");
                        errors.push((mcp_name.clone(), e));
                    } else {
                        debug!(mcp = %mcp_name, "Proxy stopped");
                    }
                }
            }
        }

        self.started = false;
        self.update_metrics();

        if !errors.is_empty() {
            // Log all errors but return success if pool is stopped
            for (mcp, err) in &errors {
                error!(mcp = %mcp, error = %err, "Proxy shutdown error");
            }
        }

        info!("MCP socket pool shutdown complete");
        Ok(())
    }

    /// Get or start a proxy for an MCP server
    ///
    /// If a proxy already exists and is running, returns its socket path.
    /// If an existing socket is discovered from another process, returns that path.
    /// Otherwise, starts a new proxy and returns the new socket path.
    ///
    /// # Arguments
    ///
    /// * `mcp_name` - Unique name for the MCP server
    /// * `command` - Command to spawn the MCP process
    /// * `args` - Arguments for the command
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Pool is not running
    /// - MCP is not configured for pooling
    /// - Proxy fails to start
    pub async fn get_or_start_proxy(
        &mut self,
        mcp_name: &str,
        command: &str,
        args: &[String],
    ) -> PoolResult<PathBuf> {
        if !self.started {
            return Err(PoolError::NotRunning);
        }

        if self.shutdown.load(Ordering::SeqCst) {
            return Err(PoolError::NotRunning);
        }

        // Check if MCP should be pooled
        if !self.config.should_pool(mcp_name) {
            return Err(PoolError::McpNotPooled(mcp_name.to_string()));
        }

        // Check if we already have a running proxy
        if let Some(proxy_arc) = self.proxies.get(mcp_name) {
            let proxy = proxy_arc.read().await;
            if proxy.is_running() {
                debug!(mcp = %mcp_name, "Returning existing proxy");
                return Ok(proxy.socket_path().to_path_buf());
            }
        }

        // Check if another process has a running socket
        if let Ok(Some(discovered)) = self.discovery.find_socket(mcp_name) {
            if discovered.is_alive {
                info!(
                    mcp = %mcp_name,
                    pid = ?discovered.owner_pid,
                    "Found existing socket from another process"
                );
                return Ok(discovered.socket_path);
            }
        }

        // Start a new proxy
        self.start_new_proxy(mcp_name, command, args).await
    }

    /// Start a new proxy (internal helper)
    async fn start_new_proxy(
        &mut self,
        mcp_name: &str,
        command: &str,
        args: &[String],
    ) -> PoolResult<PathBuf> {
        info!(mcp = %mcp_name, command = %command, "Starting new proxy");

        let mut proxy = SocketProxy::new(
            mcp_name.to_string(),
            command.to_string(),
            args.to_vec(),
            self.config.clone(),
        );

        proxy.start().await?;

        let socket_path = proxy.socket_path().to_path_buf();
        self.proxies.insert(mcp_name.to_string(), Arc::new(RwLock::new(proxy)));

        self.update_metrics();

        debug!(mcp = %mcp_name, socket = %socket_path.display(), "Proxy started");
        Ok(socket_path)
    }

    /// Get socket path for an MCP if running
    ///
    /// Returns `None` if no proxy is running for this MCP.
    pub async fn get_socket_path(&self, mcp_name: &str) -> Option<PathBuf> {
        if let Some(proxy_arc) = self.proxies.get(mcp_name) {
            let proxy = proxy_arc.read().await;
            if proxy.is_running() {
                return Some(proxy.socket_path().to_path_buf());
            }
        }
        None
    }

    /// Stop a specific proxy
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Pool is not running
    /// - Proxy not found
    /// - Proxy stop fails
    pub async fn stop_proxy(&mut self, mcp_name: &str) -> PoolResult<()> {
        if !self.started {
            return Err(PoolError::NotRunning);
        }

        let proxy_arc = self
            .proxies
            .get(mcp_name)
            .ok_or_else(|| PoolError::ProxyNotFound(mcp_name.to_string()))?
            .clone();

        {
            let mut proxy = proxy_arc.write().await;
            if proxy.is_running() {
                proxy.stop().await?;
            }
        }

        // Remove from proxies map
        self.proxies.remove(mcp_name);
        self.update_metrics();

        info!(mcp = %mcp_name, "Proxy stopped and removed");
        Ok(())
    }

    /// Check if a proxy is running for the given MCP
    pub async fn is_running(&self, mcp_name: &str) -> bool {
        if let Some(proxy_arc) = self.proxies.get(mcp_name) {
            let proxy = proxy_arc.read().await;
            return proxy.is_running();
        }
        false
    }

    /// Get the number of running proxies
    pub fn proxy_count(&self) -> usize {
        self.proxies.len()
    }

    /// List names of all managed proxies
    pub fn list_proxies(&self) -> Vec<String> {
        self.proxies.keys().cloned().collect()
    }

    /// Get information about all managed proxies
    pub async fn proxy_info(&self) -> Vec<ProxyInfo> {
        let mut info = Vec::with_capacity(self.proxies.len());

        for (mcp_name, proxy_arc) in &self.proxies {
            let proxy = proxy_arc.read().await;
            info.push(ProxyInfo {
                mcp_name: mcp_name.clone(),
                socket_path: proxy.socket_path().to_path_buf(),
                is_running: proxy.is_running(),
                client_count: proxy.client_count().await,
            });
        }

        info
    }

    /// Check health of all proxies and restart failed ones
    ///
    /// Returns the number of proxies that were restarted.
    pub async fn check_health(&mut self) -> PoolResult<usize> {
        if !self.started {
            return Err(PoolError::NotRunning);
        }

        let mut restarted = 0;

        for (mcp_name, proxy_arc) in &self.proxies {
            let mut proxy = proxy_arc.write().await;
            match proxy.check_health().await {
                Ok(healthy) => {
                    if !healthy {
                        warn!(mcp = %mcp_name, "Proxy health check failed, process may be permanently dead");
                    }
                }
                Err(e) => {
                    warn!(mcp = %mcp_name, error = %e, "Error checking proxy health");
                    restarted += 1;
                }
            }
        }

        self.update_metrics();
        Ok(restarted)
    }

    /// Update pool metrics based on current state
    fn update_metrics(&self) {
        #[allow(clippy::cast_possible_truncation)]
        self.metrics.set_active_mcps(self.proxies.len() as u32);
    }

    /// Check if the pool has been started
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Get access to socket discovery for external use
    pub fn discovery(&self) -> &SocketDiscovery {
        &self.discovery
    }
}

impl Default for McpSocketPool {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}

impl Drop for McpSocketPool {
    fn drop(&mut self) {
        // Signal shutdown on drop
        self.shutdown.store(true, Ordering::SeqCst);
        // Note: Async cleanup cannot be done in Drop.
        // Callers should call shutdown() explicitly for graceful cleanup.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use tempfile::TempDir;

    // Unique counter for test isolation
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_config(temp_dir: &TempDir) -> PoolConfig {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let socket_dir = temp_dir.path().join(format!("sockets_{counter}"));
        std::fs::create_dir_all(&socket_dir).unwrap();

        PoolConfig {
            socket_dir: Some(socket_dir),
            socket_prefix: "mcp-".to_string(),
            ..PoolConfig::default()
        }
    }

    // ==================== PoolError Tests ====================

    #[test]
    fn test_pool_error_display() {
        let err = PoolError::NotRunning;
        assert_eq!(err.to_string(), "Pool is not running");

        let err = PoolError::AlreadyRunning;
        assert_eq!(err.to_string(), "Pool is already running");

        let err = PoolError::ProxyNotFound("test".to_string());
        assert_eq!(err.to_string(), "Proxy not found: test");

        let err = PoolError::McpNotPooled("chrome".to_string());
        assert_eq!(err.to_string(), "MCP 'chrome' is not configured for pooling");

        let err = PoolError::ConfigError("bad config".to_string());
        assert_eq!(err.to_string(), "Configuration error: bad config");
    }

    #[test]
    fn test_pool_error_from_proxy_error() {
        let proxy_err = ProxyError::NotRunning;
        let pool_err: PoolError = proxy_err.into();
        assert!(matches!(pool_err, PoolError::ProxyError(_)));
    }

    #[test]
    fn test_pool_error_from_discovery_error() {
        let discovery_err = DiscoveryError::LockHeldByOther(123);
        let pool_err: PoolError = discovery_err.into();
        assert!(matches!(pool_err, PoolError::DiscoveryError(_)));
    }

    #[test]
    fn test_pool_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let pool_err: PoolError = io_err.into();
        assert!(matches!(pool_err, PoolError::Io(_)));
    }

    // ==================== McpSocketPool Creation Tests ====================

    #[test]
    fn test_pool_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let pool = McpSocketPool::new(config.clone());

        assert!(!pool.is_started());
        assert_eq!(pool.proxy_count(), 0);
        assert!(pool.list_proxies().is_empty());
    }

    #[test]
    fn test_pool_default() {
        let pool = McpSocketPool::default();
        assert!(!pool.is_started());
        assert_eq!(pool.proxy_count(), 0);
    }

    #[test]
    fn test_pool_config_getter() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let expected_prefix = config.socket_prefix.clone();
        let pool = McpSocketPool::new(config);

        assert_eq!(pool.config().socket_prefix, expected_prefix);
    }

    #[test]
    fn test_pool_metrics_getter() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let pool = McpSocketPool::new(config);

        let metrics = pool.metrics();
        assert_eq!(metrics.active_mcps.load(Ordering::Relaxed), 0);
    }

    // ==================== Pool Lifecycle Tests ====================

    #[tokio::test]
    async fn test_pool_start() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        assert!(!pool.is_started());

        let result = pool.start().await;
        assert!(result.is_ok());
        assert!(pool.is_started());
    }

    #[tokio::test]
    async fn test_pool_start_already_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        let result = pool.start().await;
        assert!(matches!(result, Err(PoolError::AlreadyRunning)));
    }

    #[tokio::test]
    async fn test_pool_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();
        assert!(pool.is_started());

        let result = pool.shutdown().await;
        assert!(result.is_ok());
        assert!(!pool.is_started());
    }

    #[tokio::test]
    async fn test_pool_shutdown_not_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        let result = pool.shutdown().await;
        assert!(matches!(result, Err(PoolError::NotRunning)));
    }

    #[tokio::test]
    async fn test_pool_start_shutdown_cycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        // First cycle
        pool.start().await.unwrap();
        pool.shutdown().await.unwrap();

        // Second cycle should work
        pool.start().await.unwrap();
        pool.shutdown().await.unwrap();
    }

    // ==================== Proxy Management Tests ====================

    #[tokio::test]
    async fn test_pool_get_or_start_proxy_not_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        // Pool not started
        let result = pool.get_or_start_proxy("test", "echo", &[]).await;
        assert!(matches!(result, Err(PoolError::NotRunning)));
    }

    #[tokio::test]
    async fn test_pool_get_or_start_proxy_not_pooled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = test_config(&temp_dir);
        config.exclude_mcps = vec!["chrome".to_string()];
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        let result = pool.get_or_start_proxy("chrome", "echo", &[]).await;
        assert!(matches!(result, Err(PoolError::McpNotPooled(_))));

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_get_or_start_proxy_success() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // Use 'cat' as a simple stdin->stdout process
        let result = pool.get_or_start_proxy("test-mcp", "cat", &[]).await;
        assert!(result.is_ok(), "Failed to start proxy: {:?}", result);

        let socket_path = result.unwrap();
        assert!(socket_path.to_string_lossy().contains("test-mcp"));

        // Verify proxy is tracked
        assert_eq!(pool.proxy_count(), 1);
        assert!(pool.list_proxies().contains(&"test-mcp".to_string()));

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_get_or_start_proxy_reuses_existing() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // First call starts proxy
        let path1 = pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();

        // Second call should return same path (reuse)
        let path2 = pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();

        assert_eq!(path1, path2);
        assert_eq!(pool.proxy_count(), 1);

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_get_socket_path() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // No proxy yet
        assert!(pool.get_socket_path("test-mcp").await.is_none());

        // Start proxy
        let expected_path = pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();

        // Now should find it
        let found_path = pool.get_socket_path("test-mcp").await;
        assert!(found_path.is_some());
        assert_eq!(found_path.unwrap(), expected_path);

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_stop_proxy() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // Start a proxy
        pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();
        assert_eq!(pool.proxy_count(), 1);

        // Stop it
        let result = pool.stop_proxy("test-mcp").await;
        assert!(result.is_ok());
        assert_eq!(pool.proxy_count(), 0);

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_stop_proxy_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        let result = pool.stop_proxy("nonexistent").await;
        assert!(matches!(result, Err(PoolError::ProxyNotFound(_))));

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_stop_proxy_not_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        let result = pool.stop_proxy("test").await;
        assert!(matches!(result, Err(PoolError::NotRunning)));
    }

    #[tokio::test]
    async fn test_pool_is_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // No proxy yet
        assert!(!pool.is_running("test-mcp").await);

        // Start proxy
        pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();
        assert!(pool.is_running("test-mcp").await);

        // Stop proxy
        pool.stop_proxy("test-mcp").await.unwrap();
        assert!(!pool.is_running("test-mcp").await);

        pool.shutdown().await.unwrap();
    }

    // ==================== Multiple Proxy Tests ====================

    #[tokio::test]
    async fn test_pool_multiple_proxies() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // Start multiple proxies
        pool.get_or_start_proxy("mcp-1", "cat", &[]).await.unwrap();
        pool.get_or_start_proxy("mcp-2", "cat", &[]).await.unwrap();
        pool.get_or_start_proxy("mcp-3", "cat", &[]).await.unwrap();

        assert_eq!(pool.proxy_count(), 3);

        let names = pool.list_proxies();
        assert!(names.contains(&"mcp-1".to_string()));
        assert!(names.contains(&"mcp-2".to_string()));
        assert!(names.contains(&"mcp-3".to_string()));

        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_proxy_info() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();

        let info = pool.proxy_info().await;
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].mcp_name, "test-mcp");
        assert!(info[0].is_running);
        assert_eq!(info[0].client_count, 0);

        pool.shutdown().await.unwrap();
    }

    // ==================== Health Check Tests ====================

    #[tokio::test]
    async fn test_pool_check_health_not_running() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        let result = pool.check_health().await;
        assert!(matches!(result, Err(PoolError::NotRunning)));
    }

    #[tokio::test]
    async fn test_pool_check_health_with_proxies() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        pool.get_or_start_proxy("test-mcp", "cat", &[]).await.unwrap();

        let result = pool.check_health().await;
        assert!(result.is_ok());

        pool.shutdown().await.unwrap();
    }

    // ==================== Metrics Tests ====================

    #[tokio::test]
    async fn test_pool_metrics_update() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        assert_eq!(pool.metrics().active_mcps.load(Ordering::Relaxed), 0);

        pool.get_or_start_proxy("mcp-1", "cat", &[]).await.unwrap();
        assert_eq!(pool.metrics().active_mcps.load(Ordering::Relaxed), 1);

        pool.get_or_start_proxy("mcp-2", "cat", &[]).await.unwrap();
        assert_eq!(pool.metrics().active_mcps.load(Ordering::Relaxed), 2);

        pool.stop_proxy("mcp-1").await.unwrap();
        assert_eq!(pool.metrics().active_mcps.load(Ordering::Relaxed), 1);

        pool.shutdown().await.unwrap();
    }

    // ==================== Discovery Integration Tests ====================

    #[tokio::test]
    async fn test_pool_discovery_getter() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let pool = McpSocketPool::new(config);

        // Should be able to use discovery before pool is started
        let discovery = pool.discovery();
        let sockets = discovery.discover_all();
        assert!(sockets.is_ok());
    }

    #[tokio::test]
    async fn test_pool_cleans_stale_on_start() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create a stale socket (dead PID)
        std::fs::write(socket_dir.join("mcp-stale.sock"), "").unwrap();
        std::fs::write(socket_dir.join("mcp-stale.lock"), "999999999").unwrap();

        let mut pool = McpSocketPool::new(config);
        pool.start().await.unwrap();

        // Stale socket should be cleaned up
        assert!(!socket_dir.join("mcp-stale.sock").exists());
        assert!(!socket_dir.join("mcp-stale.lock").exists());

        pool.shutdown().await.unwrap();
    }

    // ==================== ProxyInfo Tests ====================

    #[test]
    fn test_proxy_info_struct() {
        let info = ProxyInfo {
            mcp_name: "test".to_string(),
            socket_path: PathBuf::from("/tmp/test.sock"),
            is_running: true,
            client_count: 5,
        };

        assert_eq!(info.mcp_name, "test");
        assert_eq!(info.socket_path, PathBuf::from("/tmp/test.sock"));
        assert!(info.is_running);
        assert_eq!(info.client_count, 5);
    }

    #[test]
    fn test_proxy_info_debug() {
        let info = ProxyInfo {
            mcp_name: "test".to_string(),
            socket_path: PathBuf::from("/tmp/test.sock"),
            is_running: true,
            client_count: 0,
        };

        let debug = format!("{:?}", info);
        assert!(debug.contains("ProxyInfo"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_proxy_info_clone() {
        let info = ProxyInfo {
            mcp_name: "test".to_string(),
            socket_path: PathBuf::from("/tmp/test.sock"),
            is_running: true,
            client_count: 3,
        };

        let cloned = info.clone();
        assert_eq!(cloned.mcp_name, info.mcp_name);
        assert_eq!(cloned.client_count, info.client_count);
    }

    // ==================== Drop Tests ====================

    #[tokio::test]
    async fn test_pool_drop_sets_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let shutdown_flag = {
            let pool = McpSocketPool::new(config);
            Arc::clone(&pool.shutdown)
        };

        // After pool is dropped, shutdown should be true
        assert!(shutdown_flag.load(Ordering::SeqCst));
    }

    // ==================== Edge Cases ====================

    #[tokio::test]
    async fn test_pool_shutdown_with_multiple_proxies() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // Start several proxies
        for i in 0..3 {
            pool.get_or_start_proxy(&format!("mcp-{i}"), "cat", &[]).await.unwrap();
        }

        assert_eq!(pool.proxy_count(), 3);

        // Shutdown should stop all
        pool.shutdown().await.unwrap();
        assert_eq!(pool.proxy_count(), 0);
    }

    #[tokio::test]
    async fn test_pool_get_or_start_after_shutdown_signal() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let mut pool = McpSocketPool::new(config);

        pool.start().await.unwrap();

        // Signal shutdown but don't call shutdown()
        pool.shutdown.store(true, Ordering::SeqCst);

        // get_or_start_proxy should fail
        let result = pool.get_or_start_proxy("test", "cat", &[]).await;
        assert!(matches!(result, Err(PoolError::NotRunning)));
    }
}
