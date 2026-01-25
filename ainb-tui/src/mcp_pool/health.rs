// ABOUTME: Health monitoring for MCP socket pool with configurable check intervals
// ABOUTME: Background task that monitors proxy health, triggers restarts, and cleans stale sockets

//! Health monitoring for MCP socket pool.
//!
//! The `HealthMonitor` runs as a background task that:
//! - Checks MCP process health at configurable intervals (default 10s)
//! - Triggers restarts on failure via `SocketProxy::check_health()`
//! - Updates circuit breaker state based on health check results
//! - Cleans up stale resources via `SocketDiscovery::cleanup_stale()`
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    HealthMonitor                        │
//! │                                                         │
//! │  ┌──────────────┐    ┌──────────────┐                  │
//! │  │ Check Loop   │───▶│ Proxy Health │                  │
//! │  │ (interval)   │    │   Checks     │                  │
//! │  └──────────────┘    └──────────────┘                  │
//! │          │                   │                         │
//! │          │                   ▼                         │
//! │          │           ┌──────────────┐                  │
//! │          │           │ Restart on   │                  │
//! │          │           │   Failure    │                  │
//! │          │           └──────────────┘                  │
//! │          │                                              │
//! │          ▼                                              │
//! │  ┌──────────────┐                                      │
//! │  │ Stale Socket │                                      │
//! │  │   Cleanup    │                                      │
//! │  └──────────────┘                                      │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use std::time::Duration;
//! use mcp_pool::health::{HealthMonitor, HealthStatus};
//!
//! let mut monitor = HealthMonitor::new(Duration::from_secs(10));
//!
//! // Start monitoring with proxies map and socket directory
//! monitor.start(proxies, socket_dir, pool_config);
//!
//! // Later, stop gracefully
//! monitor.stop().await;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::config::PoolConfig;
use super::discovery::SocketDiscovery;
use super::socket_proxy::SocketProxy;

// === Health Status ===

/// Overall health status of the proxy pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// All proxies are healthy.
    Healthy,

    /// Some proxies are unhealthy but recoverable.
    Degraded {
        /// Names of unhealthy proxies.
        unhealthy: Vec<String>,
    },

    /// Some proxies have permanently failed.
    Critical {
        /// Names of permanently failed proxies.
        failed: Vec<String>,
    },
}

impl HealthStatus {
    /// Check if the pool is fully healthy.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if the pool has any critical failures.
    #[must_use]
    pub const fn is_critical(&self) -> bool {
        matches!(self, Self::Critical { .. })
    }

    /// Get the list of unhealthy proxy names (empty if healthy).
    #[must_use]
    pub fn unhealthy_proxies(&self) -> Vec<String> {
        match self {
            Self::Healthy => vec![],
            Self::Degraded { unhealthy } => unhealthy.clone(),
            Self::Critical { failed } => failed.clone(),
        }
    }
}

// === Individual Proxy Health Result ===

/// Result of a single proxy health check.
#[derive(Debug, Clone)]
pub struct ProxyHealthResult {
    /// Name of the MCP proxy.
    pub mcp_name: String,

    /// Whether the proxy is healthy.
    pub is_healthy: bool,

    /// Whether the proxy was restarted during this check.
    pub was_restarted: bool,

    /// Whether the proxy has permanently failed.
    pub permanently_failed: bool,

    /// Error message if unhealthy.
    pub error_message: Option<String>,
}

// === Health Monitor ===

/// Background health monitor for MCP socket pool.
///
/// Periodically checks the health of all proxies and performs cleanup
/// of stale socket resources.
pub struct HealthMonitor {
    /// Interval between health checks.
    interval: Duration,

    /// Shutdown flag for graceful termination.
    shutdown: Arc<AtomicBool>,

    /// Handle to the background monitoring task.
    handle: Option<JoinHandle<()>>,

    /// Number of health check cycles between stale socket cleanup runs.
    /// Default: 6 (cleanup every 60s with 10s interval).
    cleanup_interval_cycles: u32,
}

