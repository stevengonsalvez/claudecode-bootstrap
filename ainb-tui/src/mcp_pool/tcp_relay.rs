// ABOUTME: TCP relay for Docker container access to MCP Unix sockets
// ABOUTME: Bridges TCP connections to Unix sockets maintaining container isolation

//! TCP relay for Docker container MCP access.
//!
//! Provides a TCP bridge to Unix sockets so Docker containers can access MCP
//! servers without mounting the host socket, maintaining container isolation.
//!
//! # Docker Usage
//!
//! Containers connect via:
//! ```bash
//! socat TCP:host.docker.internal:PORT STDIO
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                       Docker Container                          │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │ socat TCP:host.docker.internal:19123 STDIO               │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼ TCP connection
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         TcpRelay                                │
//! │  ┌──────────────────┐    ┌──────────────────────────────────┐  │
//! │  │  TCP Listener    │ ──▶│  Bidirectional Stream Copy       │  │
//! │  │  (port 19123)    │    │  (tokio::io::copy_bidirectional) │  │
//! │  └──────────────────┘    └──────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼ Unix socket
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Unix Domain Socket                           │
//! │  ~/.agents-in-a-box/sockets/mcp-context7.sock                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use std::path::PathBuf;
//! use mcp_pool::tcp_relay::TcpRelay;
//!
//! let socket_path = PathBuf::from("/tmp/mcp.sock");
//! let mut relay = TcpRelay::new(socket_path, (19000, 19999))?;
//!
//! let port = relay.start().await?;
//! println!("Relay listening on port {}", port);
//!
//! // Container can now connect via: socat TCP:host.docker.internal:{port} STDIO
//!
//! relay.stop().await?;
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;

use thiserror::Error;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Default port range for TCP relays
pub const DEFAULT_PORT_RANGE: (u16, u16) = (19000, 19999);

/// Errors that can occur during TCP relay operation
#[derive(Debug, Error)]
pub enum RelayError {
    /// No available port in the configured range
    #[error("No available port in range {range:?}")]
    NoAvailablePort { range: (u16, u16) },

