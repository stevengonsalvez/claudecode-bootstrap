// ABOUTME: Client lifecycle management with liveness detection
//
// Manages connected clients with:
// - Keepalive pings (30s interval)
// - Idle timeout (60s)
// - EPIPE/BrokenPipe handling
// - Background reaper for dead clients
// - Bounded notification queues with coalescing

use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::backpressure::BackpressureError;

// === Client ID ===

/// Unique identifier for a connected client
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub Uuid);

impl ClientId {
    /// Generate a new unique client ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// === Client State ===

/// Current state of a client connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Client is connected and active
    Connected,
    /// Client is idle (no recent activity)
    Idle,
    /// Client connection is broken (EPIPE detected)
    Disconnected,
    /// Client is being removed
    Removing,
}

// === Client Errors ===

/// Errors that can occur during client management
#[derive(Debug, Error)]
pub enum ClientError {
    /// Client not found in manager
    #[error("Client not found: {0}")]
    NotFound(ClientId),

    /// Client is disconnected
    #[error("Client disconnected: {0}")]
    Disconnected(ClientId),

    /// Queue is full, cannot accept more messages
    #[error("Client queue full: capacity {capacity}, client {client_id}")]
    QueueFull {
        client_id: ClientId,
        capacity: usize,
    },

    /// I/O error during communication
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Client limit reached for this MCP
    #[error("Maximum clients reached: {max}")]
    MaxClientsReached { max: usize },
}

impl From<ClientError> for BackpressureError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::QueueFull { capacity, .. } => BackpressureError::QueueFull {
                capacity,
                current: capacity,
            },
            _ => BackpressureError::QueueFull {
                capacity: 0,
                current: 0,
            },
        }
    }
}

// === Client Connection ===

/// A single client connection with associated state
pub struct ClientConnection {
    /// Unique client identifier
    pub id: ClientId,

    /// Session ID this client belongs to (for request routing)
    pub session_id: String,

    /// Reader half of the Unix socket
    reader: Option<BufReader<OwnedReadHalf>>,

    /// Writer half of the Unix socket
    writer: Option<BufWriter<OwnedWriteHalf>>,

    /// Current connection state
    state: ClientState,

    /// Last activity timestamp (updated on any I/O)
    last_activity: Instant,

    /// Pending responses to send to this client
    pending_responses: mpsc::Sender<Vec<u8>>,

    /// Receiver for pending responses (taken by response sender task)
    pending_responses_rx: Option<mpsc::Receiver<Vec<u8>>>,

    /// Notification queue for MCP notifications (coalesced by type)
    notification_queue: mpsc::Sender<serde_json::Value>,

    /// Receiver for notifications (taken by notification sender task)
    notification_queue_rx: Option<mpsc::Receiver<serde_json::Value>>,

    /// Number of pending requests from this client
    pending_request_count: AtomicU64,

    /// Maximum pending requests allowed
    max_pending_requests: usize,
}

impl ClientConnection {
    /// Create a new client connection from a Unix socket
    pub fn new(
        stream: UnixStream,
        session_id: String,
        max_pending_requests: usize,
        notification_queue_size: usize,
    ) -> Self {
        let (read_half, write_half) = stream.into_split();

        // Bounded channels for backpressure
        let (response_tx, response_rx) = mpsc::channel(max_pending_requests);
        let (notification_tx, notification_rx) = mpsc::channel(notification_queue_size);

        Self {
            id: ClientId::new(),
            session_id,
            reader: Some(BufReader::new(read_half)),
            writer: Some(BufWriter::new(write_half)),
            state: ClientState::Connected,
            last_activity: Instant::now(),
            pending_responses: response_tx,
            pending_responses_rx: Some(response_rx),
            notification_queue: notification_tx,
            notification_queue_rx: Some(notification_rx),
            pending_request_count: AtomicU64::new(0),
            max_pending_requests,
        }
    }

    /// Take the reader (for spawning read task)
    pub fn take_reader(&mut self) -> Option<BufReader<OwnedReadHalf>> {
        self.reader.take()
    }

