// ABOUTME: Backpressure and circuit breaker for MCP socket pool load management
//
// Prevents OOM and cascade failures through circuit breaker pattern that stops
// sending requests to failing MCPs and allows gradual recovery.

use serde_json::Value;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;

// === JSON-RPC Error Codes for Backpressure Responses ===

/// Server is overloaded, queue full
pub const ERROR_SERVER_OVERLOADED: i32 = -32000;

/// Circuit breaker is open, requests blocked
pub const ERROR_CIRCUIT_OPEN: i32 = -32001;

/// Request timed out
pub const ERROR_REQUEST_TIMEOUT: i32 = -32002;

/// Create a JSON-RPC error response
///
/// # Arguments
/// * `code` - JSON-RPC error code (use constants above)
/// * `message` - Human-readable error message
/// * `id` - Request ID to include in response (null if unknown)
pub fn create_jsonrpc_error(code: i32, message: &str, id: Option<Value>) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": id.unwrap_or(Value::Null)
    })
}

// === Backpressure Error Types ===

/// Errors returned when backpressure mechanisms block a request
#[derive(Debug, Clone, Error)]
pub enum BackpressureError {
    /// Queue is at capacity, cannot accept more requests
    #[error("Queue full: capacity {capacity}, current {current}")]
    QueueFull {
        /// Maximum queue capacity
        capacity: usize,
        /// Current queue size
        current: usize,
    },

    /// Circuit breaker is open, blocking all requests
    #[error("Circuit breaker open until {until:?}")]
    CircuitOpen {
        /// When the circuit will attempt half-open
        until: Instant,
    },

    /// Request exceeded timeout
    #[error("Request timeout after {elapsed:?}")]
    RequestTimeout {
        /// How long the request waited
        elapsed: Duration,
    },
}

impl BackpressureError {
    /// Convert to JSON-RPC error response
    pub fn to_jsonrpc_error(&self, id: Option<Value>) -> Value {
        match self {
            Self::QueueFull { capacity, current } => create_jsonrpc_error(
                ERROR_SERVER_OVERLOADED,
                &format!("Queue full: capacity {capacity}, current {current}"),
                id,
            ),
            Self::CircuitOpen { until } => {
                let remaining = until.saturating_duration_since(Instant::now());
                create_jsonrpc_error(
                    ERROR_CIRCUIT_OPEN,
                    &format!("Circuit breaker open, retry in {remaining:?}"),
                    id,
                )
            }
            Self::RequestTimeout { elapsed } => create_jsonrpc_error(
                ERROR_REQUEST_TIMEOUT,
                &format!("Request timeout after {elapsed:?}"),
                id,
            ),
        }
    }
}

// === Circuit Breaker State Machine ===

/// Circuit breaker state for protecting against cascade failures
#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Normal operation, requests flow through
    Closed,

    /// Requests blocked, waiting for timeout
    Open {
        /// When to transition to `HalfOpen`
        until: Instant,
    },

    /// Testing if service recovered
    HalfOpen {
        /// Number of test requests remaining before closing
        test_requests_remaining: u32,
    },
}

impl CircuitState {
    /// Check if requests are allowed in current state
    pub fn allows_requests(&self) -> bool {
        match self {
            Self::Closed => true,
            Self::Open { until } => Instant::now() >= *until,
            Self::HalfOpen {
                test_requests_remaining,
            } => *test_requests_remaining > 0,
        }
    }
}

// === Circuit Breaker Implementation ===

/// Circuit breaker protecting against cascade failures
///
/// State transitions:
/// - Closed -> Open: After `failure_threshold` consecutive failures
/// - Open -> `HalfOpen`: After `reset_timeout` expires
/// - `HalfOpen` -> Closed: On successful request
/// - `HalfOpen` -> Open: On failed request
pub struct CircuitBreaker {
    /// Current state (protected by `RwLock` for concurrent access)
    state: RwLock<CircuitState>,

    /// Number of consecutive failures
    failure_count: AtomicU32,

    /// Failures required to open circuit
    failure_threshold: u32,

