// ABOUTME: Request ID rewriting for JSON-RPC multiplexing (CRITICAL)
// ABOUTME: Solves collision when multiple sessions send same request IDs

//! Request ID rewriting for JSON-RPC multiplexing.
//!
//! This module solves the request ID collision problem in multiplexed JSON-RPC.
//! When multiple sessions send requests with the same ID (e.g., both send id:1),
//! we rewrite to globally unique proxy IDs and restore on response.
//!
//! # Flow
//!
//! 1. Client sends `{"id": 1, ...}`
//! 2. Proxy rewrites to `{"id": "uuid-xxx", ...}`, stores mapping
//! 3. MCP responds `{"id": "uuid-xxx", ...}`
//! 4. Proxy restores to `{"id": 1, ...}`, routes to correct client
//!
//! # Thread Safety
//!
//! All operations are thread-safe via `Arc<RwLock<_>>`. The cleanup task runs
//! in the background to remove expired mappings.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Globally unique proxy request ID that cannot collide across sessions.
///
/// This replaces client-provided IDs during transit through the proxy,
/// ensuring that even if multiple clients use the same ID (e.g., sequential
/// integers starting at 1), they remain distinguishable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyRequestId(pub Uuid);

impl ProxyRequestId {
    /// Create a new globally unique proxy request ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Convert to JSON-RPC id value (always a string).
    #[must_use]
    pub fn to_json(&self) -> Value {
        Value::String(self.0.to_string())
    }

    /// Parse from a JSON value if it matches our UUID format.
    #[must_use]
    pub fn from_json(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Uuid::parse_str(s).ok().map(Self),
            _ => None,
        }
    }
}

impl Default for ProxyRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProxyRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Original request ID as provided by the client.
///
/// JSON-RPC allows IDs to be numbers, strings, or null. Multiple clients
/// may use the same ID values, which is why we need the proxy ID layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OriginalRequestId {
    /// Numeric ID (most common for auto-incrementing clients).
    Number(i64),
    /// String ID (some clients use UUIDs or custom strings).
    String(String),
    /// Null ID (rare but valid per JSON-RPC spec).
    Null,
}

impl OriginalRequestId {
    /// Parse from a JSON value.
    ///
    /// Returns `None` for invalid types (objects, arrays, etc.).
    #[must_use]
    pub fn from_json(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_i64().map(Self::Number),
            Value::String(s) => Some(Self::String(s.clone())),
            Value::Null => Some(Self::Null),
            _ => None,
        }
    }

    /// Convert back to JSON value for response restoration.
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Number(n) => Value::Number((*n).into()),
            Self::String(s) => Value::String(s.clone()),
            Self::Null => Value::Null,
        }
    }
}

impl std::fmt::Display for OriginalRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Null => write!(f, "null"),
        }
    }
}

/// Mapping from proxy ID to session and original ID.
///
/// Includes timing information for TTL-based cleanup.
#[derive(Debug, Clone)]
pub struct RequestMapping {
    /// Session that sent this request.
    pub session_id: String,
    /// Original ID as sent by the client.
    pub original_id: OriginalRequestId,
    /// When this mapping was created.
    pub created_at: Instant,
    /// Maximum time to wait for a response before cleanup.
    pub timeout: Duration,
}

impl RequestMapping {
    /// Create a new request mapping.
    #[must_use]
    pub fn new(session_id: String, original_id: OriginalRequestId, timeout: Duration) -> Self {
        Self {
            session_id,
            original_id,
            created_at: Instant::now(),
            timeout,
        }
    }

    /// Check if this mapping has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.timeout
    }
}

