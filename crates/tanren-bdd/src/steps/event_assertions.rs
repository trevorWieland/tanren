//! Shared event-payload assertions for BDD step definitions.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use tanren_testkit::ActorState;

pub(crate) fn assert_account_event_payload(
    actors: &HashMap<String, ActorState>,
    kind: &str,
    payload: &Value,
) {
    assert!(
        payload.get("payload").is_some(),
        "event `{kind}` must include `payload`: {payload}"
    );
    let event_payload = payload
        .get("payload")
        .expect("payload presence asserted above");

    if kind == "active_account_switched" {
        let from = event_payload
            .get("from_account_id")
            .and_then(Value::as_str)
            .expect("active_account_switched payload missing from_account_id");
        let to = event_payload
            .get("to_account_id")
            .and_then(Value::as_str)
            .expect("active_account_switched payload missing to_account_id");
        let at = event_payload
            .get("at")
            .and_then(Value::as_str)
            .expect("active_account_switched payload missing at");
        assert_ne!(
            from, to,
            "active_account_switched must change the active account"
        );
        assert!(
            !at.trim().is_empty(),
            "active_account_switched at must be non-empty"
        );

        let known_ids = known_account_ids(actors);
        assert!(
            known_ids.contains(from),
            "active_account_switched from_account_id must be one of the actor's signed-in accounts; got {from}"
        );
        assert!(
            known_ids.contains(to),
            "active_account_switched to_account_id must be one of the actor's signed-in accounts; got {to}"
        );
    }

    if kind == "active_account_switch_rejected" {
        let reason = event_payload
            .get("reason")
            .and_then(Value::as_str)
            .expect("active_account_switch_rejected payload missing reason");
        let target = event_payload
            .get("target_account_id")
            .and_then(Value::as_str)
            .expect("active_account_switch_rejected payload missing target_account_id");
        let at = event_payload
            .get("at")
            .and_then(Value::as_str)
            .expect("active_account_switch_rejected payload missing at");
        assert_eq!(
            reason, "target_account_not_signed_in",
            "active_account_switch_rejected reason mismatch"
        );
        assert!(
            !at.trim().is_empty(),
            "active_account_switch_rejected at must be non-empty"
        );

        let known_ids = known_account_ids(actors);
        assert!(
            !known_ids.contains(target),
            "active_account_switch_rejected target_account_id should not already be signed in; got {target}"
        );
    }
}

fn known_account_ids(actors: &HashMap<String, ActorState>) -> HashSet<String> {
    let mut ids = HashSet::new();
    for actor in actors.values() {
        if let Some(session) = &actor.sign_up {
            let _ = ids.insert(session.account_id.to_string());
        }
        if let Some(session) = &actor.sign_in {
            let _ = ids.insert(session.account_id.to_string());
        }
        if let Some(accept) = &actor.accept_invitation {
            let _ = ids.insert(accept.session.account_id.to_string());
        }
    }
    ids
}
