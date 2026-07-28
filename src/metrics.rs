use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct NodeMetrics {
    finalized_height: AtomicU64,
    peers: AtomicU64,
    sync_target: AtomicU64,
    consensus_round: AtomicU64,
    signer_failures: AtomicU64,
    snapshot_retries: AtomicU64,
}

/// 운영망에서는 RPC와 별도 포트에 이 Router를 바인딩합니다.
pub fn prometheus_router(metrics: Arc<NodeMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(health_handler))
        .with_state(metrics)
}

async fn metrics_handler(State(metrics): State<Arc<NodeMetrics>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        metrics.encode_prometheus(),
    )
}

async fn health_handler() -> StatusCode {
    StatusCode::OK
}

impl NodeMetrics {
    pub fn set_finalized_height(&self, value: u64) {
        self.finalized_height.store(value, Ordering::Relaxed);
    }

    pub fn set_peers(&self, value: usize) {
        self.peers.store(value as u64, Ordering::Relaxed);
    }

    pub fn set_sync_target(&self, value: u64) {
        self.sync_target.store(value, Ordering::Relaxed);
    }

    pub fn set_consensus_round(&self, value: u32) {
        self.consensus_round.store(value as u64, Ordering::Relaxed);
    }

    pub fn inc_signer_failures(&self) {
        self.signer_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_snapshot_retries(&self) {
        self.snapshot_retries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn encode_prometheus(&self) -> String {
        [
            (
                "ieum_finalized_height",
                self.finalized_height.load(Ordering::Relaxed),
            ),
            ("ieum_peer_count", self.peers.load(Ordering::Relaxed)),
            (
                "ieum_sync_target_height",
                self.sync_target.load(Ordering::Relaxed),
            ),
            (
                "ieum_consensus_round",
                self.consensus_round.load(Ordering::Relaxed),
            ),
            (
                "ieum_signer_failures_total",
                self.signer_failures.load(Ordering::Relaxed),
            ),
            (
                "ieum_snapshot_retries_total",
                self.snapshot_retries.load(Ordering::Relaxed),
            ),
        ]
        .into_iter()
        .map(|(name, value)| format!("# TYPE {name} gauge\n{name} {value}\n"))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_prometheus_text() {
        let metrics = NodeMetrics::default();
        metrics.set_peers(4);
        assert!(metrics.encode_prometheus().contains("ieum_peer_count 4"));
    }
}