    /// Failed to bind to TCP port
    #[error("Failed to bind TCP listener: {0}")]
    BindFailed(#[source] std::io::Error),

    /// Failed to connect to Unix socket
    #[error("Failed to connect to Unix socket: {0}")]
    SocketConnectionFailed(#[source] std::io::Error),

    /// Relay is not running when expected
    #[error("Relay is not running")]
    NotRunning,

    /// Relay is already running
    #[error("Relay is already running")]
    AlreadyRunning,

    /// Invalid port range specified
    #[error("Invalid port range: start ({start}) must be less than or equal to end ({end})")]
    InvalidPortRange { start: u16, end: u16 },
}

/// TCP relay that bridges TCP connections to a Unix domain socket.
///
/// Allows Docker containers to access MCP servers without mounting the
/// host socket file, maintaining better container isolation.
pub struct TcpRelay {
    /// Path to the Unix socket to relay traffic to
    socket_path: PathBuf,

    /// Port range for binding (start, end inclusive)
    port_range: (u16, u16),

    /// Currently bound port (0 if not running)
    bound_port: Arc<AtomicU16>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Background listener task handle
    listener_handle: Option<JoinHandle<()>>,
}

impl TcpRelay {
    /// Create a new TCP relay for the given Unix socket.
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path to the Unix domain socket to relay to
    /// * `port_range` - Range of ports to try binding (start, end inclusive)
    ///
    /// # Errors
    ///
    /// Returns `InvalidPortRange` if start > end.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let relay = TcpRelay::new(PathBuf::from("/tmp/mcp.sock"), (19000, 19999))?;
    /// ```
    pub fn new(socket_path: PathBuf, port_range: (u16, u16)) -> Result<Self, RelayError> {
        if port_range.0 > port_range.1 {
            return Err(RelayError::InvalidPortRange {
                start: port_range.0,
                end: port_range.1,
            });
        }

        Ok(Self {
            socket_path,
            port_range,
            bound_port: Arc::new(AtomicU16::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
            listener_handle: None,
        })
    }

    /// Start the TCP relay and return the bound port.
    ///
    /// Tries to bind to a random port in the configured range. If that fails,
    /// falls back to sequential port scanning from the start of the range.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Relay is already running
    /// - No port in the range is available
    ///
    /// # Returns
    ///
    /// The TCP port number that was successfully bound.
    pub async fn start(&mut self) -> Result<u16, RelayError> {
        if self.is_running() {
            return Err(RelayError::AlreadyRunning);
        }

        // Reset shutdown flag
        self.shutdown.store(false, Ordering::SeqCst);

        // Try to find and bind to an available port
        let listener = self.find_available_port().await?;
        let port = listener
            .local_addr()
            .map_err(|e| RelayError::BindFailed(e))?
            .port();

        self.bound_port.store(port, Ordering::SeqCst);

        info!(
            port = port,
            socket = %self.socket_path.display(),
            "TCP relay started"
        );

        // Start the listener loop
        let socket_path = self.socket_path.clone();
        let shutdown = Arc::clone(&self.shutdown);
        let bound_port = Arc::clone(&self.bound_port);

        let handle = tokio::spawn(async move {
            Self::accept_loop(listener, socket_path, shutdown, bound_port).await;
        });

        self.listener_handle = Some(handle);

        Ok(port)
    }

    /// Stop the TCP relay gracefully.
    ///
    /// # Errors
    ///
    /// Returns `NotRunning` if the relay is not currently running.
    pub async fn stop(&mut self) -> Result<(), RelayError> {
        if !self.is_running() {
            return Err(RelayError::NotRunning);
        }

        info!(port = self.port(), "Stopping TCP relay");

        // Signal shutdown
        self.shutdown.store(true, Ordering::SeqCst);

        // Wait for listener task to finish
        if let Some(handle) = self.listener_handle.take() {
            // Give it a reasonable timeout
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        }

        // Reset bound port
        self.bound_port.store(0, Ordering::SeqCst);

        info!("TCP relay stopped");

        Ok(())
    }

    /// Get the currently bound port, if running.
    ///
    /// Returns `None` if the relay is not running.
    pub fn port(&self) -> Option<u16> {
        let port = self.bound_port.load(Ordering::SeqCst);
        if port == 0 {
            None
        } else {
            Some(port)
        }
    }

    /// Check if the relay is currently running.
    pub fn is_running(&self) -> bool {
        self.bound_port.load(Ordering::SeqCst) != 0 && !self.shutdown.load(Ordering::SeqCst)
    }

    /// Get the Unix socket path this relay forwards to.
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Get the configured port range.
    pub fn port_range(&self) -> (u16, u16) {
        self.port_range
    }

    /// Find an available port and bind to it.
    ///
    /// Strategy:
    /// 1. Try a pseudo-random port in the range
    /// 2. If that fails, scan sequentially from start of range
    async fn find_available_port(&self) -> Result<TcpListener, RelayError> {
        let (start, end) = self.port_range;
        let range_size = (end - start + 1) as usize;

        // Generate a pseudo-random starting point using current time
        // (avoids needing the rand crate)
        let random_offset = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            % range_size as u128) as u16;

        let random_port = start + random_offset;

        // Try the random port first
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", random_port)).await {
            debug!(port = random_port, "Bound to random port");
            return Ok(listener);
        }

        // Fall back to sequential scan
        debug!(
            start = start,
            end = end,
            "Random port failed, scanning sequentially"
        );

        for port in start..=end {
            match TcpListener::bind(("127.0.0.1", port)).await {
                Ok(listener) => {
                    debug!(port = port, "Bound to port via sequential scan");
                    return Ok(listener);
                }
                Err(e) => {
                    // Port in use or permission denied, try next
                    if port == end {
                        return Err(RelayError::NoAvailablePort {
                            range: self.port_range,
                        });
                    }
                    debug!(port = port, error = %e, "Port unavailable, trying next");
                }
            }
        }

        Err(RelayError::NoAvailablePort {
            range: self.port_range,
        })
    }

    /// Accept loop - runs as background task.
    async fn accept_loop(
        listener: TcpListener,
        socket_path: PathBuf,
        shutdown: Arc<AtomicBool>,
        bound_port: Arc<AtomicU16>,
    ) {
        let port = bound_port.load(Ordering::SeqCst);
        info!(port = port, "TCP relay accept loop started");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                debug!(port = port, "Accept loop shutdown requested");
                break;
            }

            // Accept with timeout so we can check shutdown flag
            let accept_result =
                tokio::time::timeout(std::time::Duration::from_secs(1), listener.accept()).await;

            match accept_result {
                Ok(Ok((tcp_stream, peer_addr))) => {
                    debug!(
                        port = port,
                        peer = %peer_addr,
                        "New TCP connection"
                    );

                    let socket_path = socket_path.clone();
                    let shutdown = Arc::clone(&shutdown);

                    // Spawn handler for this connection
                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_connection(tcp_stream, socket_path, shutdown).await
                        {
                            warn!(error = %e, "Connection handler error");
                        }
                    });
                }
                Ok(Err(e)) => {
                    error!(port = port, error = %e, "Accept error");
                }
                Err(_) => {
                    // Timeout - loop again to check shutdown
                }
            }
        }

        // Clear bound port on exit
        bound_port.store(0, Ordering::SeqCst);
        info!(port = port, "TCP relay accept loop ended");
    }

    /// Handle a single TCP connection by bridging to Unix socket.
    async fn handle_connection(
        mut tcp_stream: tokio::net::TcpStream,
        socket_path: PathBuf,
        shutdown: Arc<AtomicBool>,
    ) -> Result<(), RelayError> {
        // Connect to the Unix socket
        let mut unix_stream =
            UnixStream::connect(&socket_path)
                .await
                .map_err(|e| RelayError::SocketConnectionFailed(e))?;

        debug!(
            socket = %socket_path.display(),
            "Connected to Unix socket, starting bidirectional copy"
        );

        // Bidirectional copy until one side closes or error occurs
        tokio::select! {
            result = copy_bidirectional(&mut tcp_stream, &mut unix_stream) => {
                match result {
                    Ok((to_unix, to_tcp)) => {
                        debug!(
                            to_unix = to_unix,
                            to_tcp = to_tcp,
                            "Connection closed normally"
                        );
                    }
                    Err(e) => {
                        // Connection errors are expected when clients disconnect
                        debug!(error = %e, "Bidirectional copy ended with error");
                    }
                }
            }
            _ = async {
                while !shutdown.load(Ordering::SeqCst) {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            } => {
                debug!("Shutdown requested, closing connection");
            }
        }

        Ok(())
    }
}

