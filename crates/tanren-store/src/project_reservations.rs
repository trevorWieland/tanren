//! Durable reservation helpers for project create/connect commands.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, DbErr, EntityTrait, QueryFilter, Set};
use tanren_identity_policy::{AccountId, ProjectId, ProviderFamily, RepositoryRef};
use uuid::Uuid;

use crate::db_constraints::is_project_command_reservation_unique_conflict;
use crate::entity;
use crate::{ProjectCommandReservation, ProjectCommandReservationResult, StoreError};

const RESERVATION_STATUS_PENDING: &str = "pending";
const RESERVATION_STATUS_SUCCEEDED: &str = "succeeded";
const RESERVATION_STATUS_FAILED: &str = "failed";
const RESERVATION_LEASE_SECONDS: i64 = 90;
const RESERVATION_RATE_LIMIT_THRESHOLD: i32 = 3;
const RESERVATION_RATE_LIMIT_SECONDS: i64 = 60;
const RESERVATION_UPDATE_RETRY_LIMIT: usize = 3;

enum ReservationStatus {
    Pending,
    Succeeded,
    Failed,
}

impl ReservationStatus {
    fn parse(raw: &str) -> Result<Self, StoreError> {
        match raw {
            RESERVATION_STATUS_PENDING => Ok(Self::Pending),
            RESERVATION_STATUS_SUCCEEDED => Ok(Self::Succeeded),
            RESERVATION_STATUS_FAILED => Ok(Self::Failed),
            other => Err(StoreError::Database(DbErr::Custom(format!(
                "invalid project reservation status {other}"
            )))),
        }
    }
}

pub(crate) async fn reserve_project_command(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
    now: DateTime<Utc>,
) -> Result<ProjectCommandReservationResult, StoreError> {
    if has_registered_project(conn, owning_account_id, provider_family, repository_ref).await? {
        persist_duplicate_reservation(
            conn,
            owning_account_id,
            provider_family,
            repository_ref,
            now,
        )
        .await?;
        return Ok(ProjectCommandReservationResult::DuplicateRepository);
    }

    if let Some(acquired) = try_insert_pending_reservation(
        conn,
        owning_account_id,
        provider_family,
        repository_ref,
        now,
    )
    .await?
    {
        return Ok(ProjectCommandReservationResult::Acquired(acquired));
    }

    for _ in 0..RESERVATION_UPDATE_RETRY_LIMIT {
        let Some(existing) =
            load_existing_reservation(conn, owning_account_id, provider_family, repository_ref)
                .await?
        else {
            continue;
        };

        if existing.blocked_until.is_some_and(|until| until > now) {
            return Ok(ProjectCommandReservationResult::RateLimited);
        }

        match ReservationStatus::parse(&existing.status)? {
            ReservationStatus::Succeeded => {
                return Ok(ProjectCommandReservationResult::DuplicateRepository);
            }
            ReservationStatus::Pending
                if existing.lease_expires_at.is_some_and(|until| until > now) =>
            {
                return Ok(ProjectCommandReservationResult::InFlight);
            }
            ReservationStatus::Pending | ReservationStatus::Failed => {
                if let Some(acquired) = try_claim_existing_reservation(
                    conn,
                    owning_account_id,
                    provider_family,
                    repository_ref,
                    &existing,
                    now,
                )
                .await?
                {
                    return Ok(ProjectCommandReservationResult::Acquired(acquired));
                }
            }
        }
    }

    Ok(ProjectCommandReservationResult::InFlight)
}

async fn has_registered_project(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
) -> Result<bool, StoreError> {
    let found = entity::project_repositories::Entity::find()
        .filter(
            entity::project_repositories::Column::OwningAccountId.eq(owning_account_id.as_uuid()),
        )
        .filter(entity::project_repositories::Column::ProviderFamily.eq(provider_family.as_str()))
        .filter(entity::project_repositories::Column::RepositoryRef.eq(repository_ref.as_str()))
        .one(conn)
        .await?;
    Ok(found.is_some())
}

async fn try_insert_pending_reservation(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
    now: DateTime<Utc>,
) -> Result<Option<ProjectCommandReservation>, StoreError> {
    let reservation = new_reservation(owning_account_id, provider_family, repository_ref);
    let lease_expires_at = reservation_lease_expires_at(now);
    let row = entity::project_command_reservations::ActiveModel {
        owning_account_id: Set(owning_account_id.as_uuid()),
        provider_family: Set(provider_family.as_str().to_owned()),
        repository_ref: Set(repository_ref.as_str().to_owned()),
        status: Set(RESERVATION_STATUS_PENDING.to_owned()),
        active_reservation_id: Set(Some(reservation.reservation_id.as_uuid())),
        reserved_at: Set(Some(now)),
        lease_expires_at: Set(Some(lease_expires_at)),
        failure_count: Set(0),
        blocked_until: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };
    match row.insert(conn).await {
        Ok(_) => Ok(Some(reservation)),
        Err(err) if is_project_command_reservation_unique_conflict(&err) => Ok(None),
        Err(err) => Err(StoreError::from(err)),
    }
}