    /// Duration before attempting recovery
    reset_timeout: Duration,

    /// Test requests allowed in half-open state
    half_open_max_requests: u32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `failure_threshold` - Consecutive failures before opening (typically 3)
    /// * `reset_timeout` - Wait time before half-open (typically 30s)
    /// * `half_open_max_requests` - Test requests in half-open (typically 1)
    #[must_use]
    pub const fn new(
        failure_threshold: u32,
        reset_timeout: Duration,
        half_open_max_requests: u32,
    ) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            failure_threshold,
            reset_timeout,
            half_open_max_requests,
        }
    }

    /// Check if a request can be executed
    ///
    /// Returns `Ok(())` if allowed, `Err(BackpressureError::CircuitOpen)` if blocked
    pub fn can_execute(&self) -> Result<(), BackpressureError> {
        // First, read current state
        let should_transition_to_half_open = {
            let state = self.state.read().expect("RwLock poisoned");
            match &*state {
                CircuitState::Closed => return Ok(()),
                CircuitState::Open { until } => {
                    if Instant::now() >= *until {
                        true // Should transition to half-open
                    } else {
                        return Err(BackpressureError::CircuitOpen { until: *until });
                    }
                }
                CircuitState::HalfOpen {
                    test_requests_remaining,
                } => {
                    if *test_requests_remaining > 0 {
                        return Ok(()); // Allow test request
                    }
                    // All test requests used, must wait for result
                    let until = Instant::now() + self.reset_timeout;
                    return Err(BackpressureError::CircuitOpen { until });
                }
            }
        };

        // Transition Open -> HalfOpen if timeout expired
        if should_transition_to_half_open {
            let mut state = self.state.write().expect("RwLock poisoned");
            // Double-check we're still in Open state (another thread may have changed it)
            if matches!(&*state, CircuitState::Open { until } if Instant::now() >= *until) {
                *state = CircuitState::HalfOpen {
                    test_requests_remaining: self.half_open_max_requests,
                };
            }
        }

        // Decrement test requests if in HalfOpen
        {
            let mut state = self.state.write().expect("RwLock poisoned");
            if let CircuitState::HalfOpen {
                test_requests_remaining,
            } = &mut *state
            {
                if *test_requests_remaining > 0 {
                    *test_requests_remaining -= 1;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Record a successful request
    ///
    /// Resets failure count and transitions `HalfOpen` -> Closed
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);

        let mut state = self.state.write().expect("RwLock poisoned");
        if matches!(&*state, CircuitState::HalfOpen { .. }) {
            *state = CircuitState::Closed;
        }
    }

    /// Record a failed request
    ///
    /// Increments failure count and may transition to Open state
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

        let mut state = self.state.write().expect("RwLock poisoned");

        match &*state {
            CircuitState::Closed => {
                if failures >= self.failure_threshold {
                    *state = CircuitState::Open {
                        until: Instant::now() + self.reset_timeout,
                    };
                }
            }
            CircuitState::HalfOpen { .. } => {
                // Failed during recovery, back to Open
                *state = CircuitState::Open {
                    until: Instant::now() + self.reset_timeout,
                };
            }
            CircuitState::Open { .. } => {
                // Already open, nothing to do
            }
        }
    }

    /// Get current circuit state (for monitoring)
    pub fn state(&self) -> CircuitState {
        self.state.read().expect("RwLock poisoned").clone()
    }

    /// Force reset to Closed state (for admin override)
    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        let mut state = self.state.write().expect("RwLock poisoned");
        *state = CircuitState::Closed;
    }

    /// Get current failure count (for monitoring)
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(3, Duration::from_secs(30), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::default();
        assert!(matches!(cb.state(), CircuitState::Closed));
        assert!(cb.can_execute().is_ok());
    }

    #[test]
    fn test_failures_below_threshold_stay_closed() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(30), 1);

        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Closed));
        assert_eq!(cb.failure_count(), 1);

        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Closed));
        assert_eq!(cb.failure_count(), 2);
    }

    #[test]
    fn test_threshold_failures_opens_circuit() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(30), 1);

        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        assert!(matches!(cb.state(), CircuitState::Open { .. }));
        assert!(cb.can_execute().is_err());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(30), 1);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert!(matches!(cb.state(), CircuitState::Closed));
    }

    #[test]
    fn test_reset_timeout_allows_half_open() {
        // Use very short timeout for test
        let cb = CircuitBreaker::new(1, Duration::from_millis(10), 1);

        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Open { .. }));

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to half-open on can_execute
        assert!(cb.can_execute().is_ok());
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));
    }

    #[test]
    fn test_half_open_success_closes_circuit() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(10), 1);

        // Open the circuit
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));

        // Transition to half-open
        assert!(cb.can_execute().is_ok());
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));

        // Success closes circuit
        cb.record_success();
        assert!(matches!(cb.state(), CircuitState::Closed));
    }

    #[test]
    fn test_half_open_failure_reopens_circuit() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(10), 1);

        // Open the circuit
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));

        // Transition to half-open
        assert!(cb.can_execute().is_ok());
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));

        // Failure reopens circuit
        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Open { .. }));
    }

    #[test]
    fn test_force_reset() {
        let cb = CircuitBreaker::new(1, Duration::from_secs(30), 1);

        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Open { .. }));

        cb.reset();
        assert!(matches!(cb.state(), CircuitState::Closed));
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_open_error_contains_until() {
        let cb = CircuitBreaker::new(1, Duration::from_secs(30), 1);

        cb.record_failure();

        let err = cb.can_execute().unwrap_err();
        assert!(matches!(err, BackpressureError::CircuitOpen { .. }));
    }

    #[test]
    fn test_jsonrpc_error_creation() {
        let error =
            create_jsonrpc_error(ERROR_SERVER_OVERLOADED, "Test error", Some(Value::from(42)));

        assert_eq!(error["jsonrpc"], "2.0");
        assert_eq!(error["error"]["code"], ERROR_SERVER_OVERLOADED);
        assert_eq!(error["error"]["message"], "Test error");
        assert_eq!(error["id"], 42);
    }

    #[test]
    fn test_jsonrpc_error_with_null_id() {
        let error = create_jsonrpc_error(ERROR_CIRCUIT_OPEN, "Circuit open", None);

        assert_eq!(error["jsonrpc"], "2.0");
        assert_eq!(error["error"]["code"], ERROR_CIRCUIT_OPEN);
        assert!(error["id"].is_null());
    }

    #[test]
    fn test_backpressure_error_to_jsonrpc() {
        let queue_err = BackpressureError::QueueFull {
            capacity: 100,
            current: 100,
        };
        let json = queue_err.to_jsonrpc_error(Some(Value::from(1)));
        assert_eq!(json["error"]["code"], ERROR_SERVER_OVERLOADED);

        let timeout_err = BackpressureError::RequestTimeout {
            elapsed: Duration::from_secs(5),
        };
        let json = timeout_err.to_jsonrpc_error(Some(Value::from(2)));
        assert_eq!(json["error"]["code"], ERROR_REQUEST_TIMEOUT);
    }

    #[test]
    fn test_circuit_state_allows_requests() {
        assert!(CircuitState::Closed.allows_requests());

        let past = Instant::now() - Duration::from_secs(1);
        assert!(CircuitState::Open { until: past }.allows_requests());

        let future = Instant::now() + Duration::from_secs(60);
        assert!(!CircuitState::Open { until: future }.allows_requests());

        assert!(
            CircuitState::HalfOpen {
                test_requests_remaining: 1
            }
            .allows_requests()
        );
        assert!(
            !CircuitState::HalfOpen {
                test_requests_remaining: 0
            }
            .allows_requests()
        );
    }

    #[test]
    fn test_default_circuit_breaker() {
        let cb = CircuitBreaker::default();
        assert!(matches!(cb.state(), CircuitState::Closed));
        // Default: 3 failures to open
        cb.record_failure();
        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Closed));
        cb.record_failure();
        assert!(matches!(cb.state(), CircuitState::Open { .. }));
    }
}
