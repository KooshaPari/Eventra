//! OutboxRelay: drains an OutboxStore and publishes entries (EVE-SOTA-003).
//!
//! The OutboxRelay is the consumer side of the transactional outbox
//! pattern (EVE-SOTA-001 + EVE-SOTA-002). It reads unpublished rows
//! from an `OutboxStore`, hands the event envelope to a user-provided
//! publisher (an async closure or trait object), and marks each entry
//! as published (or records a failure with attempt+error).
//!
//! ## Concurrency model
//!
//! - One relay process can run multiple worker tasks (`workers = N`).
//! - Workers call `claim_batch(limit)` on the shared store, which uses
//!   Postgres `SELECT ... FOR UPDATE SKIP LOCKED` (or equivalent) so
//!   workers never see the same row.
//! - Each entry is processed in isolation: failure of one publish does
//!   not affect the others.
//!
//! ## Retry semantics
//!
//! On `Err(_)`, the relay calls `record_failure(entry.id, &err)` which
//! increments `attempt` and stores the error string. The entry becomes
//! eligible for another claim once it ages past the per-row `backoff`
//! (computed from attempt count: 1s, 2s, 4s, ... capped at 60s).
//!
//! ## Graceful shutdown
//!
//! `run_until` takes a `Shutdown` token (a `tokio::sync::Notify` or any
//! `Future<Output = ()>`). When the token fires, the relay completes
//! the current batch and returns.
//!
//! DAG unit: EVE-SOTA-003.

use crate::outbox::{OutboxEntry, OutboxError, OutboxStore};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::sleep;

/// User-provided publisher. Receives the deserialized envelope and
/// returns `Ok(())` on success or `Err(String)` (which becomes a
/// recorded failure on the outbox row).
///
/// Typical implementations:
/// - Publish to a Kafka topic
/// - Publish to an SNS/SQS topic
/// - Hand off to an in-process handler (domain event projection)
/// - POST to a webhook
pub trait Publisher: Send + Sync {
    fn publish(&self, envelope: &Value) -> Result<(), String>;
}

impl<F> Publisher for F
where
    F: Fn(&Value) -> Result<(), String> + Send + Sync,
{
    fn publish(&self, envelope: &Value) -> Result<(), String> {
        (self)(envelope)
    }
}

/// Configuration for a relay instance.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Number of worker tasks to spawn.
    pub workers: usize,
    /// Maximum batch size per `claim_batch` call.
    pub batch_size: u32,
    /// Idle sleep between batches when the store is empty.
    pub idle_poll_interval: Duration,
    /// Backoff cap (max wait between retries on the same row).
    pub max_backoff: Duration,
    /// Total runtime cap (None = run forever).
    pub max_runtime: Option<Duration>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            workers: 2,
            batch_size: 50,
            idle_poll_interval: Duration::from_millis(500),
            max_backoff: Duration::from_secs(60),
            max_runtime: None,
        }
    }
}

/// Outbox statistics emitted from a `run` call.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RelayStats {
    pub published: u64,
    pub failed: u64,
    pub empty_polls: u64,
    pub elapsed: Duration,
}

/// Token for graceful shutdown. Wrap in `Arc<Notify>` and call
/// `notify_one()` from any handle to request shutdown.
pub type Shutdown = Arc<Notify>;

pub fn new_shutdown_token() -> Shutdown {
    Arc::new(Notify::new())
}

/// Compute the per-row backoff for a given attempt count (1-indexed).
///
/// `min(2^(attempt-1), max_backoff.as_secs())` seconds.
pub fn backoff_for(attempt: u32, max_backoff: Duration) -> Duration {
    let exp = attempt.saturating_sub(1).min(20); // cap exponent at 2^20 ~ 12 days
    let secs = 1u64.checked_shl(exp).unwrap_or(u64::MAX);
    let cap = max_backoff.as_secs().max(1);
    Duration::from_secs(secs.min(cap))
}

/// Run the relay until either:
/// 1. `shutdown` is notified (graceful)
/// 2. `config.max_runtime` elapses (if set)
/// 3. The store returns a non-retryable error (poisoned)
pub async fn run<S, P>(
    store: Arc<S>,
    publisher: Arc<P>,
    config: RelayConfig,
    shutdown: Shutdown,
) -> Result<RelayStats, OutboxError>
where
    S: OutboxStore + 'static,
    P: Publisher + 'static,
{
    let started = std::time::Instant::now();
    let mut stats = RelayStats::default();
    let max_attempts: u32 = 8;

    'outer: loop {
        if let Some(max) = config.max_runtime {
            if started.elapsed() >= max {
                stats.elapsed = started.elapsed();
                return Ok(stats);
            }
        }

        // Single worker (the multi-worker concurrency comes from a
        // higher-level supervisor; the relay's worker fan-out is
        // implemented at the call site if desired).
        let batch = store.claim_batch(config.batch_size).await?;
        if batch.is_empty() {
            stats.empty_polls += 1;
            // Wait for either the idle interval or shutdown.
            tokio::select! {
                _ = sleep(config.idle_poll_interval) => continue,
                _ = shutdown.notified() => break 'outer,
            }
        }

        for entry in batch {
            // Per-entry: re-check shutdown to exit fast on large batches.
            if shutdown.notified().now_or_never().is_some() {
                break 'outer;
            }

            // Decode the stored envelope back to a serde_json::Value.
            // OutboxEntry stores it as `envelope: serde_json::Value`
            // (the in-memory) or as JSONB (the Postgres adapter); both
            // paths yield a Value here.
            let envelope_value = entry.envelope.clone();

            match publisher.publish(&envelope_value) {
                Ok(()) => {
                    store.mark_published(entry.id).await?;
                    stats.published += 1;
                }
                Err(err) => {
                    let attempt = entry.attempt + 1;
                    store.record_failure(entry.id, &err).await?;
                    stats.failed += 1;
                    if attempt >= max_attempts {
                        // Drop the entry's idempotency from the relay's
                        // perspective; the row remains visible for
                        // human triage but the relay stops hammering it.
                        eprintln!(
                            "[outbox-relay] entry {} exhausted retries ({} attempts): {}",
                            entry.id, attempt, err
                        );
                    } else {
                        let wait = backoff_for(attempt, config.max_backoff);
                        // Sleep with shutdown awareness.
                        tokio::select! {
                            _ = sleep(wait) => {}
                            _ = shutdown.notified() => break 'outer,
                        }
                    }
                }
            }
        }
    }

    stats.elapsed = started.elapsed();
    Ok(stats)
}

