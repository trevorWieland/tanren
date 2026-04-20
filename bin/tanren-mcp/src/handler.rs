//! MCP handler: advertises the `tanren.methodology.v1` tool catalog
//! and dispatches every call through the shared
//! `MethodologyService` after a capability-scope check.
//!
//! Both `list_tools` and `call_tool` are wired end-to-end: the
//! catalog is the compile-time `super::catalog::all_tools()` list;
//! `call_tool` deserializes the caller's `arguments` into the
//! contract params type, invokes the matching service method, and
//! serializes the typed response or `ToolError` back.
//!
//! Phase + capability scope are derived from the verified
//! `TANREN_MCP_CAPABILITY_ENVELOPE` claims at startup.

use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result};
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorData as McpError, ListToolsResult,
    PaginatedRequestParams,
};
use rmcp::service::{RequestContext, RoleServer, serve_server};
use rmcp::transport::io::stdio;
use serde_json::Value;

use tanren_app_services::methodology::{
    CapabilityScope, MethodologyService, PhaseId, enter_mutation_session, finalize_mutation_session,
};

use super::{catalog, dispatch};

/// Serve the MCP stdio transport until the client disconnects.
pub(crate) async fn serve_stdio(
    scope: CapabilityScope,
    service: Arc<MethodologyService>,
    phase: PhaseId,
) -> Result<()> {
    let handler = TanrenHandler::new(scope, service, phase);
    let (stdin, stdout) = stdio();
    let server = serve_server(handler, (stdin, stdout))
        .await
        .context("serve_server startup failed")?;
    server
        .waiting()
        .await
        .context("MCP service terminated with error")?;
    Ok(())
}

/// Methodology MCP server handler. Holds the active capability
/// scope, a shared service handle, and the phase name used for
/// capability enforcement + audit trail.
#[derive(Debug, Clone)]
struct TanrenHandler {
    scope: CapabilityScope,
    service: Arc<MethodologyService>,
    phase: PhaseId,
}

impl TanrenHandler {
    fn new(scope: CapabilityScope, service: Arc<MethodologyService>, phase: PhaseId) -> Self {
        Self {
            scope,
            service,
            phase,
        }
    }
}

impl ServerHandler for TanrenHandler {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let mut result = ListToolsResult::default();
            result.tools = catalog::all_tools();
            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let scope = self.scope.clone();
        let service = self.service.clone();
        let phase = self.phase.clone();
        async move {
            let args: Value = request
                .arguments
                .map_or(Value::Object(serde_json::Map::new()), Value::Object);
            let mut session: Option<(
                tanren_app_services::methodology::service::PhaseEventsRuntime,
                Option<tanren_app_services::methodology::EnforcementGuard>,
            )> = None;
            if dispatch::is_mutation_tool(&request.name) {
                let Some(runtime) = service.phase_events_runtime() else {
                    let mut result = CallToolResult::default();
                    let outcome = dispatch::CallResult::Err(
                        tanren_app_services::methodology::ToolError::ValidationFailed {
                            field_path: "/spec_id".into(),
                            expected:
                                "audited runtime requires TANREN_SPEC_ID + TANREN_SPEC_FOLDER"
                                    .into(),
                            actual: "missing".into(),
                            remediation:
                                "set TANREN_SPEC_ID and TANREN_SPEC_FOLDER for mutating tools"
                                    .into(),
                        },
                    );
                    result.content = vec![Content::text(outcome.to_json())];
                    result.is_error = Some(true);
                    return Ok(result);
                };
                match enter_mutation_session(&runtime.spec_folder) {
                    Ok(guard) => session = Some((runtime, guard)),
                    Err(err) => {
                        let mut result = CallToolResult::default();
                        let outcome = dispatch::CallResult::Err((&err).into());
                        result.content = vec![Content::text(outcome.to_json())];
                        result.is_error = Some(true);
                        return Ok(result);
                    }
                }
            }
            let mut outcome =
                dispatch::dispatch(service.as_ref(), &scope, &phase, &request.name, args).await;
            if let Some((runtime, guard)) = session
                && let Err(err) = finalize_mutation_session(
                    service.as_ref(),
                    &phase,
                    runtime.spec_id,
                    &runtime.spec_folder,
                    &runtime.agent_session_id,
                    guard,
                )
                .await
                && !outcome.is_error()
            {
                outcome = dispatch::CallResult::Err((&err).into());
            }
            let mut result = CallToolResult::default();
            result.content = vec![Content::text(outcome.to_json())];
            result.is_error = Some(outcome.is_error());
            Ok(result)
        }
    }
}
