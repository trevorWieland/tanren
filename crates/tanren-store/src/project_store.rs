//! `SeaORM`-backed implementation of project setup/listing persistence.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, EntityTrait, Order, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::NullOrdering,
};
use tanren_identity_policy::{AccountId, ProjectId, ProviderFamily, RepositoryRef};

use crate::entity;
use crate::{
    NewProject, NewProjectRepository, ProjectCommandReservation, ProjectCommandReservationResult,
    ProjectListCursor, ProjectListPage, ProjectRecord, ProjectRepositoryRecord, ProjectSetupRecord,
    ProjectStore, ProjectStoreError, SetActiveProjectError, Store, StoreError, parse_db_project_id,
    project_reservations,
    project_store_helpers::{
        build_project_setup_records, load_repositories_for_projects,
        map_project_repository_insert_error, map_project_transaction_error,
        map_set_active_transaction_error, project_cursor_filter, project_list_cursor_from_model,
        should_retry_active_selection_project_store_error,
        should_retry_active_selection_set_active_error,
    },
};

const ACTIVE_SELECTION_RETRY_LIMIT: usize = 3;

#[async_trait]
impl ProjectStore for Store {
    async fn account_exists(&self, account_id: AccountId) -> Result<bool, StoreError> {
        let row = entity::accounts::Entity::find_by_id(account_id.as_uuid())
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    async fn reserve_project_command(
        &self,
        owning_account_id: AccountId,
        provider_family: &ProviderFamily,
        repository_ref: &RepositoryRef,
        now: DateTime<Utc>,
    ) -> Result<ProjectCommandReservationResult, StoreError> {
        project_reservations::reserve_project_command(
            &self.conn,
            owning_account_id,
            provider_family,
            repository_ref,
            now,
        )
        .await
    }

    async fn finalize_project_command_reservation(
        &self,
        reservation: &ProjectCommandReservation,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        project_reservations::finalize_project_command_reservation(&self.conn, reservation, now)
            .await
    }

    async fn fail_project_command_reservation(
        &self,
        reservation: &ProjectCommandReservation,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        project_reservations::fail_project_command_reservation(&self.conn, reservation, now).await
    }

    async fn insert_project(&self, new: NewProject) -> Result<ProjectRecord, StoreError> {
        let model = entity::projects::ActiveModel {
            id: Set(new.id.as_uuid()),
            owning_account_id: Set(new.owning_account_id.as_uuid()),
            created_at: Set(new.created_at),
            active_selected_at: Set(new.active_selected_at),
            active_selection_guard: Set(new.active_selected_at.map(|_| true)),
        };
        let inserted = model.insert(&self.conn).await?;
        ProjectRecord::try_from(inserted)
    }

    async fn insert_project_repository(
        &self,
        new: NewProjectRepository,
    ) -> Result<ProjectRepositoryRecord, ProjectStoreError> {
        let model = entity::project_repositories::ActiveModel {
            project_id: Set(new.project_id.as_uuid()),
            owning_account_id: Set(new.owning_account_id.as_uuid()),
            repository_ref: Set(new.repository_ref.as_str().to_owned()),
            provider_family: Set(new.provider_family.as_str().to_owned()),
            designated_host: Set(new.designated_host.as_str().to_owned()),
            created_at: Set(new.created_at),
        };
        let inserted = model
            .insert(&self.conn)
            .await
            .map_err(map_project_repository_insert_error)?;
        ProjectRepositoryRecord::try_from(inserted).map_err(ProjectStoreError::Store)
    }

    async fn create_project_setup(
        &self,
        project: NewProject,
        repository: NewProjectRepository,
        select_as_active: bool,
    ) -> Result<ProjectSetupRecord, ProjectStoreError> {
        let mut attempt = 0usize;
        loop {
            let result = self
                .conn
                .transaction::<_, ProjectSetupRecord, ProjectStoreError>(|txn| {
                    let project = project.clone();
                    let repository = repository.clone();
                    Box::pin(async move {
                        let inserted_project = entity::projects::ActiveModel {
                            id: Set(project.id.as_uuid()),
                            owning_account_id: Set(project.owning_account_id.as_uuid()),
                            created_at: Set(project.created_at),
                            active_selected_at: Set(None),
                            active_selection_guard: Set(None),
                        }
                        .insert(txn)
                        .await
                        .map_err(StoreError::from)
                        .map_err(ProjectStoreError::Store)?;

                        let inserted_repository = (entity::project_repositories::ActiveModel {
                            project_id: Set(repository.project_id.as_uuid()),
                            owning_account_id: Set(repository.owning_account_id.as_uuid()),
                            repository_ref: Set(repository.repository_ref.as_str().to_owned()),
                            provider_family: Set(repository.provider_family.as_str().to_owned()),
                            designated_host: Set(repository.designated_host.as_str().to_owned()),
                            created_at: Set(repository.created_at),
                        })
                        .insert(txn)
                        .await
                        .map_err(map_project_repository_insert_error)?;

                        let selected_at = if select_as_active {
                            update_active_project_for_account(
                                txn,
                                project.owning_account_id.as_uuid(),
                                project.id.as_uuid(),
                                project.created_at,
                            )
                            .await
                            .map_err(StoreError::from)
                            .map_err(ProjectStoreError::Store)?;
                            Some(project.created_at)
                        } else {
                            None
                        };

                        let project_record = ProjectRecord {
                            id: parse_db_project_id(inserted_project.id, "projects.id")
                                .map_err(ProjectStoreError::Store)?,
                            owning_account_id: AccountId::new(inserted_project.owning_account_id),
                            created_at: inserted_project.created_at,
                            active_selected_at: selected_at,
                        };
                        let repository_record =
                            ProjectRepositoryRecord::try_from(inserted_repository)
                                .map_err(ProjectStoreError::Store)?;

                        Ok(ProjectSetupRecord {
                            is_active: selected_at.is_some(),
                            project: project_record,
                            repository: repository_record,
                            spec_count: 0,
                            milestone_count: 0,
                            initiative_count: 0,
                        })
                    })
                })
                .await;

            match result {
                Ok(setup) => return Ok(setup),
                Err(err) => {
                    let mapped = map_project_transaction_error(err);
                    if should_retry_active_selection_project_store_error(&mapped)
                        && attempt + 1 < ACTIVE_SELECTION_RETRY_LIMIT
                    {
                        attempt += 1;
                        continue;
                    }
                    return Err(mapped);
                }
            }
        }
    }

    async fn find_project_repository(
        &self,
        owning_account_id: AccountId,
        provider_family: &ProviderFamily,
        repository_ref: &RepositoryRef,
    ) -> Result<Option<ProjectRepositoryRecord>, StoreError> {
        let row = entity::project_repositories::Entity::find()
            .filter(
                entity::project_repositories::Column::OwningAccountId
                    .eq(owning_account_id.as_uuid()),
            )
            .filter(
                entity::project_repositories::Column::ProviderFamily.eq(provider_family.as_str()),
            )
            .filter(entity::project_repositories::Column::RepositoryRef.eq(repository_ref.as_str()))
            .one(&self.conn)
            .await?;
        row.map(ProjectRepositoryRecord::try_from).transpose()
    }

    async fn list_projects_for_account(
        &self,
        owning_account_id: AccountId,
        page_size: u16,
        cursor: Option<&ProjectListCursor>,
    ) -> Result<ProjectListPage, StoreError> {
        let limit = u64::from(page_size) + 1;
        let mut query = entity::projects::Entity::find()
            .filter(entity::projects::Column::OwningAccountId.eq(owning_account_id.as_uuid()));
        if let Some(cursor) = cursor {
            query = query.filter(project_cursor_filter(cursor));
        }

        let mut project_rows = query
            .order_by_with_nulls(
                entity::projects::Column::ActiveSelectedAt,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_desc(entity::projects::Column::CreatedAt)
            .order_by_desc(entity::projects::Column::Id)
            .limit(limit)
            .all(&self.conn)
            .await?;

        let has_more = project_rows.len() > usize::from(page_size);
        if has_more {
            project_rows.truncate(usize::from(page_size));
        }
        let as_of = project_rows.first().map(|row| row.created_at);
        let next_cursor = if has_more {
            project_rows
                .last()
                .map(project_list_cursor_from_model)
                .transpose()?
        } else {
            None
        };

        let repositories = load_repositories_for_projects(
            &self.conn,
            owning_account_id,
            project_rows.iter().map(|row| row.id).collect(),
        )
        .await?;
        let projects = build_project_setup_records(project_rows, repositories)?;

        Ok(ProjectListPage {
            projects,
            page_size,
            has_more,
            next_cursor,
            as_of,
        })
    }

    async fn active_project_for_account(
        &self,
        owning_account_id: AccountId,
    ) -> Result<Option<ProjectSetupRecord>, StoreError> {
        let Some(project_row) = entity::projects::Entity::find()
            .filter(entity::projects::Column::OwningAccountId.eq(owning_account_id.as_uuid()))
            .filter(entity::projects::Column::ActiveSelectedAt.is_not_null())
            .order_by_with_nulls(
                entity::projects::Column::ActiveSelectedAt,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_desc(entity::projects::Column::CreatedAt)
            .order_by_desc(entity::projects::Column::Id)
            .one(&self.conn)
            .await?
        else {
            return Ok(None);
        };

        let repositories =
            load_repositories_for_projects(&self.conn, owning_account_id, vec![project_row.id])
                .await?;
        let mut records = build_project_setup_records(vec![project_row], repositories)?;
        Ok(records.pop())
    }

    async fn set_active_project(
        &self,
        owning_account_id: AccountId,
        project_id: ProjectId,
        selected_at: DateTime<Utc>,
    ) -> Result<(), SetActiveProjectError> {
        let mut attempt = 0usize;
        loop {
            let result = self
                .conn
                .transaction::<_, (), SetActiveProjectError>(|txn| {
                    Box::pin(async move {
                        let Some(selected_project) =
                            entity::projects::Entity::find_by_id(project_id.as_uuid())
                                .one(txn)
                                .await
                                .map_err(StoreError::from)
                                .map_err(SetActiveProjectError::Store)?
                        else {
                            return Err(SetActiveProjectError::NotFound);
                        };

                        if selected_project.owning_account_id != owning_account_id.as_uuid() {
                            return Err(SetActiveProjectError::NoAccess);
                        }

                        update_active_project_for_account(
                            txn,
                            owning_account_id.as_uuid(),
                            project_id.as_uuid(),
                            selected_at,
                        )
                        .await
                        .map_err(StoreError::from)
                        .map_err(SetActiveProjectError::Store)?;
                        Ok(())
                    })
                })
                .await;

            match result {
                Ok(()) => return Ok(()),
                Err(err) => {
                    let mapped = map_set_active_transaction_error(err);
                    if should_retry_active_selection_set_active_error(&mapped)
                        && attempt + 1 < ACTIVE_SELECTION_RETRY_LIMIT
                    {
                        attempt += 1;
                        continue;
                    }
                    return Err(mapped);
                }
            }
        }
    }
}

async fn update_active_project_for_account(
    txn: &DatabaseTransaction,
    owning_account_id: uuid::Uuid,
    selected_project_id: uuid::Uuid,
    selected_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    entity::projects::Entity::update_many()
        .col_expr(
            entity::projects::Column::ActiveSelectedAt,
            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
        )
        .col_expr(
            entity::projects::Column::ActiveSelectionGuard,
            sea_orm::sea_query::Expr::value(None::<bool>),
        )
        .filter(entity::projects::Column::OwningAccountId.eq(owning_account_id))
        .filter(
            Condition::any()
                .add(entity::projects::Column::ActiveSelectedAt.is_not_null())
                .add(entity::projects::Column::ActiveSelectionGuard.is_not_null()),
        )
        .exec(txn)
        .await?;

    entity::projects::Entity::update_many()
        .col_expr(
            entity::projects::Column::ActiveSelectedAt,
            sea_orm::sea_query::Expr::value(Some(selected_at)),
        )
        .col_expr(
            entity::projects::Column::ActiveSelectionGuard,
            sea_orm::sea_query::Expr::value(Some(true)),
        )
        .filter(entity::projects::Column::OwningAccountId.eq(owning_account_id))
        .filter(entity::projects::Column::Id.eq(selected_project_id))
        .exec(txn)
        .await?;

    Ok(())
}