impl HealthMonitor {
    /// Create a new health monitor with the specified check interval.
    ///
    /// # Arguments
    ///
    /// * `interval` - Duration between health checks (default 10s recommended)
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: None,
            cleanup_interval_cycles: 6,
        }
    }

    /// Create a health monitor from pool configuration.
    #[must_use]
    pub fn from_config(config: &PoolConfig) -> Self {
        Self::new(config.health_check_interval)
    }

    /// Set the number of health check cycles between stale cleanup runs.
    pub const fn set_cleanup_interval_cycles(&mut self, cycles: u32) {
        self.cleanup_interval_cycles = cycles;
    }

    /// Start the health monitoring background task.
    ///
    /// # Arguments
    ///
    /// * `proxies` - Shared map of MCP name to proxy instances
    /// * `socket_dir` - Directory containing socket files for cleanup
    /// * `config` - Pool configuration for socket discovery
    pub fn start(
        &mut self,
        proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>>,
        socket_dir: PathBuf,
        config: PoolConfig,
    ) {
        // Reset shutdown flag
        self.shutdown.store(false, Ordering::SeqCst);

        let shutdown = Arc::clone(&self.shutdown);
        let interval = self.interval;
        let cleanup_cycles = self.cleanup_interval_cycles;

        let handle = tokio::spawn(async move {
            Self::health_check_loop(proxies, socket_dir, config, interval, cleanup_cycles, shutdown)
                .await;
        });

        self.handle = Some(handle);
        info!(interval_ms = %self.interval.as_millis(), "Health monitor started");
    }

    /// Stop the health monitoring gracefully.
    ///
    /// Signals the background task to stop and waits for it to complete.
    pub async fn stop(&mut self) {
        // Signal shutdown
        self.shutdown.store(true, Ordering::SeqCst);

        // Wait for task to complete
        if let Some(handle) = self.handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        info!("Health monitor stopped");
    }

    /// Check if the monitor is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.handle.is_some() && !self.shutdown.load(Ordering::SeqCst)
    }

    /// Perform a single health check iteration on all proxies.
    ///
    /// This is exposed for testing purposes and manual health checks.
    ///
    /// # Arguments
    ///
    /// * `proxies` - Map of MCP name to proxy instances
    ///
    /// # Returns
    ///
    /// Vector of health check results for each proxy.
    pub async fn check_all_proxies(
        proxies: &RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>,
    ) -> Vec<ProxyHealthResult> {
        let mut results = Vec::new();

        // Get a snapshot of proxy names to avoid holding the lock during checks
        let proxy_names: Vec<String> = {
            let proxies_guard = proxies.read().await;
            proxies_guard.keys().cloned().collect()
        };

        for mcp_name in proxy_names {
            let result = Self::check_single_proxy(proxies, &mcp_name).await;
            results.push(result);
        }

        results
    }

    /// Check the health of a single proxy.
    async fn check_single_proxy(
        proxies: &RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>,
        mcp_name: &str,
    ) -> ProxyHealthResult {
        // Get the proxy reference
        let proxy_arc = {
            let proxies_guard = proxies.read().await;
            proxies_guard.get(mcp_name).cloned()
        };

        let Some(proxy_arc) = proxy_arc else {
            return ProxyHealthResult {
                mcp_name: mcp_name.to_string(),
                is_healthy: false,
                was_restarted: false,
                permanently_failed: true,
                error_message: Some("Proxy not found".to_string()),
            };
        };

        // Perform health check (requires mutable access)
        // Store results in local variables to allow early drop of the write guard
        let (health_result, is_running_after) = {
            let mut proxy = proxy_arc.write().await;
            let result = proxy.check_health().await;
            let running = proxy.is_running();
            drop(proxy); // Explicitly drop the write guard before returning from block
            (result, running)
        };

        match health_result {
            Ok(true) => {
                debug!(mcp = %mcp_name, "Proxy health check passed");
                ProxyHealthResult {
                    mcp_name: mcp_name.to_string(),
                    is_healthy: true,
                    was_restarted: false,
                    permanently_failed: false,
                    error_message: None,
                }
            }
            Ok(false) => {
                // Proxy returned false - permanently failed
                warn!(mcp = %mcp_name, "Proxy permanently failed");
                ProxyHealthResult {
                    mcp_name: mcp_name.to_string(),
                    is_healthy: false,
                    was_restarted: false,
                    permanently_failed: true,
                    error_message: Some("MCP process permanently failed".to_string()),
                }
            }
            Err(e) => {
                // Health check failed but proxy may have been restarted
                error!(mcp = %mcp_name, error = %e, "Proxy health check error");
                ProxyHealthResult {
                    mcp_name: mcp_name.to_string(),
                    is_healthy: is_running_after,
                    was_restarted: is_running_after,
                    permanently_failed: !is_running_after,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    /// Aggregate individual proxy results into overall health status.
    #[must_use]
    pub fn aggregate_health_status(results: &[ProxyHealthResult]) -> HealthStatus {
        let mut unhealthy = Vec::new();
        let mut failed = Vec::new();

        for result in results {
            if result.permanently_failed {
                failed.push(result.mcp_name.clone());
            } else if !result.is_healthy {
                unhealthy.push(result.mcp_name.clone());
            }
        }

        if !failed.is_empty() {
            HealthStatus::Critical { failed }
        } else if !unhealthy.is_empty() {
            HealthStatus::Degraded { unhealthy }
        } else {
            HealthStatus::Healthy
        }
    }

    /// The main health check loop running in the background.
    async fn health_check_loop(
        proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>>,
        socket_dir: PathBuf,
        config: PoolConfig,
        interval: Duration,
        cleanup_cycles: u32,
        shutdown: Arc<AtomicBool>,
    ) {
        let mut cycle_count: u32 = 0;
        let discovery = SocketDiscovery::new(config);

        debug!("Health check loop started");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                debug!("Health check loop received shutdown signal");
                break;
            }

            // Wait for the interval
            tokio::time::sleep(interval).await;

            // Check shutdown again after sleep
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // Perform health checks on all proxies
            let results = Self::check_all_proxies(&proxies).await;
            let status = Self::aggregate_health_status(&results);

            // Log overall status
            match &status {
                HealthStatus::Healthy => {
                    debug!(proxy_count = results.len(), "All proxies healthy");
                }
                HealthStatus::Degraded { unhealthy } => {
                    warn!(
                        unhealthy_count = unhealthy.len(),
                        unhealthy = ?unhealthy,
                        "Some proxies degraded"
                    );
                }
                HealthStatus::Critical { failed } => {
                    error!(
                        failed_count = failed.len(),
                        failed = ?failed,
                        "Critical: proxies permanently failed"
                    );
                }
            }

            // Log restart events
            for result in &results {
                if result.was_restarted {
                    info!(mcp = %result.mcp_name, "Proxy was restarted");
                }
            }

            // Periodic stale socket cleanup
            cycle_count = cycle_count.wrapping_add(1);
            if cycle_count % cleanup_cycles == 0 {
                Self::cleanup_stale_sockets(&discovery, &socket_dir);
            }
        }

        debug!("Health check loop ended");
    }

    /// Clean up stale sockets from dead processes.
    fn cleanup_stale_sockets(discovery: &SocketDiscovery, socket_dir: &Path) {
        debug!(socket_dir = %socket_dir.display(), "Running stale socket cleanup");

        match discovery.cleanup_stale() {
            Ok(count) if count > 0 => {
                info!(cleaned = count, "Cleaned up stale sockets");
            }
            Ok(_) => {
                debug!("No stale sockets to clean up");
            }
            Err(e) => {
                warn!(error = %e, "Failed to clean up stale sockets");
            }
        }
    }
}

impl Drop for HealthMonitor {
    fn drop(&mut self) {
        // Signal shutdown on drop
        self.shutdown.store(true, Ordering::SeqCst);
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
        let socket_dir = temp_dir.path().join(format!("health_sockets_{counter}"));
        std::fs::create_dir_all(&socket_dir).unwrap();

        PoolConfig {
            socket_dir: Some(socket_dir),
            socket_prefix: "mcp-".to_string(),
            health_check_interval: Duration::from_millis(100),
            ..PoolConfig::default()
        }
    }

    // ==================== HealthStatus Tests ====================

    #[test]
    fn test_health_status_healthy() {
        let status = HealthStatus::Healthy;
        assert!(status.is_healthy());
        assert!(!status.is_critical());
        assert!(status.unhealthy_proxies().is_empty());
    }

    #[test]
    fn test_health_status_degraded() {
        let status = HealthStatus::Degraded {
            unhealthy: vec!["mcp1".to_string(), "mcp2".to_string()],
        };
        assert!(!status.is_healthy());
        assert!(!status.is_critical());
        assert_eq!(status.unhealthy_proxies().len(), 2);
    }

    #[test]
    fn test_health_status_critical() {
        let status = HealthStatus::Critical {
            failed: vec!["mcp1".to_string()],
        };
        assert!(!status.is_healthy());
        assert!(status.is_critical());
        assert_eq!(status.unhealthy_proxies().len(), 1);
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(
            HealthStatus::Healthy,
            HealthStatus::Degraded {
                unhealthy: vec![]
            }
        );
    }

    // ==================== ProxyHealthResult Tests ====================

    #[test]
    fn test_proxy_health_result_healthy() {
        let result = ProxyHealthResult {
            mcp_name: "test".to_string(),
            is_healthy: true,
            was_restarted: false,
            permanently_failed: false,
            error_message: None,
        };
        assert!(result.is_healthy);
        assert!(!result.was_restarted);
        assert!(!result.permanently_failed);
    }

    #[test]
    fn test_proxy_health_result_restarted() {
        let result = ProxyHealthResult {
            mcp_name: "test".to_string(),
            is_healthy: true,
            was_restarted: true,
            permanently_failed: false,
            error_message: None,
        };
        assert!(result.is_healthy);
        assert!(result.was_restarted);
    }

    #[test]
    fn test_proxy_health_result_failed() {
        let result = ProxyHealthResult {
            mcp_name: "test".to_string(),
            is_healthy: false,
            was_restarted: false,
            permanently_failed: true,
            error_message: Some("Process died".to_string()),
        };
        assert!(!result.is_healthy);
        assert!(result.permanently_failed);
        assert!(result.error_message.is_some());
    }

    // ==================== HealthMonitor Creation Tests ====================

    #[test]
    fn test_health_monitor_new() {
        let monitor = HealthMonitor::new(Duration::from_secs(10));
        assert_eq!(monitor.interval, Duration::from_secs(10));
        assert!(!monitor.is_running());
        assert_eq!(monitor.cleanup_interval_cycles, 6);
    }

    #[test]
    fn test_health_monitor_from_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let monitor = HealthMonitor::from_config(&config);
        assert_eq!(monitor.interval, Duration::from_millis(100));
    }

    #[test]
    fn test_health_monitor_set_cleanup_cycles() {
        let mut monitor = HealthMonitor::new(Duration::from_secs(10));
        monitor.set_cleanup_interval_cycles(12);
        assert_eq!(monitor.cleanup_interval_cycles, 12);
    }

    // ==================== Aggregate Status Tests ====================

    #[test]
    fn test_aggregate_all_healthy() {
        let results = vec![
            ProxyHealthResult {
                mcp_name: "mcp1".to_string(),
                is_healthy: true,
                was_restarted: false,
                permanently_failed: false,
                error_message: None,
            },
            ProxyHealthResult {
                mcp_name: "mcp2".to_string(),
                is_healthy: true,
                was_restarted: false,
                permanently_failed: false,
                error_message: None,
            },
        ];

        let status = HealthMonitor::aggregate_health_status(&results);
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_aggregate_some_unhealthy() {
        let results = vec![
            ProxyHealthResult {
                mcp_name: "mcp1".to_string(),
                is_healthy: true,
                was_restarted: false,
                permanently_failed: false,
                error_message: None,
            },
            ProxyHealthResult {
                mcp_name: "mcp2".to_string(),
                is_healthy: false,
                was_restarted: false,
                permanently_failed: false,
                error_message: Some("Error".to_string()),
            },
        ];

        let status = HealthMonitor::aggregate_health_status(&results);
        match status {
            HealthStatus::Degraded { unhealthy } => {
                assert_eq!(unhealthy.len(), 1);
                assert_eq!(unhealthy[0], "mcp2");
            }
            _ => panic!("Expected Degraded status"),
        }
    }

    #[test]
    fn test_aggregate_some_failed() {
        let results = vec![
            ProxyHealthResult {
                mcp_name: "mcp1".to_string(),
                is_healthy: false,
                was_restarted: false,
                permanently_failed: true,
                error_message: Some("Permanently failed".to_string()),
            },
            ProxyHealthResult {
                mcp_name: "mcp2".to_string(),
                is_healthy: true,
                was_restarted: false,
                permanently_failed: false,
                error_message: None,
            },
        ];

        let status = HealthMonitor::aggregate_health_status(&results);
        match status {
            HealthStatus::Critical { failed } => {
                assert_eq!(failed.len(), 1);
                assert_eq!(failed[0], "mcp1");
            }
            _ => panic!("Expected Critical status"),
        }
    }

    #[test]
    fn test_aggregate_empty_results() {
        let results: Vec<ProxyHealthResult> = vec![];
        let status = HealthMonitor::aggregate_health_status(&results);
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_aggregate_failed_takes_priority() {
        // If some are failed and some are just unhealthy, Critical takes priority
        let results = vec![
            ProxyHealthResult {
                mcp_name: "mcp1".to_string(),
                is_healthy: false,
                was_restarted: false,
                permanently_failed: true,
                error_message: Some("Permanently failed".to_string()),
            },
            ProxyHealthResult {
                mcp_name: "mcp2".to_string(),
                is_healthy: false,
                was_restarted: false,
                permanently_failed: false,
                error_message: Some("Temporarily unhealthy".to_string()),
            },
        ];

        let status = HealthMonitor::aggregate_health_status(&results);
        assert!(matches!(status, HealthStatus::Critical { .. }));
    }

    // ==================== Start/Stop Tests ====================

    #[tokio::test]
    async fn test_health_monitor_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        let proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut monitor = HealthMonitor::new(Duration::from_millis(50));

        // Start monitoring
        monitor.start(proxies, socket_dir, config);
        assert!(monitor.is_running());

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop
        monitor.stop().await;
        assert!(!monitor.is_running());
    }

    #[tokio::test]
    async fn test_health_monitor_stop_without_start() {
        let mut monitor = HealthMonitor::new(Duration::from_secs(10));

        // Should not panic
        monitor.stop().await;
        assert!(!monitor.is_running());
    }

    #[tokio::test]
    async fn test_health_monitor_multiple_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        let proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut monitor = HealthMonitor::new(Duration::from_millis(50));

        // First start/stop cycle
        monitor.start(Arc::clone(&proxies), socket_dir.clone(), config.clone());
        assert!(monitor.is_running());
        monitor.stop().await;
        assert!(!monitor.is_running());

        // Second start/stop cycle should work
        monitor.start(proxies, socket_dir, config);
        assert!(monitor.is_running());
        monitor.stop().await;
        assert!(!monitor.is_running());
    }

    // ==================== Check All Proxies Tests ====================

    #[tokio::test]
    async fn test_check_all_proxies_empty() {
        let proxies: RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>> =
            RwLock::new(HashMap::new());

        let results = HealthMonitor::check_all_proxies(&proxies).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_check_all_proxies_with_proxy() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Create a proxy using 'cat' as a simple echo-like process
        let proxy = SocketProxy::new(
            "test-mcp".to_string(),
            "cat".to_string(),
            vec![],
            config.clone(),
        );

        let proxies: RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>> = RwLock::new(HashMap::new());
        {
            let mut guard = proxies.write().await;
            guard.insert("test-mcp".to_string(), Arc::new(RwLock::new(proxy)));
        }

        let results = HealthMonitor::check_all_proxies(&proxies).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].mcp_name, "test-mcp");
        // Proxy is not started, so health check will show it as not healthy
    }

    #[tokio::test]
    async fn test_check_all_proxies_proxy_not_found() {
        let proxies: RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>> =
            RwLock::new(HashMap::new());

        // This shouldn't happen in practice but test the edge case
        let results = HealthMonitor::check_all_proxies(&proxies).await;
        assert!(results.is_empty());
    }

    // ==================== Drop Tests ====================

    #[test]
    fn test_health_monitor_drop_sets_shutdown() {
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        {
            let mut monitor = HealthMonitor::new(Duration::from_secs(10));
            // Override the shutdown flag for testing
            monitor.shutdown = Arc::clone(&shutdown_flag);
            // monitor goes out of scope and Drop is called
        }
        assert!(shutdown_flag.load(Ordering::SeqCst));
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_health_check_loop_runs() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        let proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut monitor = HealthMonitor::new(Duration::from_millis(20));

        // Start monitoring
        monitor.start(Arc::clone(&proxies), socket_dir, config);

        // Let several health check cycles run
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify still running
        assert!(monitor.is_running());

        // Clean stop
        monitor.stop().await;
    }

    #[tokio::test]
    async fn test_health_check_with_started_proxy() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Create and start a proxy
        let mut proxy = SocketProxy::new(
            "test-echo".to_string(),
            "cat".to_string(),
            vec![],
            config.clone(),
        );

        // Start the proxy
        let start_result = proxy.start().await;
        assert!(start_result.is_ok(), "Failed to start proxy: {:?}", start_result);
        assert!(proxy.is_running());

        // Add to proxies map
        let proxies: RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>> = RwLock::new(HashMap::new());
        {
            let mut guard = proxies.write().await;
            guard.insert("test-echo".to_string(), Arc::new(RwLock::new(proxy)));
        }

        // Check health
        let results = HealthMonitor::check_all_proxies(&proxies).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].mcp_name, "test-echo");
        // The proxy should be healthy since it's running
        assert!(results[0].is_healthy);
        assert!(!results[0].permanently_failed);

        // Stop the proxy
        {
            let guard = proxies.read().await;
            if let Some(proxy_arc) = guard.get("test-echo") {
                let mut proxy = proxy_arc.write().await;
                let _ = proxy.stop().await;
            }
        }
    }

    #[tokio::test]
    async fn test_cleanup_happens_on_schedule() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let socket_dir = config.get_socket_dir().unwrap();

        // Create a stale socket file (dead PID)
        std::fs::write(socket_dir.join("mcp-stale.sock"), "").unwrap();
        std::fs::write(socket_dir.join("mcp-stale.lock"), "999999999").unwrap();

        let proxies: Arc<RwLock<HashMap<String, Arc<RwLock<SocketProxy>>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut monitor = HealthMonitor::new(Duration::from_millis(20));
        // Set cleanup to happen every cycle for faster testing
        monitor.set_cleanup_interval_cycles(1);

        // Start monitoring
        monitor.start(proxies, socket_dir.clone(), config);

        // Wait for a cleanup cycle
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify stale socket was cleaned up
        assert!(!socket_dir.join("mcp-stale.sock").exists());
        assert!(!socket_dir.join("mcp-stale.lock").exists());

        monitor.stop().await;
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_health_status_debug_format() {
        let status = HealthStatus::Degraded {
            unhealthy: vec!["mcp1".to_string()],
        };
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("Degraded"));
        assert!(debug_str.contains("mcp1"));
    }

    #[test]
    fn test_proxy_health_result_debug_format() {
        let result = ProxyHealthResult {
            mcp_name: "test".to_string(),
            is_healthy: false,
            was_restarted: true,
            permanently_failed: false,
            error_message: Some("Test error".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("Test error"));
    }

    #[test]
    fn test_health_status_clone() {
        let status = HealthStatus::Critical {
            failed: vec!["mcp1".to_string()],
        };
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_proxy_health_result_clone() {
        let result = ProxyHealthResult {
            mcp_name: "test".to_string(),
            is_healthy: true,
            was_restarted: false,
            permanently_failed: false,
            error_message: None,
        };
        let cloned = result.clone();
        assert_eq!(result.mcp_name, cloned.mcp_name);
        assert_eq!(result.is_healthy, cloned.is_healthy);
    }
}
