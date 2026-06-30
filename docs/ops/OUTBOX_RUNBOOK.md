# Outbox Relay — SLO, Runbook & Postmortem Guide

Covers `phenotype-event-bus` crate: `OutboxStore`, `OutboxRelay`, and the
SQLite/Postgres persistence backends (feature flags `sqlite`, `postgres`).

---

## SLO Targets

| Signal | Target | Measurement window |
|---|---|---|
| Relay drain latency (p99) | < 5 s from write to publish | 1-hour rolling |
| Failed delivery rate | < 0.1 % of outbox rows | 24-hour rolling |
| Unrecoverable (max-attempts) rows | < 10 per day | 24-hour rolling |
| Relay process uptime | > 99.9 % | 30-day rolling |

These are aspirational targets for the PoC/alpha phase. Operators must
instrument their publisher integration to measure actual latency.

---

## Architecture recap

```
[Domain write] ──► [OutboxStore.push()] ──► [DB row: status=pending]
                                                   │
                                     [OutboxRelay worker]
                                           │  claim_batch()
                                           │  publisher.publish(envelope)
                                           │  mark_published() or record_failure()
                                           ▼
                                    [Downstream consumer]
```

The relay uses **jittered exponential backoff** for failed rows:
`backoff_ms = min(60_000, 1_000 * 2^attempt) + jitter(0..1_000)`.

---

## Failure modes and remediation

### 1. Relay process crash / pod restart

**Symptom:** Gap in published events; relay metrics show no activity.

**Root cause:** Normal pod lifecycle or OOM. Pending rows in the outbox survive
the crash because they are committed to the DB before the relay processes them.

**Remediation:**
1. Restart the relay process (K8s restarts automatically under the Deployment
   controller — see `deploy/k8s/deployment.yaml`).
2. After restart, `claim_batch()` re-picks rows whose `claimed_until` has expired
   (default lease TTL: 30 s). No manual intervention needed.
3. Verify by querying: `SELECT count(*) FROM outbox WHERE status = 'pending';`

**Prevention:** Set `resources.limits.memory` conservatively and monitor OOM
kills via `kubectl describe pod`.

---

### 2. Publisher returns persistent errors

**Symptom:** `outbox.failures` counter rising; rows stuck at `status=failed`
with increasing `attempt` count.

**Root cause:** Downstream consumer is down, misconfigured, or rejecting the
event schema.

**Remediation:**
1. Fix the downstream consumer.
2. Reset failed rows so the relay retries them:
   ```sql
   UPDATE outbox SET status = 'pending', attempt = 0, last_error = NULL
   WHERE status = 'failed' AND attempt < 10;
   ```
3. If rows have reached `max_attempts` they are left in `status=failed`
   permanently. Review and replay individually after fixing the consumer.

**Prevention:** Add a dead-letter table or alerting on `attempt >= 5`.

---

### 3. Relay drain latency spike

**Symptom:** p99 drain latency exceeds SLO; backlog of pending rows growing.

**Root cause:** Slow publisher, under-provisioned workers, or DB lock contention
(`SELECT ... FOR UPDATE SKIP LOCKED` on a hot table).

**Remediation:**
1. Increase `RelayConfig::workers` (default: 2).
2. Reduce `RelayConfig::batch_size` to lower per-batch lock hold time.
3. Add a DB index on `(status, created_at)` if not present.
4. Scale horizontally — multiple relay instances can share the same outbox table
   via `SKIP LOCKED`-based claiming.

---

### 4. Unbounded outbox table growth

**Symptom:** DB disk usage growing; query performance degrading.

**Root cause:** Published rows are not pruned.

**Remediation:**
```sql
-- Prune rows published more than 7 days ago
DELETE FROM outbox WHERE status = 'published' AND created_at < NOW() - INTERVAL '7 days';
```

**Prevention:** Schedule a daily pruning job. A `pg_partman`-based time
partition on `created_at` is the long-term solution.

---

### 5. SQLite WAL file growing unboundedly (SQLite backend only)

**Symptom:** `*.db-wal` file consuming disk space.

**Root cause:** WAL checkpoint not triggered when connections are idle.

**Remediation:**
```sql
PRAGMA wal_checkpoint(TRUNCATE);
```

**Prevention:** Configure `PRAGMA journal_size_limit = 67108864;` (64 MB cap)
on connection open.

---

## Postmortem template

```
## Postmortem — Outbox Delivery Failure

**Date:**
**Duration:**
**Impact (rows undelivered, services affected):**

### Timeline
- HH:MM  Event observed
- HH:MM  Root cause identified
- HH:MM  Remediation applied
- HH:MM  Recovery confirmed

### Root cause
_One paragraph._

### Contributing factors
- ...

### What went well
- ...

### Corrective actions
| Action | Owner | Due |
|---|---|---|
| ... | ... | ... |
```
