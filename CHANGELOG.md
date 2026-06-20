# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- `PersistentEventStore`: file-backed `EventStore` implementation with
  append-only JSONL log, per-aggregate version checkpoints, and optimistic
  concurrency control. See `docs/adapter-migration.md` for the intended
  PostgreSQL schema and migration path.
- `PersistentEventBus`: file-backed `EventBus` implementation using the
  outbox pattern with per-subscriber offset persistence. Documents the
  intended Kafka/RabbitMQ migration path.
- `CheckpointedProjectionRunner` and `CheckpointStore` trait:
  `FileCheckpointStore` persists projection positions to disk so
  `ProjectionRunner` no longer has to replay the full event log on every
  start. A `PostgresCheckpointStore` can be added later behind the same
  trait.
- `docs/adapter-migration.md`: adapter migration guide covering the
  in-memory → persistent → Postgres/Kafka/RabbitMQ upgrade path.
