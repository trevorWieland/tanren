//! Dispatch service — bridges contract types to orchestrator operations.
//!
//! This is the stable use-case API consumed by all transport interfaces
//! (CLI, API, MCP, TUI). It maps contract request/response types to and
//! from the domain, translating errors into wire-safe [`ErrorResponse`].

use tanren_contract::{
    CancelDispatchRequest, ContractError, CreateDispatchRequest, DispatchListFilter,
    DispatchListResponse, DispatchResponse, ErrorResponse,
};
use tanren_domain::{CreateDispatch, DispatchId};
use tanren_orchestrator::Orchestrator;
use tanren_store::{
    DispatchCursor, DispatchFilter, EventStore, JobQueue, MAX_DISPATCH_QUERY_LIMIT, StateStore,
};
use uuid::Uuid;

use crate::error::map_orchestrator_error;

/// Application service for dispatch operations.
///
/// Generic over `S` — the store implementation. Trait bounds are on the
/// impl block, not the struct definition.
#[derive(Debug)]
pub struct DispatchService<S> {
    orchestrator: Orchestrator<S>,
}

impl<S> DispatchService<S>
where
    S: EventStore + JobQueue + StateStore,
{
    /// Create a new dispatch service wrapping the given orchestrator.
    pub fn new(orchestrator: Orchestrator<S>) -> Self {
        Self { orchestrator }
    }

    /// Borrow the underlying orchestrator.
    pub fn orchestrator(&self) -> &Orchestrator<S> {
        &self.orchestrator
    }

    /// Create a new dispatch from a contract request.
    pub async fn create(
        &self,
        req: CreateDispatchRequest,
    ) -> Result<DispatchResponse, ErrorResponse> {
        let cmd = CreateDispatch::try_from(req).map_err(ErrorResponse::from)?;
        let view = self
            .orchestrator
            .create_dispatch(cmd)
            .await
            .map_err(map_orchestrator_error)?;
        Ok(DispatchResponse::from(view))
    }

    /// Get a dispatch by its UUID.
    pub async fn get(&self, dispatch_id: Uuid) -> Result<DispatchResponse, ErrorResponse> {
        let id = DispatchId::from_uuid(dispatch_id);
        let view = self
            .orchestrator
            .get_dispatch(&id)
            .await
            .map_err(map_orchestrator_error)?;
        match view {
            Some(v) => Ok(DispatchResponse::from(v)),
            None => Err(ErrorResponse::from(ContractError::NotFound {
                entity: "dispatch".to_owned(),
                id: dispatch_id.to_string(),
            })),
        }
    }

    /// List dispatches matching the given filter.
    pub async fn list(
        &self,
        filter: DispatchListFilter,
    ) -> Result<DispatchListResponse, ErrorResponse> {
        let store_filter = convert_list_filter(filter)?;
        let views = self
            .orchestrator
            .list_dispatches(store_filter)
            .await
            .map_err(map_orchestrator_error)?;
        Ok(DispatchListResponse {
            dispatches: views
                .dispatches
                .into_iter()
                .map(DispatchResponse::from)
                .collect(),
            next_cursor: views.next_cursor.map(format_cursor),
        })
    }

    /// Cancel a dispatch.
    pub async fn cancel(&self, req: CancelDispatchRequest) -> Result<(), ErrorResponse> {
        let cmd = tanren_domain::CancelDispatch::try_from(req).map_err(ErrorResponse::from)?;
        self.orchestrator
            .cancel_dispatch(cmd)
            .await
            .map_err(map_orchestrator_error)
    }
}

/// Convert a contract list filter to a store filter.
fn convert_list_filter(filter: DispatchListFilter) -> Result<DispatchFilter, ErrorResponse> {
    let mut f = DispatchFilter::new();
    f.status = filter.status.map(Into::into);
    f.lane = filter.lane.map(Into::into);
    f.project = filter.project;
    if let Some(limit) = filter.limit {
        if limit == 0 {
            return Err(ErrorResponse::from(ContractError::InvalidField {
                field: "limit".to_owned(),
                reason: "must be >= 1".to_owned(),
            }));
        }
        if limit > MAX_DISPATCH_QUERY_LIMIT {
            return Err(ErrorResponse::from(ContractError::InvalidField {
                field: "limit".to_owned(),
                reason: format!("must be <= {MAX_DISPATCH_QUERY_LIMIT} (received {limit})",),
            }));
        }
        f.limit = limit;
    }
    if let Some(cursor) = filter.cursor {
        f.cursor = Some(parse_cursor(&cursor)?);
    }
    Ok(f)
}

fn format_cursor(cursor: DispatchCursor) -> String {
    format!(
        "{}|{}",
        cursor.created_at.to_rfc3339(),
        cursor.dispatch_id.into_uuid()
    )
}

fn parse_cursor(raw: &str) -> Result<DispatchCursor, ErrorResponse> {
    let (created_at_raw, dispatch_id_raw) = raw.split_once('|').ok_or_else(|| {
        ErrorResponse::from(ContractError::InvalidField {
            field: "cursor".to_owned(),
            reason: "invalid cursor format".to_owned(),
        })
    })?;

    let created_at = chrono::DateTime::parse_from_rfc3339(created_at_raw)
        .map_err(|_| {
            ErrorResponse::from(ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: "invalid cursor timestamp".to_owned(),
            })
        })?
        .with_timezone(&chrono::Utc);
    let dispatch_id = Uuid::parse_str(dispatch_id_raw).map_err(|_| {
        ErrorResponse::from(ContractError::InvalidField {
            field: "cursor".to_owned(),
            reason: "invalid cursor dispatch_id".to_owned(),
        })
    })?;

    Ok(DispatchCursor {
        created_at,
        dispatch_id: DispatchId::from_uuid(dispatch_id),
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tanren_contract::DispatchListFilter;

    use super::{convert_list_filter, format_cursor, parse_cursor};

    #[test]
    fn convert_list_filter_rejects_limit_above_max() {
        let filter = DispatchListFilter {
            limit: Some(tanren_store::MAX_DISPATCH_QUERY_LIMIT + 1),
            ..DispatchListFilter::default()
        };
        let err = convert_list_filter(filter).expect_err("should fail");
        assert_eq!(err.code, "invalid_input");
        assert!(err.message.contains("limit"));
    }

    #[test]
    fn convert_list_filter_rejects_zero_limit() {
        let filter = DispatchListFilter {
            limit: Some(0),
            ..DispatchListFilter::default()
        };
        let err = convert_list_filter(filter).expect_err("should fail");
        assert_eq!(err.code, "invalid_input");
        assert!(err.message.contains("limit"));
    }

    #[test]
    fn cursor_roundtrip_is_stable() {
        let cursor = tanren_store::DispatchCursor {
            created_at: Utc::now(),
            dispatch_id: tanren_domain::DispatchId::new(),
        };
        let encoded = format_cursor(cursor);
        let decoded = parse_cursor(&encoded).expect("decode");
        assert_eq!(decoded, cursor);
    }

    #[test]
    fn parse_cursor_rejects_bad_format() {
        let err = parse_cursor("bad").expect_err("should fail");
        assert_eq!(err.code, "invalid_input");
        assert!(err.message.contains("cursor"));
    }
}