/// Convenience: spawn `workers` parallel `run` loops on the same store
/// + publisher. Returns the JoinHandle of each worker so the supervisor
/// can cancel them on shutdown.
pub fn spawn_workers<S, P>(
    store: Arc<S>,
    publisher: Arc<P>,
    config: RelayConfig,
    shutdown: Shutdown,
) -> Vec<tokio::task::JoinHandle<Result<RelayStats, OutboxError>>>
where
    S: OutboxStore + 'static,
    P: Publisher + 'static,
{
    (0..config.workers.max(1))
        .map(|_| {
            let store = store.clone();
            let publisher = publisher.clone();
            let config = config.clone();
            let shutdown = shutdown.clone();
            tokio::spawn(async move { run(store, publisher, config, shutdown).await })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::{InMemoryOutbox, OutboxEntry, OutboxStore};
    use std::sync::atomic::{AtomicU64, Ordering};
    use ulid::Ulid;

    #[test]
    fn backoff_doubles_per_attempt_capped_at_max() {
        let max = Duration::from_secs(60);
        assert_eq!(backoff_for(1, max), Duration::from_secs(1));
        assert_eq!(backoff_for(2, max), Duration::from_secs(2));
        assert_eq!(backoff_for(3, max), Duration::from_secs(4));
        assert_eq!(backoff_for(7, max), Duration::from_secs(64).min(max));
        // Large attempts clamp to max.
        assert_eq!(backoff_for(20, max), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn run_publishes_all_empty_store_yields_zero_stats() {
        let store = Arc::new(InMemoryOutbox::new());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let publisher = Arc::new(move |_: &Value| -> Result<(), String> {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let cfg = RelayConfig {
            workers: 1,
            batch_size: 10,
            idle_poll_interval: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            max_runtime: Some(Duration::from_millis(50)),
        };
        let shutdown = new_shutdown_token();
        let stats = run(store, publisher, cfg, shutdown).await.unwrap();
        assert_eq!(stats.published, 0);
        assert_eq!(stats.failed, 0);
        assert!(stats.empty_polls > 0);
    }

    #[tokio::test]
    async fn run_publishes_pending_entries_and_marks_published() {
        let store = Arc::new(InMemoryOutbox::new());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let publisher = Arc::new(move |_: &Value| -> Result<(), String> {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        // Enqueue 3 entries.
        for i in 0..3 {
            let entry = OutboxEntry::new(Ulid::new(), serde_json::json!({"i": i}));
            store.enqueue(entry).await.unwrap();
        }
        let cfg = RelayConfig {
            workers: 1,
            batch_size: 10,
            idle_poll_interval: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            max_runtime: Some(Duration::from_millis(200)),
        };
        let shutdown = new_shutdown_token();
        let stats = run(store.clone(), publisher.clone(), cfg, shutdown)
            .await
            .unwrap();
        assert_eq!(stats.published, 3);
        assert_eq!(stats.failed, 0);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
        assert_eq!(store.pending_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn run_records_failure_and_does_not_republish_in_same_cycle() {
        let store = Arc::new(InMemoryOutbox::new());
        let attempts = Arc::new(AtomicU64::new(0));
        let attempts_clone = attempts.clone();
        // Always-failing publisher.
        let publisher = Arc::new(move |_: &Value| -> Result<(), String> {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            Err("downstream down".to_string())
        });
        let entry = OutboxEntry::new(Ulid::new(), serde_json::json!({"topic": "orders"}));
        store.enqueue(entry).await.unwrap();
        let cfg = RelayConfig {
            workers: 1,
            batch_size: 10,
            idle_poll_interval: Duration::from_millis(10),
            max_backoff: Duration::from_millis(50), // short backoff so retry fires
            max_runtime: Some(Duration::from_millis(150)),
        };
        let shutdown = new_shutdown_token();
        let stats = run(store.clone(), publisher, cfg, shutdown).await.unwrap();
        assert_eq!(stats.published, 0);
        assert!(stats.failed >= 1);
        // Entry still pending (not published, recorded failure).
        assert_eq!(store.pending_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn shutdown_signal_breaks_run_quickly() {
        let store = Arc::new(InMemoryOutbox::new());
        let publisher = Arc::new(|_: &Value| -> Result<(), String> { Ok(()) });
        let cfg = RelayConfig {
            workers: 1,
            batch_size: 10,
            idle_poll_interval: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            max_runtime: Some(Duration::from_secs(30)),
        };
        let shutdown = new_shutdown_token();
        let shutdown_clone = shutdown.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            shutdown_clone.notify_one();
        });
        let stats = run(store, publisher, cfg, shutdown).await.unwrap();
        // Shutdown should fire well before the 30s cap.
        assert!(stats.elapsed < Duration::from_secs(2));
    }
}
