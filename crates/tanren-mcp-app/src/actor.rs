//! MCP actor-context resolution for project-tool authorization.
//!
//! The MCP surface derives its actor from configured capability context
//! (bootstrap API key + `TANREN_MCP_ACCOUNT_ID`), not from caller-supplied
//! tool parameters. This module resolves the configured account id into an
//! [`ActorContext`] that the project tools use for typed policy decisions.

use std::env;

use tanren_app_services::Store;
use tanren_identity_policy::AccountId;
use tanren_policy::ActorContext;

const ACCOUNT_ID_ENV: &str = "TANREN_MCP_ACCOUNT_ID";

pub(crate) async fn resolve_serve_actor_context(database_url: &str) -> Option<ActorContext> {
    let raw = env::var(ACCOUNT_ID_ENV).ok().filter(|s| !s.is_empty())?;
    let uuid = match uuid::Uuid::parse_str(&raw) {
        Ok(u) => u,
        Err(err) => {
            tracing::warn!(
                target: "tanren_mcp",
                env_var = ACCOUNT_ID_ENV,
                error = %err,
                "TANREN_MCP_ACCOUNT_ID is not a valid UUID — MCP project tools will be unavailable"
            );
            return None;
        }
    };
    let account_id = AccountId::new(uuid);
    match Store::connect(database_url).await {
        Ok(store) => {
            match <Store as tanren_store::AccountStore>::find_account_by_id(&store, account_id)
                .await
            {
                Ok(Some(record)) => {
                    tracing::info!(
                        target: "tanren_mcp",
                        account_id = %account_id,
                        org_id = ?record.org_id,
                        "MCP actor context resolved from TANREN_MCP_ACCOUNT_ID"
                    );
                    Some(ActorContext {
                        account_id,
                        org: record.org_id,
                    })
                }
                Ok(None) => {
                    tracing::warn!(
                        target: "tanren_mcp",
                        account_id = %account_id,
                        "TANREN_MCP_ACCOUNT_ID does not match any account — MCP project tools will be unavailable"
                    );
                    None
                }
                Err(err) => {
                    tracing::warn!(
                        target: "tanren_mcp",
                        error = %err,
                        "Failed to look up MCP actor account — project tools will be unavailable"
                    );
                    None
                }
            }
        }
        Err(err) => {
            tracing::warn!(
                target: "tanren_mcp",
                error = %err,
                "Failed to connect to store for actor resolution — project tools will be unavailable"
            );
            None
        }
    }
}
