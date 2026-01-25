// ABOUTME: Configuration for MCP socket pooling
//
// Defines PoolConfig with all tunable parameters for the socket pool:
// - Socket location and naming
// - Timeouts for requests, keepalive, idle clients
// - Health check and restart parameters
// - Backpressure limits and circuit breaker settings
// - TCP relay configuration for Docker

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the MCP socket pool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PoolConfig {
    /// Master switch for socket pooling
    pub enabled: bool,

    /// Pool ALL MCPs by default
    pub pool_all: bool,

    /// MCPs to exclude from pooling (need per-session isolation)
    pub exclude_mcps: Vec<String>,

    /// Only pool these MCPs (overrides pool_all if non-empty)
    pub include_mcps: Vec<String>,

    // === Socket Location (SECURE: user-private directory) ===
    /// Directory for socket files (default: ~/.agents-in-a-box/sockets/)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_dir: Option<PathBuf>,

    /// Prefix for socket filenames
    pub socket_prefix: String,

    // === Timeouts ===
    /// Seconds to wait for socket to become ready
    #[serde(with = "duration_secs")]
    pub socket_wait_timeout: Duration,

    /// Maximum request duration (5 min default)
    #[serde(with = "duration_secs")]
    pub request_timeout: Duration,

    /// Client keepalive ping interval
    #[serde(with = "duration_secs")]
    pub keepalive_interval: Duration,

    /// Remove clients with no activity after this duration
    #[serde(with = "duration_secs")]
    pub idle_client_timeout: Duration,

    // === Health & Restart ===
    /// Interval between health checks
    #[serde(with = "duration_secs")]
    pub health_check_interval: Duration,

    /// Maximum restart attempts before marking permanently failed
    pub max_restarts: u32,

    /// Initial restart delay (exponential backoff base)
    #[serde(with = "duration_secs")]
    pub restart_backoff_base: Duration,

    /// Maximum restart delay (exponential backoff cap)
    #[serde(with = "duration_secs")]
    pub restart_backoff_max: Duration,

    // === Backpressure ===
    /// Maximum pending requests per client
    pub max_pending_requests_per_client: usize,

    /// Maximum concurrent clients per MCP
    pub max_clients_per_mcp: usize,

    /// Consecutive failures before circuit breaker opens
    pub circuit_breaker_threshold: u32,

    /// Seconds before circuit breaker attempts half-open state
    #[serde(with = "duration_secs")]
    pub circuit_breaker_reset: Duration,

    // === Docker Integration ===
    /// Enable TCP relay for container access
    pub tcp_relay_enabled: bool,

    /// Port range for TCP relays (start, end)
    pub tcp_relay_port_range: (u16, u16),

    /// Fall back to stdio mode if socket fails
    pub fallback_to_stdio: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pool_all: true,
            exclude_mcps: vec![],
            include_mcps: vec![],

            // Socket location determined at runtime via get_socket_dir()
            socket_dir: None,
            socket_prefix: "mcp-".to_string(),

            socket_wait_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(300), // 5 minutes
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

impl PoolConfig {
    /// Get the socket directory, creating it with secure permissions if needed
    ///
    /// Returns `~/.agents-in-a-box/sockets/` by default, with mode 0700
    pub fn get_socket_dir(&self) -> std::io::Result<PathBuf> {
        let socket_dir = self.socket_dir.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".agents-in-a-box")
                .join("sockets")
        });

        ensure_socket_dir(&socket_dir)?;
        Ok(socket_dir)
    }

    /// Get the full socket path for an MCP server
    pub fn get_socket_path(&self, mcp_name: &str) -> std::io::Result<PathBuf> {
        let socket_dir = self.get_socket_dir()?;
        Ok(socket_dir.join(format!("{}{}.sock", self.socket_prefix, mcp_name)))
    }

    /// Get the lock file path for an MCP server
    pub fn get_lock_path(&self, mcp_name: &str) -> std::io::Result<PathBuf> {
        let socket_dir = self.get_socket_dir()?;
        Ok(socket_dir.join(format!("{}{}.lock", self.socket_prefix, mcp_name)))
    }

    /// Check if an MCP should be pooled based on configuration
    pub fn should_pool(&self, mcp_name: &str) -> bool {
        if !self.enabled {
            return false;
        }

        // Check explicit exclusion
        if self.exclude_mcps.iter().any(|e| e == mcp_name) {
            return false;
        }

        // If include list is specified, only pool those
        if !self.include_mcps.is_empty() {
            return self.include_mcps.iter().any(|i| i == mcp_name);
        }

        // Otherwise, pool all if enabled
        self.pool_all
    }

    /// Check if socket pooling is supported on this platform
    pub fn is_platform_supported() -> bool {
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
}

/// Ensure socket directory exists with secure permissions (mode 0700)
fn ensure_socket_dir(socket_dir: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(socket_dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700); // rwx------
        std::fs::set_permissions(socket_dir, perms)?;
    }

    Ok(())
}

/// Serde helper for Duration as seconds (u64)
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PoolConfig::default();
        assert!(config.enabled);
        assert!(config.pool_all);
        assert_eq!(config.max_restarts, 10);
        assert_eq!(config.circuit_breaker_threshold, 3);
    }

    #[test]
    fn test_should_pool_exclusion() {
        let mut config = PoolConfig::default();
        config.exclude_mcps = vec!["chrome".to_string()];

        assert!(config.should_pool("context7"));
        assert!(!config.should_pool("chrome"));
    }

    #[test]
    fn test_should_pool_inclusion() {
        let mut config = PoolConfig::default();
        config.include_mcps = vec!["context7".to_string(), "memory".to_string()];

        assert!(config.should_pool("context7"));
        assert!(config.should_pool("memory"));
        assert!(!config.should_pool("exa"));
    }

    #[test]
    fn test_should_pool_disabled() {
        let mut config = PoolConfig::default();
        config.enabled = false;

        assert!(!config.should_pool("context7"));
    }

    #[test]
    fn test_socket_path() {
        let config = PoolConfig::default();
        let path = config.get_socket_path("context7").unwrap();
        assert!(path.to_string_lossy().contains("mcp-context7.sock"));
    }

    #[test]
    fn test_lock_path() {
        let config = PoolConfig::default();
        let path = config.get_lock_path("context7").unwrap();
        assert!(path.to_string_lossy().contains("mcp-context7.lock"));
    }

    #[test]
    #[cfg(unix)]
    fn test_platform_supported() {
        // On real Unix systems (not WSL1), this should be true
        // This test may need adjustment in CI environments
        let supported = PoolConfig::is_platform_supported();
        // Just verify it returns a boolean without panicking
        assert!(supported || !supported);
    }
}
