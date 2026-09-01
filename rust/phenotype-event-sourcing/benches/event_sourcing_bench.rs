//! Benchmarks for phenotype-event-sourcing

use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phenotype_event_sourcing::{compute_hash, detect_gaps, verify_chain};
use uuid::Uuid;

fn bench_compute_hash(c: &mut Criterion) {
    let id = Uuid::new_v4();
    let timestamp = Utc::now();
    let event_type = "UserCreated";
    let payload = serde_json::json!({"user_id": "user-123", "email": "test@example.com"});
    let actor = "system";
    let zero_hash = "0".repeat(64);

    c.bench_function("compute_hash", |b| {
        b.iter(|| {
            compute_hash(
                black_box(&id),
                black_box(timestamp),
                black_box(event_type),
                black_box(&payload),
                black_box(actor),
                black_box(&zero_hash),
            )
            .unwrap()
        });
    });
}

fn bench_compute_hash_large_payload(c: &mut Criterion) {
    let id = Uuid::new_v4();
    let timestamp = Utc::now();
    let event_type = "LargeEvent";
    let payload = serde_json::json!({
        "data": "x".repeat(10000)
    });
    let actor = "system";
    let zero_hash = "0".repeat(64);

    c.bench_function("compute_hash_large_payload", |b| {
        b.iter(|| {
            compute_hash(
                black_box(&id),
                black_box(timestamp),
                black_box(event_type),
                black_box(&payload),
                black_box(actor),
                black_box(&zero_hash),
            )
            .unwrap()
        });
    });
}

fn bench_verify_chain_empty(c: &mut Criterion) {
    c.bench_function("verify_chain_empty", |b| {
        b.iter(|| verify_chain(black_box(&[] as &[(String, String)])));
    });
}

fn bench_verify_chain_single(c: &mut Criterion) {
    let zero_hash = "0".repeat(64);
    let events = vec![("abc123".to_string(), zero_hash)];
    c.bench_function("verify_chain_single", |b| {
        b.iter(|| verify_chain(black_box(&events)));
    });
}

fn bench_verify_chain_100_events(c: &mut Criterion) {
    let zero_hash = "0".repeat(64);
    let mut events = vec![("abc123".to_string(), zero_hash)];
    for i in 1..100 {
        let hash = format!("{:064x}", i);
        let prev = events.last().unwrap().0.clone();
        events.push((hash, prev));
    }
    c.bench_function("verify_chain_100_events", |b| {
        b.iter(|| verify_chain(black_box(&events)));
    });
}

fn bench_detect_gaps_no_gap(c: &mut Criterion) {
    let sequences: Vec<i64> = (1..=1000).collect();
    c.bench_function("detect_gaps_no_gap_1000", |b| {
        b.iter(|| detect_gaps(black_box(&sequences)));
    });
}

fn bench_detect_gaps_with_gap(c: &mut Criterion) {
    let mut sequences: Vec<i64> = (1..=1000).collect();
    sequences.remove(500);
    c.bench_function("detect_gaps_with_gap", |b| {
        b.iter(|| detect_gaps(black_box(&sequences)));
    });
}

fn bench_detect_gaps_empty(c: &mut Criterion) {
    let empty: Vec<i64> = vec![];
    c.bench_function("detect_gaps_empty", |b| {
        b.iter(|| detect_gaps(black_box(&empty)));
    });
}

criterion_group!(
    benches,
    bench_compute_hash,
    bench_compute_hash_large_payload,
    bench_verify_chain_empty,
    bench_verify_chain_single,
    bench_verify_chain_100_events,
    bench_detect_gaps_no_gap,
    bench_detect_gaps_with_gap,
    bench_detect_gaps_empty
);
criterion_main!(benches);
