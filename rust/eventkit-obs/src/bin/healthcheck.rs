//! CLI healthcheck for container `HEALTHCHECK` and orchestrator probes.
//!
//! Usage:
//!   eventkit-healthcheck                    # exit 0 (library-only liveness)
//!   eventkit-healthcheck http://host:8080/health
//!   eventkit-healthcheck http://host:8080/ready

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let timeout_ms: u64 = env::var("EVENTKIT_HEALTHCHECK_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    if args.is_empty() {
        // Library-only mode: process is alive.
        let report = eventkit_obs::health::HealthReport::alive(env!("CARGO_PKG_VERSION"));
        let _ = writeln!(io::stdout(), "{}", serde_json::to_string(&report).unwrap_or_default());
        return ExitCode::SUCCESS;
    }

    let url = &args[0];
    match probe_url(url, Duration::from_millis(timeout_ms)) {
        Ok(body) => {
            let _ = writeln!(io::stdout(), "{body}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(io::stderr(), "healthcheck failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn probe_url(url: &str, timeout: Duration) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?;

    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {}", response.status()));
    }

    response
        .into_string()
        .map_err(|e| e.to_string())
}
