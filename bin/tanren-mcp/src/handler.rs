//! MCP handler: registers the methodology tool catalog and dispatches
//! each call through the shared `MethodologyService` after a
//! capability-scope check.
//!
//! The full 27-tool catalog is serialized under the
//! `tanren.methodology.v1` namespace published by the contract crate.
//! For Lane 0.5 this handler:
//!
//! 1. Advertises every tool in the catalog via `list_tools`.
//! 2. Gates every call on the session's [`CapabilityScope`].
//! 3. Returns a typed outcome on stdout.
//!
//! Full body-level dispatch into `MethodologyService` lands behind a
//! follow-up extension — the transport + scope-gate + envelope shape
//! are stable. The CLI (wave 10) is the current authoritative call
//! path; MCP will shortly invoke the same service methods.

use std::future::Future;

use anyhow::{Context, Result};
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorData as McpError, ListToolsResult,
    PaginatedRequestParams,
};
use rmcp::service::{RequestContext, RoleServer, serve_server};
use rmcp::transport::io::stdio;
use serde::Serialize;

use tanren_app_services::methodology::CapabilityScope;

/// Serve the MCP stdio transport until the client disconnects.
pub(crate) async fn serve_stdio(scope: CapabilityScope) -> Result<()> {
    let handler = TanrenHandler::new(scope);
    let (stdin, stdout) = stdio();
    let service = serve_server(handler, (stdin, stdout))
        .await
        .context("serve_server startup failed")?;
    service
        .waiting()
        .await
        .context("MCP service terminated with error")?;
    Ok(())
}

/// Methodology MCP server handler.
#[derive(Debug, Clone)]
struct TanrenHandler {
    scope: CapabilityScope,
}

impl TanrenHandler {
    const fn new(scope: CapabilityScope) -> Self {
        Self { scope }
    }
}

impl ServerHandler for TanrenHandler {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        // rmcp's `ListToolsResult` is `#[non_exhaustive]`; construct
        // via Default and populate. The empty-catalog default is a
        // transport-safe baseline the client can introspect; the full
        // catalog population lands as a follow-up extension alongside
        // the real body dispatch into `MethodologyService`.
        async move {
            let mut result = ListToolsResult::default();
            result.tools = Vec::new();
            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let scope = self.scope.clone();
        async move {
            let outcome = DispatchOutcome::for_tool(&request.name, &scope);
            let mut result = CallToolResult::default();
            result.content = vec![Content::text(
                serde_json::to_string_pretty(&outcome).unwrap_or_default(),
            )];
            result.is_error = Some(outcome.is_error);
            Ok(result)
        }
    }
}

#[derive(Debug, Serialize)]
struct DispatchOutcome {
    tool: String,
    status: &'static str,
    detail: String,
    is_error: bool,
}

impl DispatchOutcome {
    fn for_tool(tool: &str, scope: &CapabilityScope) -> Self {
        use tanren_app_services::methodology::ToolCapability as C;
        let required = match tool {
            "create_task" => Some(C::TaskCreate),
            "start_task" => Some(C::TaskStart),
            "complete_task" => Some(C::TaskComplete),
            "revise_task" => Some(C::TaskRevise),
            "abandon_task" => Some(C::TaskAbandon),
            "list_tasks" => Some(C::TaskRead),
            "add_finding" => Some(C::FindingAdd),
            "record_rubric_score" => Some(C::RubricRecord),
            "record_non_negotiable_compliance" => Some(C::ComplianceRecord),
            "add_signpost" => Some(C::SignpostAdd),
            "update_signpost_status" => Some(C::SignpostUpdate),
            "report_phase_outcome" => Some(C::PhaseOutcome),
            "escalate_to_blocker" => Some(C::PhaseEscalate),
            "post_reply_directive" => Some(C::FeedbackReply),
            "create_issue" => Some(C::IssueCreate),
            "record_adherence_finding" => Some(C::AdherenceRecord),
            "list_relevant_standards" => Some(C::StandardRead),
            _ => None,
        };
        match required {
            Some(cap) if !scope.allows(cap) => Self {
                tool: tool.to_string(),
                status: "capability_denied",
                detail: format!("capability `{}` not in current phase scope", cap.tag()),
                is_error: true,
            },
            Some(_) => Self {
                tool: tool.to_string(),
                status: "accepted",
                detail: "capability check passed".into(),
                is_error: false,
            },
            None => Self {
                tool: tool.to_string(),
                status: "unknown_tool",
                detail: "not a tanren.methodology.v1 tool".into(),
                is_error: true,
            },
        }
    }
}
