//! Lean list path for the [`StateStore`](crate::StateStore) trait.
//!
//! Implements [`StateStore::query_dispatch_summaries`] against the
//! `dispatch_projection` table by issuing a scalar-only `SELECT`.
//! Crucially this path **never decodes** the `dispatch` or `actor`
//! JSON columns, which is the hot-path performance gap identified in
//! lane-0.4 audit finding 4.

use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, QuerySelect, sea_query::Expr};
use tanren_domain::DispatchSummary;

use crate::converters::dispatch_summary::{SummaryRow, tuple_to_summary};
use crate::entity::dispatch_projection;
use crate::errors::StoreResult;
use crate::params::{DispatchCursor, DispatchFilter, DispatchSummaryQueryPage};
use crate::state_store::{apply_common_dispatch_filters, apply_scoped_dispatch_filter};

pub(crate) async fn query_dispatch_summaries_impl(
    conn: &DatabaseConnection,
    filter: &DispatchFilter,
) -> StoreResult<DispatchSummaryQueryPage> {
    let page_size = filter.limit.min(crate::params::MAX_DISPATCH_QUERY_LIMIT);
    if page_size == 0 {
        return Ok(DispatchSummaryQueryPage {
            summaries: Vec::new(),
            next_cursor: None,
        });
    }

    let rows = build_summary_query(filter, page_size).all(conn).await?;
    build_summary_page(rows, page_size)
}

fn build_summary_query(
    filter: &DispatchFilter,
    page_size: u64,
) -> sea_orm::Selector<sea_orm::SelectGetableTuple<SummaryRow>> {
    let mut query = apply_common_dispatch_filters(dispatch_projection::Entity::find(), filter);
    if let Some(scope) = filter.read_scope {
        query = apply_scoped_dispatch_filter(query, scope);
    }

    query
        .select_only()
        .column(dispatch_projection::Column::DispatchId)
        .column(dispatch_projection::Column::Mode)
        .column(dispatch_projection::Column::Status)
        .column(dispatch_projection::Column::Outcome)
        .column(dispatch_projection::Column::Lane)
        .column(dispatch_projection::Column::Project)
        .column(dispatch_projection::Column::CreatedAt)
        .column(dispatch_projection::Column::UpdatedAt)
        .order_by_desc(Expr::col(dispatch_projection::Column::CreatedAt))
        .order_by_desc(Expr::col(dispatch_projection::Column::DispatchId))
        .limit(page_size.saturating_add(1))
        .into_tuple::<SummaryRow>()
}

fn build_summary_page(
    mut rows: Vec<SummaryRow>,
    page_size: u64,
) -> StoreResult<DispatchSummaryQueryPage> {
    let page_size_usize = usize::try_from(page_size).unwrap_or(usize::MAX);
    let has_more = rows.len() > page_size_usize;
    if has_more {
        rows.truncate(page_size_usize);
    }

    let next_cursor = rows.last().map(|row| DispatchCursor {
        created_at: row.6,
        dispatch_id: tanren_domain::DispatchId::from_uuid(row.0),
    });

    let mut summaries: Vec<DispatchSummary> = Vec::with_capacity(rows.len());
    for row in rows {
        summaries.push(tuple_to_summary(row)?);
    }

    Ok(DispatchSummaryQueryPage {
        summaries,
        next_cursor: if has_more { next_cursor } else { None },
    })
}
