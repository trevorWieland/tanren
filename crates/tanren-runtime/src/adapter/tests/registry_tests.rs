use std::sync::Arc;

use super::*;

#[test]
fn registry_supports_trait_object_lookup() {
    let adapter = Arc::new(MockAdapter {
        output: raw_output(),
        provider_failure: None,
        provider_run_id: None,
    });
    let mut registry = HarnessAdapterRegistry::default();
    assert!(registry.register(adapter).is_ok());
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    let Some(registered) = registry.get("mock") else {
        unreachable!("registry lookup must resolve registered adapter");
    };
    assert_eq!(registered.adapter_name(), "mock");
    assert!(registered.capabilities().can_use_tools);
}

#[test]
fn registry_rejects_duplicate_adapter_names() {
    let first = Arc::new(MockAdapter {
        output: raw_output(),
        provider_failure: None,
        provider_run_id: None,
    });
    let second = Arc::new(MockAdapter {
        output: raw_output(),
        provider_failure: None,
        provider_run_id: None,
    });
    let mut registry = HarnessAdapterRegistry::default();
    assert!(registry.register(first).is_ok());
    let duplicate = registry
        .register(second)
        .expect_err("must reject duplicate adapter name");
    assert_eq!(
        duplicate,
        HarnessAdapterRegistryError::DuplicateAdapterName { name: "mock" }
    );
}