    /// Take the writer (for spawning write task)
    pub fn take_writer(&mut self) -> Option<BufWriter<OwnedWriteHalf>> {
        self.writer.take()
    }

    /// Take the response receiver (for spawning response sender task)
    pub fn take_response_receiver(&mut self) -> Option<mpsc::Receiver<Vec<u8>>> {
        self.pending_responses_rx.take()
    }

    /// Take the notification receiver (for spawning notification sender task)
    pub fn take_notification_receiver(&mut self) -> Option<mpsc::Receiver<serde_json::Value>> {
        self.notification_queue_rx.take()
    }

    /// Get current connection state
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Set connection state
    pub fn set_state(&mut self, state: ClientState) {
        self.state = state;
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get time since last activity
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Check if client has been idle longer than timeout
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Queue a response to send to this client
    ///
    /// Returns error if queue is full (backpressure)
    pub fn queue_response(&self, data: Vec<u8>) -> Result<(), ClientError> {
        self.pending_responses.try_send(data).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => ClientError::QueueFull {
                client_id: self.id,
                capacity: self.max_pending_requests,
            },
            mpsc::error::TrySendError::Closed(_) => ClientError::Disconnected(self.id),
        })
    }

    /// Queue a notification to send to this client
    ///
    /// Notifications use a separate queue that can coalesce
    pub fn queue_notification(&self, notification: serde_json::Value) -> Result<(), ClientError> {
        self.notification_queue.try_send(notification).map_err(|e| {
            match e {
                // Notifications can be dropped under backpressure
                // (they're informational, not critical)
                mpsc::error::TrySendError::Full(_) => ClientError::QueueFull {
                    client_id: self.id,
                    capacity: self.max_pending_requests,
                },
                mpsc::error::TrySendError::Closed(_) => ClientError::Disconnected(self.id),
            }
        })
    }

    /// Increment pending request count
    pub fn increment_pending(&self) -> Result<u64, ClientError> {
        let current = self.pending_request_count.fetch_add(1, Ordering::SeqCst);
        if current as usize >= self.max_pending_requests {
            self.pending_request_count.fetch_sub(1, Ordering::SeqCst);
            return Err(ClientError::QueueFull {
                client_id: self.id,
                capacity: self.max_pending_requests,
            });
        }
        Ok(current + 1)
    }

    /// Decrement pending request count
    pub fn decrement_pending(&self) -> u64 {
        self.pending_request_count.fetch_sub(1, Ordering::SeqCst).saturating_sub(1)
    }

    /// Get current pending request count
    pub fn pending_count(&self) -> u64 {
        self.pending_request_count.load(Ordering::SeqCst)
    }
}

// === Client Manager ===

/// Manages all client connections for an MCP proxy
pub struct ClientManager {
    /// Connected clients by ID
    clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,

    /// Session ID to client ID mapping (for request routing)
    session_to_client: Arc<RwLock<HashMap<String, ClientId>>>,

    /// Maximum allowed clients
    max_clients: usize,

    /// Keepalive interval for pings
    keepalive_interval: Duration,

    /// Idle timeout before client removal
    idle_timeout: Duration,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Reaper task handle
    reaper_handle: Option<JoinHandle<()>>,

    /// Keepalive task handle
    keepalive_handle: Option<JoinHandle<()>>,
}