impl Drop for TcpRelay {
    fn drop(&mut self) {
        // Signal shutdown on drop
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    /// Helper to create a test Unix socket that echoes data
    async fn create_echo_socket() -> (PathBuf, JoinHandle<()>) {
        let socket_file = NamedTempFile::new().unwrap();
        let socket_path = socket_file.path().to_path_buf();
        // Remove the temp file so we can create a socket there
        drop(socket_file);
        std::fs::remove_file(&socket_path).ok();

        let listener = UnixListener::bind(&socket_path).unwrap();
        let path_clone = socket_path.clone();

        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        // Echo back anything received
                        tokio::spawn(async move {
                            let mut buf = [0u8; 1024];
                            loop {
                                match stream.read(&mut buf).await {
                                    Ok(0) => break, // EOF
                                    Ok(n) => {
                                        if stream.write_all(&buf[..n]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        // Give the listener time to start
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        (path_clone, handle)
    }

    // === Unit Tests for Error Types ===

    #[test]
    fn test_relay_error_display() {
        let err = RelayError::NoAvailablePort {
            range: (19000, 19999),
        };
        assert!(err.to_string().contains("No available port"));
        assert!(err.to_string().contains("19000"));
        assert!(err.to_string().contains("19999"));

        let err = RelayError::NotRunning;
        assert_eq!(err.to_string(), "Relay is not running");

        let err = RelayError::AlreadyRunning;
        assert_eq!(err.to_string(), "Relay is already running");

        let err = RelayError::InvalidPortRange {
            start: 20000,
            end: 19000,
        };
        assert!(err.to_string().contains("Invalid port range"));
        assert!(err.to_string().contains("20000"));
        assert!(err.to_string().contains("19000"));
    }

    #[test]
    fn test_relay_error_bind_failed() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "port in use");
        let err = RelayError::BindFailed(io_err);
        assert!(err.to_string().contains("Failed to bind"));
    }

    #[test]
    fn test_relay_error_socket_connection_failed() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "socket not found");
        let err = RelayError::SocketConnectionFailed(io_err);
        assert!(err.to_string().contains("Failed to connect to Unix socket"));
    }

    // === Unit Tests for TcpRelay Creation ===

    #[test]
    fn test_relay_new_valid_range() {
        let relay = TcpRelay::new(PathBuf::from("/tmp/test.sock"), (19000, 19999));
        assert!(relay.is_ok());

        let relay = relay.unwrap();
        assert_eq!(relay.port_range(), (19000, 19999));
        assert_eq!(relay.socket_path(), &PathBuf::from("/tmp/test.sock"));
    }

    #[test]
    fn test_relay_new_single_port_range() {
        let relay = TcpRelay::new(PathBuf::from("/tmp/test.sock"), (19500, 19500));
        assert!(relay.is_ok());
        assert_eq!(relay.unwrap().port_range(), (19500, 19500));
    }

    #[test]
    fn test_relay_new_invalid_range() {
        let result = TcpRelay::new(PathBuf::from("/tmp/test.sock"), (20000, 19000));
        assert!(matches!(
            result,
            Err(RelayError::InvalidPortRange {
                start: 20000,
                end: 19000
            })
        ));
    }

    #[test]
    fn test_relay_initial_state() {
        let relay = TcpRelay::new(PathBuf::from("/tmp/test.sock"), (19000, 19999)).unwrap();
        assert!(!relay.is_running());
        assert!(relay.port().is_none());
    }

    // === Async Tests for Start/Stop ===

    #[tokio::test]
    async fn test_relay_start_stop() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        let mut relay = TcpRelay::new(socket_path.clone(), (19100, 19199)).unwrap();

        // Start the relay
        let port = relay.start().await;
        assert!(port.is_ok(), "Failed to start relay: {:?}", port);

        let port = port.unwrap();
        assert!(port >= 19100 && port <= 19199);
        assert!(relay.is_running());
        assert_eq!(relay.port(), Some(port));

        // Stop the relay
        let result = relay.stop().await;
        assert!(result.is_ok(), "Failed to stop relay: {:?}", result);
        assert!(!relay.is_running());
        assert!(relay.port().is_none());

        // Cleanup
        std::fs::remove_file(&socket_path).ok();
    }

    #[tokio::test]
    async fn test_relay_start_already_running() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        let mut relay = TcpRelay::new(socket_path.clone(), (19200, 19299)).unwrap();

        relay.start().await.expect("First start should succeed");

        // Try to start again
        let result = relay.start().await;
        assert!(matches!(result, Err(RelayError::AlreadyRunning)));

        relay.stop().await.ok();
        std::fs::remove_file(&socket_path).ok();
    }

    #[tokio::test]
    async fn test_relay_stop_not_running() {
        let relay_result = TcpRelay::new(PathBuf::from("/tmp/nonexistent.sock"), (19300, 19399));
        let mut relay = relay_result.unwrap();

        let result = relay.stop().await;
        assert!(matches!(result, Err(RelayError::NotRunning)));
    }

    // === Integration Tests for Data Transfer ===

    #[tokio::test]
    async fn test_relay_echo_data() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        let mut relay = TcpRelay::new(socket_path.clone(), (19400, 19499)).unwrap();
        let port = relay.start().await.expect("Failed to start relay");

        // Connect via TCP and send data
        let mut tcp_stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("Failed to connect to relay");

        let test_data = b"Hello, MCP!\n";
        tcp_stream
            .write_all(test_data)
            .await
            .expect("Failed to write");

        // Read echoed data
        let mut buf = [0u8; 64];
        let n = tcp_stream.read(&mut buf).await.expect("Failed to read");

        assert_eq!(&buf[..n], test_data);

        relay.stop().await.ok();
        std::fs::remove_file(&socket_path).ok();
    }

