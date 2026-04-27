//! MCP catalog view derived from the compile-time tool registry.

use rmcp::model::Tool;

/// Build the full tool catalog in stable registry order.
#[must_use]
pub(crate) fn all_tools() -> Vec<Tool> {
    super::tool_registry::all()
        .iter()
        .map(super::tool_registry::as_rmcp_tool)
        .collect()
}