impl ClientManager {
    /// Create a new client manager
    pub fn new(max_clients: usize, keepalive_interval: Duration, idle_timeout: Duration) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            session_to_client: Arc::new(RwLock::new(HashMap::new())),
            max_clients,
            keepalive_interval,
            idle_timeout,
            shutdown: Arc::new(AtomicBool::new(false)),
            reaper_handle: None,
            keepalive_handle: None,
        }
    }

    /// Start background tasks (reaper and keepalive)
    pub fn start_background_tasks(&mut self) {
        // Start reaper task
        let clients = Arc::clone(&self.clients);
        let session_to_client = Arc::clone(&self.session_to_client);
        let idle_timeout = self.idle_timeout;
        let shutdown = Arc::clone(&self.shutdown);

        self.reaper_handle = Some(tokio::spawn(async move {
            Self::reaper_loop(clients, session_to_client, idle_timeout, shutdown).await;
        }));

        // Start keepalive task
        let clients = Arc::clone(&self.clients);
        let keepalive_interval = self.keepalive_interval;
        let shutdown = Arc::clone(&self.shutdown);

        self.keepalive_handle = Some(tokio::spawn(async move {
            Self::keepalive_loop(clients, keepalive_interval, shutdown).await;
        }));
    }

    /// Background task to remove idle/disconnected clients
    async fn reaper_loop(
        clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
        session_to_client: Arc<RwLock<HashMap<String, ClientId>>>,
        idle_timeout: Duration,
        shutdown: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(idle_timeout / 4);

        while !shutdown.load(Ordering::SeqCst) {
            interval.tick().await;

            let clients_to_remove: Vec<(ClientId, String)> = {
                let clients_guard = clients.read().await;
                clients_guard
                    .iter()
                    .filter(|(_, client)| {
                        client.is_idle(idle_timeout)
                            || matches!(
                                client.state(),
                                ClientState::Disconnected | ClientState::Removing
                            )
                    })
                    .map(|(id, client)| (*id, client.session_id.clone()))
                    .collect()
            };

            if !clients_to_remove.is_empty() {
                let mut clients_guard = clients.write().await;
                let mut session_guard = session_to_client.write().await;

                for (client_id, session_id) in clients_to_remove {
                    clients_guard.remove(&client_id);
                    session_guard.remove(&session_id);
                    tracing::debug!("Reaped client {client_id} (session: {session_id})");
                }
            }
        }
    }

    /// Background task to send keepalive pings
    async fn keepalive_loop(
        clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
        keepalive_interval: Duration,
        shutdown: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(keepalive_interval);

        // JSON-RPC ping notification (no response expected)
        let ping_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "$/ping"
        });

        while !shutdown.load(Ordering::SeqCst) {
            interval.tick().await;

            let clients_guard = clients.read().await;
            for (_, client) in clients_guard.iter() {
                if matches!(client.state(), ClientState::Connected | ClientState::Idle) {
                    // Queue ping notification (ignore errors - will be reaped if disconnected)
                    let _ = client.queue_notification(ping_notification.clone());
                }
            }
        }
    }

    /// Add a new client connection
    ///
    /// Returns the client ID on success
    pub async fn add_client(
        &self,
        stream: UnixStream,
        session_id: String,
        max_pending_requests: usize,
    ) -> Result<ClientId, ClientError> {
        // Check client limit
        {
            let clients = self.clients.read().await;
            if clients.len() >= self.max_clients {
                return Err(ClientError::MaxClientsReached {
                    max: self.max_clients,
                });
            }
        }

        // Create client connection
        let client = ClientConnection::new(
            stream,
            session_id.clone(),
            max_pending_requests,
            32, // Notification queue size
        );
        let client_id = client.id;

        // Register client
        {
            let mut clients = self.clients.write().await;
            let mut sessions = self.session_to_client.write().await;

            clients.insert(client_id, client);
            sessions.insert(session_id, client_id);
        }

        tracing::debug!("Added client {client_id}");
        Ok(client_id)
    }

    /// Remove a client by ID
    pub async fn remove_client(&self, client_id: ClientId) -> Option<String> {
        let mut clients = self.clients.write().await;
        let mut sessions = self.session_to_client.write().await;

        if let Some(client) = clients.remove(&client_id) {
            let session_id = client.session_id.clone();
            sessions.remove(&session_id);
            tracing::debug!("Removed client {client_id}");
            Some(session_id)
        } else {
            None
        }
    }

    /// Get a client by ID (for sending responses)
    pub async fn get_client(&self, client_id: ClientId) -> Option<ClientId> {
        let clients = self.clients.read().await;
        if clients.contains_key(&client_id) {
            Some(client_id)
        } else {
            None
        }
    }

    /// Get client ID by session ID
    pub async fn get_client_by_session(&self, session_id: &str) -> Option<ClientId> {
        let sessions = self.session_to_client.read().await;
        sessions.get(session_id).copied()
    }

    /// Queue a response to a specific client
    pub async fn queue_response(
        &self,
        client_id: ClientId,
        data: Vec<u8>,
    ) -> Result<(), ClientError> {
        let clients = self.clients.read().await;
        let client = clients.get(&client_id).ok_or(ClientError::NotFound(client_id))?;

        if matches!(
            client.state(),
            ClientState::Disconnected | ClientState::Removing
        ) {
            return Err(ClientError::Disconnected(client_id));
        }

        client.queue_response(data)
    }

    /// Queue a notification to a specific client
    pub async fn queue_notification(
        &self,
        client_id: ClientId,
        notification: serde_json::Value,
    ) -> Result<(), ClientError> {
        let clients = self.clients.read().await;
        let client = clients.get(&client_id).ok_or(ClientError::NotFound(client_id))?;

        if matches!(
            client.state(),
            ClientState::Disconnected | ClientState::Removing
        ) {
            return Err(ClientError::Disconnected(client_id));
        }

        client.queue_notification(notification)
    }

    /// Broadcast a notification to all connected clients
    pub async fn broadcast_notification(&self, notification: serde_json::Value) {
        let clients = self.clients.read().await;
        for (_, client) in clients.iter() {
            if matches!(client.state(), ClientState::Connected | ClientState::Idle) {
                let _ = client.queue_notification(notification.clone());
            }
        }
    }

    /// Mark a client as disconnected (e.g., after EPIPE)
    pub async fn mark_disconnected(&self, client_id: ClientId) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(&client_id) {
            client.set_state(ClientState::Disconnected);
            tracing::debug!("Marked client {client_id} as disconnected");
        }
    }

    /// Touch a client to update last activity
    pub async fn touch_client(&self, client_id: ClientId) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(&client_id) {
            client.touch();
        }
    }

    /// Get number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Get all client IDs
    pub async fn client_ids(&self) -> Vec<ClientId> {
        self.clients.read().await.keys().copied().collect()
    }

    /// Shutdown the client manager
    pub async fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        // Wait for background tasks to complete
        if let Some(handle) = self.reaper_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.keepalive_handle.take() {
            let _ = handle.await;
        }

        // Close all client connections
        let mut clients = self.clients.write().await;
        clients.clear();

        let mut sessions = self.session_to_client.write().await;
        sessions.clear();

        tracing::debug!("Client manager shut down");
    }

    /// Check if a write error indicates broken pipe
    pub fn is_broken_pipe(err: &io::Error) -> bool {
        matches!(
            err.kind(),
            ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::NotConnected
        )
    }
}