/// Thread-safe request ID router for JSON-RPC multiplexing.
///
/// Handles rewriting request IDs to prevent collisions and restoring
/// them on responses for correct routing.
///
/// # Example
///
/// ```ignore
/// let router = RequestRouter::new(Duration::from_secs(300));
///
/// // Rewrite outgoing request
/// let mut request = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
/// let proxy_id = router.rewrite_request("session-1", &mut request).await;
///
/// // Later, restore incoming response
/// let mut response = json!({"jsonrpc": "2.0", "id": proxy_id.to_string(), "result": {}});
/// let (session, original_id) = router.restore_response(&mut response).await.unwrap();
/// ```
pub struct RequestRouter {
    /// Active request mappings: proxy ID -> (session, original ID, timing).
    mappings: Arc<RwLock<HashMap<ProxyRequestId, RequestMapping>>>,
    /// Default timeout for new mappings.
    default_timeout: Duration,
    /// Flag to signal cleanup task shutdown.
    shutdown: Arc<AtomicBool>,
    /// Handle to cleanup task for graceful shutdown.
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RequestRouter {
    /// Create a new request router with the specified default timeout.
    ///
    /// Spawns a background task that periodically cleans up expired mappings.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        let mappings: Arc<RwLock<HashMap<ProxyRequestId, RequestMapping>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let cleanup_mappings = Arc::clone(&mappings);
        let cleanup_shutdown = Arc::clone(&shutdown);

        // Spawn cleanup task that runs every 10 seconds
        let cleanup_handle = tokio::spawn(async move {
            let cleanup_interval = Duration::from_secs(10);
            loop {
                tokio::time::sleep(cleanup_interval).await;

                if cleanup_shutdown.load(Ordering::Relaxed) {
                    break;
                }

                // Clean up expired mappings
                let mut mappings = cleanup_mappings.write().await;
                mappings.retain(|_, mapping| !mapping.is_expired());
            }
        });

        Self {
            mappings,
            default_timeout: timeout,
            shutdown,
            cleanup_handle: Some(cleanup_handle),
        }
    }

    /// Rewrite a JSON-RPC request's ID to a globally unique proxy ID.
    ///
    /// Stores the mapping for later response restoration.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Identifier for the client session
    /// * `request` - Mutable JSON-RPC request to rewrite
    ///
    /// # Returns
    ///
    /// The new proxy ID if successful, `None` if the request has no valid ID field.
    pub async fn rewrite_request(
        &self,
        session_id: &str,
        request: &mut Value,
    ) -> Option<ProxyRequestId> {
        // Extract the original ID
        let id_value = request.get("id")?;
        let original_id = OriginalRequestId::from_json(id_value)?;

        // Generate new proxy ID
        let proxy_id = ProxyRequestId::new();

        // Store mapping
        let mapping =
            RequestMapping::new(session_id.to_string(), original_id, self.default_timeout);

        {
            let mut mappings = self.mappings.write().await;
            mappings.insert(proxy_id.clone(), mapping);
        }

        // Rewrite the request ID
        if let Some(obj) = request.as_object_mut() {
            obj.insert("id".to_string(), proxy_id.to_json());
        }

        Some(proxy_id)
    }

    /// Restore a JSON-RPC response's ID to the original client ID.
    ///
    /// Removes the mapping after restoration (each response is handled once).
    ///
    /// # Arguments
    ///
    /// * `response` - Mutable JSON-RPC response to restore
    ///
    /// # Returns
    ///
    /// The session ID and original request ID if found, `None` if the response
    /// ID doesn't match any pending request.
    pub async fn restore_response(
        &self,
        response: &mut Value,
    ) -> Option<(String, OriginalRequestId)> {
        // Extract proxy ID from response
        let id_value = response.get("id")?;
        let proxy_id = ProxyRequestId::from_json(id_value)?;

        // Look up and remove mapping
        let mapping = {
            let mut mappings = self.mappings.write().await;
            mappings.remove(&proxy_id)?
        };

        // Restore original ID
        if let Some(obj) = response.as_object_mut() {
            obj.insert("id".to_string(), mapping.original_id.to_json());
        }

        Some((mapping.session_id, mapping.original_id))
    }

    /// Manually trigger cleanup of expired mappings.
    ///
    /// Returns the number of expired mappings removed.
    pub async fn cleanup_expired(&self) -> usize {
        let mut mappings = self.mappings.write().await;
        let before = mappings.len();
        mappings.retain(|_, mapping| !mapping.is_expired());
        before - mappings.len()
    }

    /// Get the number of pending (unresolved) requests.
    pub async fn pending_count(&self) -> usize {
        self.mappings.read().await.len()
    }

