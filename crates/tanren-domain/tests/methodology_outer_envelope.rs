//! Guardrail: `DomainEvent::Methodology(MethodologyEvent::...)` must
//! round-trip through JSON cleanly. Serde's internally-tagged enum with
//! a newtype variant wrapping another internally-tagged enum can
//! produce adjacency collisions; this test pins the observable shape.

use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::{MethodologyEvent, TaskStarted};
use tanren_domain::{SpecId, TaskId};

#[test]
fn outer_envelope_methodology_roundtrip() {
    let ev = DomainEvent::Methodology {
        event: MethodologyEvent::TaskStarted(TaskStarted {
            task_id: TaskId::new(),
            spec_id: SpecId::new(),
        }),
    };
    let json = serde_json::to_string(&ev).expect("serialize");
    let back: DomainEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ev, back);
}

#[test]
fn outer_envelope_top_tag_is_methodology() {
    let ev = DomainEvent::Methodology {
        event: MethodologyEvent::TaskStarted(TaskStarted {
            task_id: TaskId::new(),
            spec_id: SpecId::new(),
        }),
    };
    let v = serde_json::to_value(&ev).expect("serialize");
    let top_tag = v
        .get("event_type")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    assert_eq!(top_tag.as_deref(), Some("methodology"));
    // Nested discriminant is still present under the "event" key.
    let inner_tag = v
        .get("event")
        .and_then(|e| e.get("event_type"))
        .and_then(|x| x.as_str())
        .map(str::to_string);
    assert_eq!(inner_tag.as_deref(), Some("task_started"));
}
