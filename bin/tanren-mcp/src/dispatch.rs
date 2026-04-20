//! Tool-call dispatch through the compile-time registry.

use serde_json::Value;
use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseId, ToolError};

pub(crate) use super::tool_registry::CallResult;

/// Dispatch one tool call. Unknown tool names surface as
/// `ToolError::NotFound` so the client sees a typed envelope.
pub(crate) async fn dispatch(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &PhaseId,
    tool: &str,
    args: Value,
) -> CallResult {
    let Some(registration) = super::tool_registry::find(tool) else {
        return CallResult::Err(ToolError::NotFound {
            resource: "tool".into(),
            key: tool.to_owned(),
        });
    };
    (registration.dispatch)(service, scope, phase, args).await
}

/// True when the tool mutates methodology state or evidence.
#[must_use]
pub(crate) fn is_mutation_tool(tool: &str) -> bool {
    super::tool_registry::find(tool).is_some_and(|registration| registration.descriptor.mutation)
}
