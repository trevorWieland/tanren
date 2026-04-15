//! `dispatch_projection` table — materialized view of dispatch state.
//!
//! One row per dispatch. Carries everything `StateStore::get_dispatch`
//! needs to return a dispatch view
//! without re-reading the event log. Event appends that affect a
//! dispatch update this row co-transactionally.
//!
//! `user_id` and `project` are denormalized out of the `actor` and
//! `dispatch` JSON columns so the query filters in
//! `DispatchFilter` can use plain column indexes — no JSON path
//! operators, no scans.

use sea_orm::entity::prelude::*;

use super::enums::{DispatchStatusModel, LaneModel, OutcomeModel};

/// Row shape of the `dispatch_projection` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "dispatch_projection")]
pub struct Model {
    /// Dispatch identifier — same UUID as the domain dispatch ID.
    #[sea_orm(primary_key, auto_increment = false)]
    pub dispatch_id: Uuid,

    /// Snake-case tag of the dispatch mode enum.
    pub mode: String,

    /// Snake-case tag of the dispatch status enum.
    pub status: DispatchStatusModel,

    /// Snake-case tag of the outcome enum, if
    /// the dispatch has reached a terminal state.
    pub outcome: Option<OutcomeModel>,

    /// Snake-case tag of the lane enum.
    pub lane: LaneModel,

    /// Serialized dispatch snapshot.
    /// Holds the original configuration so history queries reflect
    /// exactly what the caller asked for, even after related config
    /// has rotated.
    #[sea_orm(column_type = "JsonBinary")]
    pub dispatch: Json,

    /// Serialized actor context.
    #[sea_orm(column_type = "JsonBinary")]
    pub actor: Json,

    /// Graph revision this dispatch was planned against. Stored as
    /// `i32` because `SeaORM`'s default `u32` column type is not
    /// supported on all backends.
    pub graph_revision: i32,

    /// Denormalized `actor.user_id` for indexable queries.
    pub user_id: Uuid,

    /// Denormalized `actor.org_id` for policy-scoped reads.
    pub org_id: Option<Uuid>,

    /// Denormalized `actor.project_id` for policy-scoped reads.
    pub scope_project_id: Option<Uuid>,

    /// Denormalized `actor.team_id` for policy-scoped reads.
    pub scope_team_id: Option<Uuid>,

    /// Denormalized `actor.api_key_id` for policy-scoped reads.
    pub scope_api_key_id: Option<Uuid>,

    /// Denormalized `dispatch.project` for indexable queries and
    /// prefix/equality filtering.
    pub project: String,

    /// Wall-clock creation timestamp.
    pub created_at: DateTimeUtc,

    /// Wall-clock last-modified timestamp. Updated on every status
    /// change and outcome materialization.
    pub updated_at: DateTimeUtc,
}

/// Each dispatch has many steps; we don't declare it as a relation
/// because all step queries are keyed on `dispatch_id` directly.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
