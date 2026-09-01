@echo off
git add -A
git commit -m "Merge branch 'task/E-E7-metrics' into main

Resolved merge conflicts keeping main's structural changes while
integrating metrics additions from task/E-E7-metrics:
- event_bus.rs: merged main's Arc handling + test structure
- projection.rs: merged main's cleaner code with metrics support
- aggregate.rs: merged main's execute method + apply logic
- infrastructure/mod.rs: added pub mod metrics from metrics branch
- adapters/event_store.rs: merged main's imports
- Cargo.lock: kept ours (regenerated on build)"