async fn load_existing_reservation(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
) -> Result<Option<entity::project_command_reservations::Model>, StoreError> {
    entity::project_command_reservations::Entity::find_by_id((
        owning_account_id.as_uuid(),
        provider_family.as_str().to_owned(),
        repository_ref.as_str().to_owned(),
    ))
    .one(conn)
    .await
    .map_err(StoreError::from)
}

async fn try_claim_existing_reservation(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
    existing: &entity::project_command_reservations::Model,
    now: DateTime<Utc>,
) -> Result<Option<ProjectCommandReservation>, StoreError> {
    let reservation = new_reservation(owning_account_id, provider_family, repository_ref);
    let lease_expires_at = reservation_lease_expires_at(now);
    let claimed = entity::project_command_reservations::Entity::update_many()
        .col_expr(
            entity::project_command_reservations::Column::Status,
            sea_orm::sea_query::Expr::value(RESERVATION_STATUS_PENDING.to_owned()),
        )
        .col_expr(
            entity::project_command_reservations::Column::ActiveReservationId,
            sea_orm::sea_query::Expr::value(Some(reservation.reservation_id.as_uuid())),
        )
        .col_expr(
            entity::project_command_reservations::Column::ReservedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .col_expr(
            entity::project_command_reservations::Column::LeaseExpiresAt,
            sea_orm::sea_query::Expr::value(Some(lease_expires_at)),
        )
        .col_expr(
            entity::project_command_reservations::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(
            entity::project_command_reservations::Column::OwningAccountId
                .eq(owning_account_id.as_uuid()),
        )
        .filter(
            entity::project_command_reservations::Column::ProviderFamily
                .eq(provider_family.as_str()),
        )
        .filter(
            entity::project_command_reservations::Column::RepositoryRef.eq(repository_ref.as_str()),
        )
        .filter(entity::project_command_reservations::Column::UpdatedAt.eq(existing.updated_at))
        .filter(
            Condition::all()
                .add(
                    Condition::any()
                        .add(entity::project_command_reservations::Column::BlockedUntil.is_null())
                        .add(entity::project_command_reservations::Column::BlockedUntil.lte(now)),
                )
                .add(
                    Condition::any()
                        .add(
                            entity::project_command_reservations::Column::Status
                                .eq(RESERVATION_STATUS_FAILED),
                        )
                        .add(
                            Condition::all()
                                .add(
                                    entity::project_command_reservations::Column::Status
                                        .eq(RESERVATION_STATUS_PENDING),
                                )
                                .add(
                                    entity::project_command_reservations::Column::LeaseExpiresAt
                                        .lte(now),
                                ),
                        ),
                ),
        )
        .exec(conn)
        .await?;
    if claimed.rows_affected == 1 {
        Ok(Some(reservation))
    } else {
        Ok(None)
    }
}

pub(crate) async fn finalize_project_command_reservation(
    conn: &sea_orm::DatabaseConnection,
    reservation: &ProjectCommandReservation,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let result = entity::project_command_reservations::Entity::update_many()
        .col_expr(
            entity::project_command_reservations::Column::Status,
            sea_orm::sea_query::Expr::value(RESERVATION_STATUS_SUCCEEDED.to_owned()),
        )
        .col_expr(
            entity::project_command_reservations::Column::ActiveReservationId,
            sea_orm::sea_query::Expr::value(None::<Uuid>),
        )
        .col_expr(
            entity::project_command_reservations::Column::ReservedAt,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::project_command_reservations::Column::LeaseExpiresAt,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::project_command_reservations::Column::FailureCount,
            sea_orm::sea_query::Expr::value(0),
        )
        .col_expr(
            entity::project_command_reservations::Column::BlockedUntil,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::project_command_reservations::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(
            entity::project_command_reservations::Column::OwningAccountId
                .eq(reservation.owning_account_id.as_uuid()),
        )
        .filter(
            entity::project_command_reservations::Column::ProviderFamily
                .eq(reservation.provider_family.as_str()),
        )
        .filter(
            entity::project_command_reservations::Column::RepositoryRef
                .eq(reservation.repository_ref.as_str()),
        )
        .filter(
            entity::project_command_reservations::Column::ActiveReservationId
                .eq(reservation.reservation_id.as_uuid()),
        )
        .exec(conn)
        .await?;
    if result.rows_affected != 1 {
        return Err(StoreError::Database(DbErr::Custom(
            "project command reservation finalize lost ownership".to_owned(),
        )));
    }
    Ok(())
}

pub(crate) async fn fail_project_command_reservation(
    conn: &sea_orm::DatabaseConnection,
    reservation: &ProjectCommandReservation,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let Some(existing) = entity::project_command_reservations::Entity::find_by_id((
        reservation.owning_account_id.as_uuid(),
        reservation.provider_family.as_str().to_owned(),
        reservation.repository_ref.as_str().to_owned(),
    ))
    .one(conn)
    .await?
    else {
        return Err(StoreError::Database(DbErr::Custom(
            "project command reservation missing during failure mark".to_owned(),
        )));
    };

    if existing.active_reservation_id != Some(reservation.reservation_id.as_uuid()) {
        return Err(StoreError::Database(DbErr::Custom(
            "project command reservation failure mark lost ownership".to_owned(),
        )));
    }

    let failure_count = existing.failure_count.saturating_add(1);
    let blocked_until = if failure_count >= RESERVATION_RATE_LIMIT_THRESHOLD {
        Some(now + Duration::seconds(RESERVATION_RATE_LIMIT_SECONDS))
    } else {
        None
    };
    let result = entity::project_command_reservations::Entity::update_many()
        .col_expr(
            entity::project_command_reservations::Column::Status,
            sea_orm::sea_query::Expr::value(RESERVATION_STATUS_FAILED.to_owned()),
        )
        .col_expr(
            entity::project_command_reservations::Column::ActiveReservationId,
            sea_orm::sea_query::Expr::value(None::<Uuid>),
        )
        .col_expr(
            entity::project_command_reservations::Column::ReservedAt,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::project_command_reservations::Column::LeaseExpiresAt,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::project_command_reservations::Column::FailureCount,
            sea_orm::sea_query::Expr::value(failure_count),
        )
        .col_expr(
            entity::project_command_reservations::Column::BlockedUntil,
            sea_orm::sea_query::Expr::value(blocked_until),
        )
        .col_expr(
            entity::project_command_reservations::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(
            entity::project_command_reservations::Column::OwningAccountId
                .eq(reservation.owning_account_id.as_uuid()),
        )
        .filter(
            entity::project_command_reservations::Column::ProviderFamily
                .eq(reservation.provider_family.as_str()),
        )
        .filter(
            entity::project_command_reservations::Column::RepositoryRef
                .eq(reservation.repository_ref.as_str()),
        )
        .filter(
            entity::project_command_reservations::Column::ActiveReservationId
                .eq(reservation.reservation_id.as_uuid()),
        )
        .exec(conn)
        .await?;
    if result.rows_affected != 1 {
        return Err(StoreError::Database(DbErr::Custom(
            "project command reservation failure mark update lost ownership".to_owned(),
        )));
    }
    Ok(())
}

fn reservation_lease_expires_at(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::seconds(RESERVATION_LEASE_SECONDS)
}

fn new_reservation(
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
) -> ProjectCommandReservation {
    ProjectCommandReservation {
        reservation_id: ProjectId::fresh(),
        owning_account_id,
        provider_family: provider_family.clone(),
        repository_ref: repository_ref.clone(),
    }
}

async fn persist_duplicate_reservation(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository_ref: &RepositoryRef,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let row = entity::project_command_reservations::ActiveModel {
        owning_account_id: Set(owning_account_id.as_uuid()),
        provider_family: Set(provider_family.as_str().to_owned()),
        repository_ref: Set(repository_ref.as_str().to_owned()),
        status: Set(RESERVATION_STATUS_SUCCEEDED.to_owned()),
        active_reservation_id: Set(None),
        reserved_at: Set(None),
        lease_expires_at: Set(None),
        failure_count: Set(0),
        blocked_until: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };
    match row.insert(conn).await {
        Ok(_) => Ok(()),
        Err(err) if is_project_command_reservation_unique_conflict(&err) => {
            entity::project_command_reservations::Entity::update_many()
                .col_expr(
                    entity::project_command_reservations::Column::Status,
                    sea_orm::sea_query::Expr::value(RESERVATION_STATUS_SUCCEEDED.to_owned()),
                )
                .col_expr(
                    entity::project_command_reservations::Column::ActiveReservationId,
                    sea_orm::sea_query::Expr::value(None::<Uuid>),
                )
                .col_expr(
                    entity::project_command_reservations::Column::ReservedAt,
                    sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
                )
                .col_expr(
                    entity::project_command_reservations::Column::LeaseExpiresAt,
                    sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
                )
                .col_expr(
                    entity::project_command_reservations::Column::FailureCount,
                    sea_orm::sea_query::Expr::value(0),
                )
                .col_expr(
                    entity::project_command_reservations::Column::BlockedUntil,
                    sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
                )
                .col_expr(
                    entity::project_command_reservations::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(
                    entity::project_command_reservations::Column::OwningAccountId
                        .eq(owning_account_id.as_uuid()),
                )
                .filter(
                    entity::project_command_reservations::Column::ProviderFamily
                        .eq(provider_family.as_str()),
                )
                .filter(
                    entity::project_command_reservations::Column::RepositoryRef
                        .eq(repository_ref.as_str()),
                )
                .exec(conn)
                .await?;
            Ok(())
        }
        Err(err) => Err(StoreError::from(err)),
    }
}