    /// Check if there are any pending requests.
    pub async fn has_pending_requests(&self) -> bool {
        !self.mappings.read().await.is_empty()
    }

    /// Get pending requests for a specific session.
    pub async fn pending_for_session(&self, session_id: &str) -> usize {
        self.mappings
            .read()
            .await
            .values()
            .filter(|m| m.session_id == session_id)
            .count()
    }

    /// Shutdown the cleanup task gracefully.
    pub async fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.cleanup_handle.take() {
            // Give it time to notice the shutdown flag
            let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
        }
    }

    /// Cancel all pending requests for a session (e.g., on disconnect).
    ///
    /// Returns the number of requests cancelled.
    pub async fn cancel_session(&self, session_id: &str) -> usize {
        let mut mappings = self.mappings.write().await;
        let before = mappings.len();
        mappings.retain(|_, mapping| mapping.session_id != session_id);
        before - mappings.len()
    }
}

impl Drop for RequestRouter {
    fn drop(&mut self) {
        // Signal shutdown on drop (non-async cleanup)
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_proxy_request_id_creation() {
        let id1 = ProxyRequestId::new();
        let id2 = ProxyRequestId::new();

        // Each ID should be unique
        assert_ne!(id1, id2);
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_proxy_request_id_json_roundtrip() {
        let id = ProxyRequestId::new();
        let json = id.to_json();
        let parsed = ProxyRequestId::from_json(&json).expect("Should parse");

        assert_eq!(id, parsed);
    }

    #[test]
    fn test_proxy_request_id_from_invalid_json() {
        // Number is not a valid proxy ID
        assert!(ProxyRequestId::from_json(&json!(123)).is_none());

        // Non-UUID string is not valid
        assert!(ProxyRequestId::from_json(&json!("not-a-uuid")).is_none());

        // Null is not valid
        assert!(ProxyRequestId::from_json(&json!(null)).is_none());
    }

    #[test]
    fn test_original_request_id_number() {
        let id = OriginalRequestId::from_json(&json!(42)).expect("Should parse number");
        assert!(matches!(id, OriginalRequestId::Number(42)));
        assert_eq!(id.to_json(), json!(42));
    }

    #[test]
    fn test_original_request_id_string() {
        let id = OriginalRequestId::from_json(&json!("req-123")).expect("Should parse string");
        assert!(matches!(id, OriginalRequestId::String(ref s) if s == "req-123"));
        assert_eq!(id.to_json(), json!("req-123"));
    }

    #[test]
    fn test_original_request_id_null() {
        let id = OriginalRequestId::from_json(&json!(null)).expect("Should parse null");
        assert!(matches!(id, OriginalRequestId::Null));
        assert_eq!(id.to_json(), json!(null));
    }

    #[test]
    fn test_original_request_id_invalid() {
        // Objects are not valid
        assert!(OriginalRequestId::from_json(&json!({"id": 1})).is_none());

        // Arrays are not valid
        assert!(OriginalRequestId::from_json(&json!([1, 2, 3])).is_none());
    }

    #[test]
    fn test_request_mapping_expiry() {
        let mapping = RequestMapping::new(
            "session-1".to_string(),
            OriginalRequestId::Number(1),
            Duration::from_millis(50),
        );

        assert!(!mapping.is_expired());

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(60));
        assert!(mapping.is_expired());
    }

    #[tokio::test]
    async fn test_rewrite_request_with_number_id() {
        let router = RequestRouter::new(Duration::from_secs(300));

        let mut request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "test",
            "params": {}
        });

        let proxy_id =
            router.rewrite_request("session-1", &mut request).await.expect("Should rewrite");

        // Request should now have UUID string ID
        let new_id = request.get("id").expect("Should have id");
        assert!(new_id.is_string());

        // Should be parseable as our proxy ID
        let parsed = ProxyRequestId::from_json(new_id).expect("Should parse");
        assert_eq!(proxy_id, parsed);

