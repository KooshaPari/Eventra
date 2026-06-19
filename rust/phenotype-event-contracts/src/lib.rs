//! Event and event-bus contract traits for the Phenotype ecosystem.
//!
//! Terminal owner: **Eventra** (P4 contracts decompose slice 3).
//! Generic cross-cutting contracts (`MetricsHook`) remain on phenoShared interim per
//! [ADR-ECO-014](https://github.com/KooshaPari/phenotype-registry/blob/main/docs/adrs/ADR-ECO-014-phenoshared-decompose.md).

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod bus;
pub mod contract;
pub mod envelope;
pub mod error;
pub mod event;
pub mod pubsub;
pub mod store;

pub use bus::EventBus;
pub use contract::Contract;
pub use envelope::{EventEnvelope, EventMetadata};
pub use error::{EventBusError, Result};
pub use event::Event;
pub use pubsub::{EventHandler, EventHandlerClone, PubSubBus};
pub use store::EventStore;

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use crate::contract::Contract;
    use crate::envelope::EventEnvelope;
    use crate::event::Event;

    struct TestEvent {
        aggregate_id: String,
        sequence: u64,
        correlation_id: Uuid,
    }

    impl Contract for TestEvent {
        fn contract_type(&self) -> &'static str {
            "test.event"
        }

        fn timestamp(&self) -> chrono::DateTime<Utc> {
            Utc::now()
        }

        fn correlation_id(&self) -> Uuid {
            self.correlation_id
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    impl Event for TestEvent {
        fn aggregate_id(&self) -> &str {
            &self.aggregate_id
        }

        fn sequence(&self) -> u64 {
            self.sequence
        }
    }

    #[test]
    fn event_trait_exposes_aggregate_metadata() {
        let event = TestEvent {
            aggregate_id: "agg-1".to_string(),
            sequence: 7,
            correlation_id: Uuid::new_v4(),
        };

        assert_eq!(event.aggregate_id(), "agg-1");
        assert_eq!(event.sequence(), 7);
        assert_eq!(event.contract_type(), "test.event");
    }

    #[test]
    fn envelope_carries_metadata_and_payload() {
        let envelope = EventEnvelope::new(
            "agg-1",
            "order",
            "order.created",
            1,
            serde_json::json!({"total": 42}),
        );

        assert_eq!(envelope.metadata.aggregate_id, "agg-1");
        assert_eq!(envelope.metadata.event_type, "order.created");
        assert_eq!(envelope.payload["total"], 42);
    }
}
