# Test Tooling

Use `insta` for snapshot testing, `proptest` for property-based testing, and `wiremock` for HTTP mocking. Each tool has a specific purpose — use the right one.

**Snapshot testing with insta:**

```rust
// ✓ Good: Snapshot test for complex output
use insta::assert_yaml_snapshot;

#[test]
fn serializes_dispatch_event() {
    let event = DispatchEvent::started(agent_id, task_id);
    assert_yaml_snapshot!(event);
}
// Snapshot stored in snapshots/module__serializes_dispatch_event.snap
// Review with: cargo insta review
```

**Property-based testing with proptest:**

```rust
// ✓ Good: Roundtrip property test
use proptest::prelude::*;

proptest! {
    #[test]
    fn json_roundtrip(event in arb_event()) {
        let json = serde_json::to_string(&event).unwrap();
        let decoded: Event = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(event, decoded);
    }
}

// Define strategies for domain types
fn arb_event() -> impl Strategy<Value = Event> {
    (arb_event_kind(), any::<u64>()).prop_map(|(kind, seq)| {
        Event { kind, sequence: seq }
    })
}
```

**HTTP mocking with wiremock:**

```rust
// ✓ Good: Mock external API with wiremock
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn fetches_remote_config() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/config"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_config()))
        .mount(&server)
        .await;

    let client = ConfigClient::new(&server.uri());
    let config = client.fetch().await.unwrap();
    assert_eq!(config.version, 2);
}
```

**Rules:**
- `insta`: use for serialized output, error messages, complex struct snapshots
- `proptest`: use for parsing, serialization roundtrips, invariant validation
- `wiremock`: use for HTTP mocking — prefer over trait-mocking for HTTP clients
- `testcontainers`: use for database integration tests against real Postgres
- Review insta snapshots with `cargo insta review` before committing
- Define proptest strategies for all domain types used in property tests

**Why:** Each tool targets a specific testing need. Snapshot tests catch unexpected output changes. Property tests find edge cases humans miss. wiremock tests HTTP integration without flaky network dependencies.