        // Should have one pending request
        assert_eq!(router.pending_count().await, 1);
    }

    #[tokio::test]
    async fn test_rewrite_request_with_string_id() {
        let router = RequestRouter::new(Duration::from_secs(300));

        let mut request = json!({
            "jsonrpc": "2.0",
            "id": "my-custom-id",
            "method": "test"
        });

        let proxy_id =
            router.rewrite_request("session-1", &mut request).await.expect("Should rewrite");

        assert!(proxy_id.0.to_string().len() == 36); // UUID length
    }

    #[tokio::test]
    async fn test_rewrite_request_with_null_id() {
        let router = RequestRouter::new(Duration::from_secs(300));

        let mut request = json!({
            "jsonrpc": "2.0",
            "id": null,
            "method": "test"
        });

        let proxy_id =
            router.rewrite_request("session-1", &mut request).await.expect("Should rewrite");

        assert!(proxy_id.0.to_string().len() == 36);
    }

    #[tokio::test]
    async fn test_rewrite_request_no_id() {
        let router = RequestRouter::new(Duration::from_secs(300));

        // Notification - no ID field
        let mut request = json!({
            "jsonrpc": "2.0",
            "method": "notify"
        });

        let result = router.rewrite_request("session-1", &mut request).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_restore_response() {
        let router = RequestRouter::new(Duration::from_secs(300));

        // First, rewrite a request
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "test"
        });

        let proxy_id =
            router.rewrite_request("session-1", &mut request).await.expect("Should rewrite");

        // Now create a response with the proxy ID
        let mut response = json!({
            "jsonrpc": "2.0",
            "id": proxy_id.0.to_string(),
            "result": {"success": true}
        });

        let (session, original_id) =
            router.restore_response(&mut response).await.expect("Should restore");

        assert_eq!(session, "session-1");
        assert!(matches!(original_id, OriginalRequestId::Number(42)));

        // Response should now have original ID
        assert_eq!(response.get("id"), Some(&json!(42)));

        // Mapping should be removed
        assert_eq!(router.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_restore_response_unknown_id() {
        let router = RequestRouter::new(Duration::from_secs(300));

        let mut response = json!({
            "jsonrpc": "2.0",
            "id": Uuid::new_v4().to_string(),
            "result": {}
        });

        let result = router.restore_response(&mut response).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_rewrite_restore_roundtrip_all_id_types() {
        let router = RequestRouter::new(Duration::from_secs(300));

        // Test with Number ID
        let mut req1 = json!({"jsonrpc": "2.0", "id": 123, "method": "test"});
        let pid1 = router.rewrite_request("s1", &mut req1).await.unwrap();
        let mut resp1 = json!({"jsonrpc": "2.0", "id": pid1.0.to_string(), "result": {}});
        let (sess1, oid1) = router.restore_response(&mut resp1).await.unwrap();
        assert_eq!(sess1, "s1");
        assert_eq!(resp1["id"], json!(123));
        assert!(matches!(oid1, OriginalRequestId::Number(123)));

        // Test with String ID
        let mut req2 = json!({"jsonrpc": "2.0", "id": "abc-def", "method": "test"});
        let pid2 = router.rewrite_request("s2", &mut req2).await.unwrap();
        let mut resp2 = json!({"jsonrpc": "2.0", "id": pid2.0.to_string(), "result": {}});
        let (sess2, oid2) = router.restore_response(&mut resp2).await.unwrap();
        assert_eq!(sess2, "s2");
        assert_eq!(resp2["id"], json!("abc-def"));
        assert!(matches!(oid2, OriginalRequestId::String(ref s) if s == "abc-def"));

        // Test with Null ID
        let mut req3 = json!({"jsonrpc": "2.0", "id": null, "method": "test"});
        let pid3 = router.rewrite_request("s3", &mut req3).await.unwrap();
        let mut resp3 = json!({"jsonrpc": "2.0", "id": pid3.0.to_string(), "result": {}});
        let (sess3, oid3) = router.restore_response(&mut resp3).await.unwrap();
        assert_eq!(sess3, "s3");
        assert_eq!(resp3["id"], json!(null));
        assert!(matches!(oid3, OriginalRequestId::Null));
    }

    #[tokio::test]
    async fn test_concurrent_requests_no_collision() {
        let router = RequestRouter::new(Duration::from_secs(300));

        // Two sessions both send id: 1
        let mut req1 = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let mut req2 = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});

        let pid1 = router.rewrite_request("session-A", &mut req1).await.unwrap();
        let pid2 = router.rewrite_request("session-B", &mut req2).await.unwrap();

        // Proxy IDs must be different
        assert_ne!(pid1, pid2);

        // Both should be pending
        assert_eq!(router.pending_count().await, 2);

        // Restore in reverse order
        let mut resp2 = json!({"jsonrpc": "2.0", "id": pid2.0.to_string(), "result": "B"});
        let (sess2, _) = router.restore_response(&mut resp2).await.unwrap();
        assert_eq!(sess2, "session-B");
        assert_eq!(resp2["id"], json!(1)); // Original ID restored

        let mut resp1 = json!({"jsonrpc": "2.0", "id": pid1.0.to_string(), "result": "A"});
        let (sess1, _) = router.restore_response(&mut resp1).await.unwrap();
        assert_eq!(sess1, "session-A");
        assert_eq!(resp1["id"], json!(1)); // Original ID restored

        assert_eq!(router.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let router = RequestRouter::new(Duration::from_millis(10));

        // Create some requests
        let mut req1 = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let mut req2 = json!({"jsonrpc": "2.0", "id": 2, "method": "test"});

        router.rewrite_request("s1", &mut req1).await.unwrap();
        router.rewrite_request("s2", &mut req2).await.unwrap();

        assert_eq!(router.pending_count().await, 2);

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Manual cleanup
        let removed = router.cleanup_expired().await;
        assert_eq!(removed, 2);
        assert_eq!(router.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_session() {
        let router = RequestRouter::new(Duration::from_secs(300));

        // Create requests from multiple sessions
        let mut req1 = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let mut req2 = json!({"jsonrpc": "2.0", "id": 2, "method": "test"});
        let mut req3 = json!({"jsonrpc": "2.0", "id": 3, "method": "test"});

        router.rewrite_request("session-A", &mut req1).await.unwrap();
        router.rewrite_request("session-A", &mut req2).await.unwrap();
        router.rewrite_request("session-B", &mut req3).await.unwrap();

        assert_eq!(router.pending_count().await, 3);
        assert_eq!(router.pending_for_session("session-A").await, 2);
        assert_eq!(router.pending_for_session("session-B").await, 1);

        // Cancel session A
        let cancelled = router.cancel_session("session-A").await;
        assert_eq!(cancelled, 2);
        assert_eq!(router.pending_count().await, 1);
        assert_eq!(router.pending_for_session("session-A").await, 0);
        assert_eq!(router.pending_for_session("session-B").await, 1);
    }

    #[tokio::test]
    async fn test_has_pending_requests() {
        let router = RequestRouter::new(Duration::from_secs(300));

        assert!(!router.has_pending_requests().await);

        let mut request = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let proxy_id = router.rewrite_request("s1", &mut request).await.unwrap();

        assert!(router.has_pending_requests().await);

        let mut response = json!({"jsonrpc": "2.0", "id": proxy_id.0.to_string(), "result": {}});
        router.restore_response(&mut response).await.unwrap();

        assert!(!router.has_pending_requests().await);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut router = RequestRouter::new(Duration::from_secs(300));

        // Shutdown should complete without hanging
        router.shutdown().await;

        // Router should still work for synchronous operations
        // (just no cleanup task running)
        let mut request = json!({"jsonrpc": "2.0", "id": 1, "method": "test"});
        let result = router.rewrite_request("s1", &mut request).await;
        assert!(result.is_some());
    }

    #[test]
    fn test_original_request_id_display() {
        assert_eq!(format!("{}", OriginalRequestId::Number(42)), "42");
        assert_eq!(
            format!("{}", OriginalRequestId::String("abc".into())),
            "\"abc\""
        );
        assert_eq!(format!("{}", OriginalRequestId::Null), "null");
    }

    #[test]
    fn test_proxy_request_id_display() {
        let id = ProxyRequestId::new();
        let display = format!("{id}");
        // Should be a valid UUID string
        assert_eq!(display.len(), 36);
        assert!(Uuid::parse_str(&display).is_ok());
    }
}