    #[tokio::test]
    async fn test_relay_multiple_connections() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        let mut relay = TcpRelay::new(socket_path.clone(), (19500, 19599)).unwrap();
        let port = relay.start().await.expect("Failed to start relay");

        // Create multiple concurrent connections
        let mut handles = vec![];

        for i in 0..5 {
            let port = port;
            let handle = tokio::spawn(async move {
                let mut tcp_stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .expect("Failed to connect");

                let test_data = format!("Message {}\n", i);
                tcp_stream
                    .write_all(test_data.as_bytes())
                    .await
                    .expect("Failed to write");

                let mut buf = [0u8; 64];
                let n = tcp_stream.read(&mut buf).await.expect("Failed to read");

                assert_eq!(&buf[..n], test_data.as_bytes());
            });
            handles.push(handle);
        }

        // Wait for all connections to complete
        for handle in handles {
            handle.await.expect("Task panicked");
        }

        relay.stop().await.ok();
        std::fs::remove_file(&socket_path).ok();
    }

    #[tokio::test]
    async fn test_relay_socket_not_found() {
        // Use a path that definitely doesn't exist
        let socket_path = PathBuf::from("/tmp/definitely_not_a_real_socket_12345.sock");
        std::fs::remove_file(&socket_path).ok(); // Ensure it doesn't exist

        let mut relay = TcpRelay::new(socket_path, (19600, 19699)).unwrap();
        let port = relay.start().await.expect("Relay should start");

        // Connect - the relay should accept the connection
        let tcp_result = tokio::net::TcpStream::connect(("127.0.0.1", port)).await;
        assert!(tcp_result.is_ok(), "Should connect to relay");

        // Give it a moment for the handler to try connecting to the nonexistent socket
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        relay.stop().await.ok();
    }

    // === Tests for Port Selection ===

    #[tokio::test]
    async fn test_relay_port_in_range() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        // Try with a small range
        let mut relay = TcpRelay::new(socket_path.clone(), (19700, 19710)).unwrap();
        let port = relay.start().await.expect("Should find a port");

        assert!(
            port >= 19700 && port <= 19710,
            "Port {} not in range 19700-19710",
            port
        );

        relay.stop().await.ok();
        std::fs::remove_file(&socket_path).ok();
    }

    #[tokio::test]
    async fn test_default_port_range_constant() {
        assert_eq!(DEFAULT_PORT_RANGE, (19000, 19999));
    }

    // === Drop Behavior Tests ===

    #[tokio::test]
    async fn test_relay_drop_signals_shutdown() {
        let (socket_path, _echo_handle) = create_echo_socket().await;

        let shutdown_flag;
        {
            let mut relay = TcpRelay::new(socket_path.clone(), (19800, 19899)).unwrap();
            relay.start().await.expect("Should start");

            shutdown_flag = Arc::clone(&relay.shutdown);
            assert!(!shutdown_flag.load(Ordering::SeqCst));

            // Relay dropped here
        }

        // Shutdown flag should be set after drop
        assert!(shutdown_flag.load(Ordering::SeqCst));

        std::fs::remove_file(&socket_path).ok();
    }

    // === Tests for Getters ===

    #[test]
    fn test_relay_getters() {
        let path = PathBuf::from("/custom/path/socket.sock");
        let relay = TcpRelay::new(path.clone(), (20000, 20100)).unwrap();

        assert_eq!(relay.socket_path(), &path);
        assert_eq!(relay.port_range(), (20000, 20100));
        assert!(!relay.is_running());
        assert!(relay.port().is_none());
    }
}
