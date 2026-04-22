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

#[cfg(test)]
mod tests {
    use super::*;
    use tanren_domain::methodology::all_tool_descriptors;

    #[test]
    fn catalog_has_all_domain_tools() {
        let tools = all_tools();
        let expected = all_tool_descriptors().len();
        // Ingest/replay are CLI-only transports and are intentionally
        // absent from the MCP registry.
        assert_eq!(
            tools.len(),
            expected,
            "expected MCP registry to mirror tanren-domain tool catalog"
        );
    }

    #[test]
    fn every_tool_has_schema_and_meta() {
        for tool in all_tools() {
            assert!(
                !tool.input_schema.is_empty(),
                "{} missing schema",
                tool.name
            );
            assert!(tool.meta.is_some(), "{} missing meta", tool.name);
            assert!(
                tool.description.is_some(),
                "{} missing description",
                tool.name
            );
        }
    }

    #[test]
    fn tool_names_are_unique_and_snake_case() {
        let tools = all_tools();
        let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate tool name in catalog");
        for tool in tools {
            for c in tool.name.chars() {
                assert!(
                    c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit(),
                    "{} not snake_case",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn version_meta_advertises_v1() {
        let tool = all_tools().into_iter().next().expect("at least one tool");
        let meta = tool.meta.expect("meta");
        let ns = meta
            .0
            .get("schema_namespace")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let ver = meta
            .0
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(ns, "tanren.methodology.v1");
        assert_eq!(ver, "1.0.0");
    }

    #[test]
    fn catalog_names_match_typed_domain_tool_catalog() {
        let registry: std::collections::BTreeSet<String> = all_tools()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect();
        let domain: std::collections::BTreeSet<String> = all_tool_descriptors()
            .iter()
            .map(|descriptor| descriptor.name.to_owned())
            .collect();
        assert_eq!(
            registry, domain,
            "MCP registry names must match tanren-domain typed tool catalog"
        );
    }
}