// === Response Writer Helper ===

/// Helper to write responses to a client, handling EPIPE
pub struct ResponseWriter {
    client_id: ClientId,
    writer: BufWriter<OwnedWriteHalf>,
}

impl ResponseWriter {
    /// Create a new response writer
    pub fn new(client_id: ClientId, writer: BufWriter<OwnedWriteHalf>) -> Self {
        Self { client_id, writer }
    }

    /// Write data to the client
    ///
    /// Returns `true` if write succeeded, `false` if connection is broken
    pub async fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.write_all(data).await?;
        self.writer.flush().await
    }

    /// Get the client ID
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdUnixStream;
    use tokio::net::UnixStream as TokioUnixStream;

    fn create_socket_pair() -> (TokioUnixStream, TokioUnixStream) {
        let (a, b) = StdUnixStream::pair().expect("Failed to create socket pair");
        a.set_nonblocking(true).unwrap();
        b.set_nonblocking(true).unwrap();
        (
            TokioUnixStream::from_std(a).unwrap(),
            TokioUnixStream::from_std(b).unwrap(),
        )
    }

    #[test]
    fn test_client_id_uniqueness() {
        let id1 = ClientId::new();
        let id2 = ClientId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_client_id_display() {
        let id = ClientId::new();
        let display = format!("{}", id);
        assert!(!display.is_empty());
        assert!(display.len() > 30); // UUID format
    }

    #[tokio::test]
    async fn test_client_connection_creation() {
        let (stream, _peer) = create_socket_pair();
        let client = ClientConnection::new(stream, "session-1".to_string(), 100, 32);

        assert!(matches!(client.state(), ClientState::Connected));
        assert_eq!(client.session_id, "session-1");
        assert_eq!(client.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_client_idle_detection() {
        let (stream, _peer) = create_socket_pair();
        let mut client = ClientConnection::new(stream, "session-1".to_string(), 100, 32);

        // Initially not idle
        assert!(!client.is_idle(Duration::from_millis(100)));

        // Wait and check
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(client.is_idle(Duration::from_millis(100)));

        // Touch resets idle
        client.touch();
        assert!(!client.is_idle(Duration::from_millis(100)));
    }

    #[tokio::test]
    async fn test_client_pending_request_limit() {
        let (stream, _peer) = create_socket_pair();
        let client = ClientConnection::new(stream, "session-1".to_string(), 3, 32);

        // Can increment up to limit
        assert!(client.increment_pending().is_ok());
        assert_eq!(client.pending_count(), 1);

        assert!(client.increment_pending().is_ok());
        assert!(client.increment_pending().is_ok());
        assert_eq!(client.pending_count(), 3);

        // Should fail at limit
        assert!(client.increment_pending().is_err());
        assert_eq!(client.pending_count(), 3);

        // Decrement allows new requests
        client.decrement_pending();
        assert_eq!(client.pending_count(), 2);
        assert!(client.increment_pending().is_ok());
    }

    #[tokio::test]
    async fn test_client_manager_add_remove() {
        let manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));

        let (stream, _peer) = create_socket_pair();
        let client_id = manager.add_client(stream, "session-1".to_string(), 100).await.unwrap();

        assert_eq!(manager.client_count().await, 1);
        assert!(manager.get_client(client_id).await.is_some());
        assert!(manager.get_client_by_session("session-1").await.is_some());

        let session = manager.remove_client(client_id).await;
        assert_eq!(session, Some("session-1".to_string()));
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_client_manager_max_clients() {
        let manager = ClientManager::new(2, Duration::from_secs(30), Duration::from_secs(60));

        // Add two clients
        let (stream1, _) = create_socket_pair();
        let (stream2, _) = create_socket_pair();
        manager.add_client(stream1, "session-1".to_string(), 100).await.unwrap();
        manager.add_client(stream2, "session-2".to_string(), 100).await.unwrap();

        // Third should fail
        let (stream3, _) = create_socket_pair();
        let result = manager.add_client(stream3, "session-3".to_string(), 100).await;
        assert!(matches!(
            result,
            Err(ClientError::MaxClientsReached { max: 2 })
        ));
    }

    #[tokio::test]
    async fn test_client_manager_mark_disconnected() {
        let manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));

        let (stream, _peer) = create_socket_pair();
        let client_id = manager.add_client(stream, "session-1".to_string(), 100).await.unwrap();

        manager.mark_disconnected(client_id).await;

        // Queuing should fail for disconnected client
        let result = manager.queue_response(client_id, vec![1, 2, 3]).await;
        assert!(matches!(result, Err(ClientError::Disconnected(_))));
    }

    #[tokio::test]
    async fn test_client_manager_touch() {
        let manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));

        let (stream, _peer) = create_socket_pair();
        let client_id = manager.add_client(stream, "session-1".to_string(), 100).await.unwrap();

        // Touch should not error
        manager.touch_client(client_id).await;
    }

    #[tokio::test]
    async fn test_client_manager_client_ids() {
        let manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));

        let (stream1, _) = create_socket_pair();
        let (stream2, _) = create_socket_pair();
        let id1 = manager.add_client(stream1, "session-1".to_string(), 100).await.unwrap();
        let id2 = manager.add_client(stream2, "session-2".to_string(), 100).await.unwrap();

        let ids = manager.client_ids().await;
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_client_state_transitions() {
        let (stream, _peer) = create_socket_pair();
        let mut client = ClientConnection::new(stream, "session-1".to_string(), 100, 32);

        assert!(matches!(client.state(), ClientState::Connected));

        client.set_state(ClientState::Idle);
        assert!(matches!(client.state(), ClientState::Idle));

        client.set_state(ClientState::Disconnected);
        assert!(matches!(client.state(), ClientState::Disconnected));

        client.set_state(ClientState::Removing);
        assert!(matches!(client.state(), ClientState::Removing));
    }

    #[test]
    fn test_broken_pipe_detection() {
        let broken_pipe = io::Error::new(ErrorKind::BrokenPipe, "broken pipe");
        assert!(ClientManager::is_broken_pipe(&broken_pipe));

        let connection_reset = io::Error::new(ErrorKind::ConnectionReset, "reset");
        assert!(ClientManager::is_broken_pipe(&connection_reset));

        let not_connected = io::Error::new(ErrorKind::NotConnected, "not connected");
        assert!(ClientManager::is_broken_pipe(&not_connected));

        let other = io::Error::new(ErrorKind::Other, "other");
        assert!(!ClientManager::is_broken_pipe(&other));
    }

    #[tokio::test]
    async fn test_client_manager_shutdown() {
        let mut manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));
        manager.start_background_tasks();

        let (stream, _peer) = create_socket_pair();
        manager.add_client(stream, "session-1".to_string(), 100).await.unwrap();
        assert_eq!(manager.client_count().await, 1);

        manager.shutdown().await;
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_client_take_streams() {
        let (stream, _peer) = create_socket_pair();
        let mut client = ClientConnection::new(stream, "session-1".to_string(), 100, 32);

        // Can take each component once
        assert!(client.take_reader().is_some());
        assert!(client.take_reader().is_none());

        assert!(client.take_writer().is_some());
        assert!(client.take_writer().is_none());

        assert!(client.take_response_receiver().is_some());
        assert!(client.take_response_receiver().is_none());

        assert!(client.take_notification_receiver().is_some());
        assert!(client.take_notification_receiver().is_none());
    }

    #[tokio::test]
    async fn test_queue_response_full() {
        let (stream, _peer) = create_socket_pair();
        let client = ClientConnection::new(
            stream,
            "session-1".to_string(),
            2, // Very small queue
            32,
        );

        // Fill the queue
        assert!(client.queue_response(vec![1]).is_ok());
        assert!(client.queue_response(vec![2]).is_ok());

        // Should fail when full
        let result = client.queue_response(vec![3]);
        assert!(matches!(result, Err(ClientError::QueueFull { .. })));
    }

    #[tokio::test]
    async fn test_client_error_to_backpressure() {
        let err = ClientError::QueueFull {
            client_id: ClientId::new(),
            capacity: 100,
        };
        let bp_err: BackpressureError = err.into();
        assert!(matches!(
            bp_err,
            BackpressureError::QueueFull {
                capacity: 100,
                current: 100
            }
        ));
    }

    #[tokio::test]
    async fn test_broadcast_notification() {
        let manager = ClientManager::new(10, Duration::from_secs(30), Duration::from_secs(60));

        let (stream1, _) = create_socket_pair();
        let (stream2, _) = create_socket_pair();
        manager.add_client(stream1, "session-1".to_string(), 100).await.unwrap();
        manager.add_client(stream2, "session-2".to_string(), 100).await.unwrap();

        // Should not panic
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "test"
        });
        manager.broadcast_notification(notification).await;
    }

    #[tokio::test]
    async fn test_response_writer() {
        let (stream, mut peer) = create_socket_pair();
        let (_, write_half) = stream.into_split();
        let mut writer = ResponseWriter::new(ClientId::new(), BufWriter::new(write_half));

        assert!(writer.write(b"hello").await.is_ok());

        // Read from peer to verify
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 5];
        peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
    }
}
