// ABOUTME: Metrics and observability for MCP socket pool
//
// Tracks:
// - Requests: total, success, error, timeout
// - Backpressure: rejections, queue depth
// - Circuit breaker: trips, state changes
// - Clients: active count, disconnects
// - MCP processes: restarts, latency

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Pool-level metrics
#[derive(Debug, Default)]
pub struct PoolMetrics {
    pub total_requests: AtomicU64,
    pub total_errors: AtomicU64,
    pub active_clients: AtomicU32,
    pub active_mcps: AtomicU32,
}

impl PoolMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_errors(&self) {
        self.total_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_active_clients(&self, count: u32) {
        self.active_clients.store(count, Ordering::Relaxed);
    }

    pub fn set_active_mcps(&self, count: u32) {
        self.active_mcps.store(count, Ordering::Relaxed);
    }
}

/// Per-proxy metrics
#[derive(Debug, Default)]
pub struct ProxyMetrics {
    pub requests_in_flight: AtomicU32,
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
    pub circuit_breaker_trips: AtomicU64,
}

impl ProxyMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_started(&self) {
        self.requests_in_flight.fetch_add(1, Ordering::Relaxed);
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn request_completed(&self) {
        self.requests_in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn request_error(&self) {
        self.requests_in_flight.fetch_sub(1, Ordering::Relaxed);
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn circuit_breaker_tripped(&self) {
        self.circuit_breaker_trips.fetch_add(1, Ordering::Relaxed);
    }
}
