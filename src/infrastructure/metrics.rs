//! Prometheus-style metrics.
//!
//! A tiny, dependency-free registry of atomic counters plus a renderer that
//! emits the Prometheus text exposition format. No external runtime or metrics
//! crate is required: increments are lock-free (`AtomicU64`) and rendering is a
//! pure function over the current counter values.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// A Prometheus counter: a monotonically increasing 64-bit value.
#[derive(Debug)]
struct Counter {
    value: AtomicU64,
}

impl Counter {
    const fn new() -> Self {
        Counter {
            value: AtomicU64::new(0),
        }
    }

    fn incr(&self, by: u64) {
        self.value.fetch_add(by, Ordering::Relaxed);
    }

    fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// The full set of counters exposed by the framework.
#[derive(Debug)]
pub struct Metrics {
    total_events_processed: Counter,
    total_handlers_registered: Counter,
    total_errors: Counter,
    uptime_monotonic_seconds: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create a fresh registry with all counters at zero.
    pub fn new() -> Self {
        Metrics {
            total_events_processed: Counter::new(),
            total_handlers_registered: Counter::new(),
            total_errors: Counter::new(),
            uptime_monotonic_seconds: AtomicU64::new(0),
        }
    }

    /// Increment the number of events processed by the bus/projections.
    pub fn record_event_processed(&self, by: u64) {
        self.total_events_processed.incr(by);
    }

    /// Record that `count` additional handlers were registered.
    pub fn record_handlers_registered(&self, count: u64) {
        self.total_handlers_registered.incr(count);
    }

    /// Increment the number of errors observed across the framework.
    pub fn record_error(&self) {
        self.total_errors.incr(1);
    }

    /// Set the running process uptime in whole seconds. It is the embedder's
    /// responsibility to update this using a clock of their choosing.
    pub fn set_uptime_seconds(&self, seconds: u64) {
        self.uptime_monotonic_seconds.store(seconds, Ordering::Relaxed);
    }
}

impl Metrics {
    /// Render the registry in the Prometheus text exposition format.
    ///
    /// Emits `# HELP`, `# TYPE`, and counter lines for each metric.
    /// The `eventkit` namespace is applied to every metric name.
    pub fn render(&self) -> String {
        let events = self.total_events_processed.get();
        let handlers = self.total_handlers_registered.get();
        let errors = self.total_errors.get();
        let uptime = self.uptime_monotonic_seconds.load(Ordering::Relaxed);

        format!(
            "# HELP eventkit_total_events_processed Total number of events processed by the framework.\n\
             # TYPE eventkit_total_events_processed counter\n\
             eventkit_total_events_processed {events}\n\
             # HELP eventkit_total_handlers_registered Total number of handlers registered.\n\
             # TYPE eventkit_total_handlers_registered counter\n\
             eventkit_total_handlers_registered {handlers}\n\
             # HELP eventkit_total_errors Total number of errors observed by the framework.\n\
             # TYPE eventkit_total_errors counter\n\
             eventkit_total_errors {errors}\n\
             # HELP eventkit_uptime_seconds The number of seconds the process has been running.\n\
             # TYPE eventkit_uptime_seconds counter\n\
             eventkit_uptime_seconds {uptime}\n"
        )
    }
}

/// Convenience top-level function that renders a default (all-zero) registry.
pub fn render_default_metrics() -> String {
    Metrics::new().render()
}

/// Serve the rendered metrics over a plain, blocking HTTP `GET /metrics`
/// endpoint bound to `addr`.
///
/// This is a deliberately minimal, dependency-free server (`std::net::TcpListener`)
/// so the library needs no async runtime or HTTP framework. It blocks and
/// serves until the process terminates.
///
/// The uptime gauge is refreshed from elapsed wall-clock time on each request,
/// anchored to the moment the listener was bound.
pub fn serve(addr: &str, metrics: Arc<Metrics>) -> std::io::Result<()> {
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let uptime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map(|now| now.saturating_sub(started_at))
            .unwrap_or(0);
        metrics.set_uptime_seconds(uptime);

        let mut buf = [0u8; 2048];
        let _ = stream.read(&mut buf);

        let body = metrics.render();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    fn find_free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    #[test]
    fn counters_increment_independently() {
        let m = Metrics::new();
        m.record_event_processed(2);
        m.record_event_processed(3);
        m.record_handlers_registered(1);
        m.record_handlers_registered(4);
        m.record_error();

        assert_eq!(m.total_events_processed.get(), 5);
        assert_eq!(m.total_handlers_registered.get(), 5);
        assert_eq!(m.total_errors.get(), 1);
        assert_eq!(m.uptime_monotonic_seconds.load(Ordering::Relaxed), 0);
    }

    fn assert_line_matching(out: &str, expected: &str) {
        assert!(
            out.lines().any(|l| l.starts_with(expected)),
            "missing line starting with {expected:?} in:\n{out}"
        );
    }

    #[test]
    fn render_contains_help_type_and_counter_lines() {
        let m = Metrics::new();
        m.record_event_processed(1);
        m.record_handlers_registered(2);
        m.record_error();
        m.set_uptime_seconds(42);

        let out = m.render();
        assert_line_matching(&out, "# HELP eventkit_total_events_processed ");
        assert_line_matching(&out, "# TYPE eventkit_total_events_processed counter");
        assert_line_matching(&out, "eventkit_total_events_processed 1");
        assert_line_matching(&out, "# HELP eventkit_total_handlers_registered ");
        assert_line_matching(&out, "# TYPE eventkit_total_handlers_registered counter");
        assert_line_matching(&out, "eventkit_total_handlers_registered 2");
        assert_line_matching(&out, "# HELP eventkit_total_errors ");
        assert_line_matching(&out, "# TYPE eventkit_total_errors counter");
        assert_line_matching(&out, "eventkit_total_errors 1");
        assert_line_matching(&out, "# HELP eventkit_uptime_seconds ");
        assert_line_matching(&out, "# TYPE eventkit_uptime_seconds counter");
        assert_line_matching(&out, "eventkit_uptime_seconds 42");
    }

    #[test]
    fn render_default_metrics_has_all_names() {
        let out = render_default_metrics();
        for expected in [
            "eventkit_total_events_processed 0",
            "eventkit_total_handlers_registered 0",
            "eventkit_total_errors 0",
        ] {
            assert_line_matching(&out, expected);
        }
    }

    #[test]
    fn serve_responds_with_metrics() {
        let metrics = Arc::new(Metrics::new());
        metrics.record_event_processed(7);
        metrics.record_error();

        let port = find_free_port();
        let addr = format!("127.0.0.1:{port}");

        let server_metrics = metrics.clone();
        let server_addr = addr.clone();
        thread::spawn(move || {
            let _ = serve(&server_addr, server_metrics);
        });

        // Poll until the server accepts connections.
        let mut body = String::new();
        for _ in 0..50 {
            if let Ok(mut stream) = TcpStream::connect(&addr) {
                let req = "GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n";
                if stream.write_all(req.as_bytes()).is_ok() {
                    let mut buf = String::new();
                    if stream.read_to_string(&mut buf).is_ok() {
                        body = buf;
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        assert!(body.contains("eventkit_total_events_processed 7"), "body: {body}");
        assert!(body.contains("eventkit_total_errors 1"), "body: {body}");
        assert!(body.contains("# TYPE eventkit_uptime_seconds counter"), "body: {body}");
    }
}