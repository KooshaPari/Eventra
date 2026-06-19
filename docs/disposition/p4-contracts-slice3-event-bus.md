# P4 contracts slice 3 — event/bus traits (Eventra)

**Date:** 2026-06-18  
**Disposition:** D-01 slice 3  
**Source interim:** phenoShared `crates/phenotype-contracts`, HexaKit `crates/phenotype-contracts`  
**Terminal owner:** Eventra  
**Plan:** [phenotype-registry contracts-decompose-plan](https://github.com/KooshaPari/phenotype-registry/blob/main/docs/disposition/contracts-decompose-plan.md)

## Target layout

| Surface | Canonical path |
|---------|----------------|
| Core contract + domain event traits (`Contract`, `Event`) | `rust/phenotype-event-contracts/src/contract.rs`, `event.rs` |
| Publish-only bus port (`EventBus`) | `rust/phenotype-event-contracts/src/bus.rs` |
| Pub/sub ports (`PubSubBus`, `EventHandler`, `EventHandlerClone`) | `rust/phenotype-event-contracts/src/pubsub.rs` |
| Event store port (`EventStore`) | `rust/phenotype-event-contracts/src/store.rs` |
| Shared envelope types (`EventEnvelope`, `EventMetadata`) | `rust/phenotype-event-contracts/src/envelope.rs` |

## Scope

- **In:** trait-only event/bus contracts extracted from phenoShared and HexaKit outbound ports, plus Eventra pub/sub and store ports.
- **Out:** generic `MetricsHook` (remain phenoShared interim per slice 1).
- **Out:** InMemory adapters (HexaKit `phenotype-contract-adapters` — done per HexaKit#264).
- **Out:** eventkit main crate repoint to contracts crate (future slice).

## Consumer repoint

| Consumer | Action |
|----------|--------|
| Eventra `src/domain/event.rs` | Future slice: depend on / re-export contracts crate |
| HexaKit consumers of `EventBus` outbound port | Git-pin `phenotype-event-contracts` from Eventra when repointing |

## Verification

```bash
cargo check -p phenotype-event-contracts
```

## Registry

Row **#11** (`phenotype-contracts`) stays `fsm: relocating` — slice 3 partial; slice 4 (Agentora) pending.
