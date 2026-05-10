use std::collections::HashMap;

use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use tanren_identity_policy::{AccountId, ValidationError};

use crate::entity;
use crate::{
    ProjectListCursor, ProjectRecord, ProjectRepositoryRecord, ProjectSetupRecord,
    ProjectStoreError, SetActiveProjectError, StoreError,
    db_constraints::is_project_repository_unique_conflict,
    db_constraints::is_projects_single_active_unique_conflict, parse_db_project_id,
};

pub(crate) fn project_cursor_filter(cursor: &ProjectListCursor) -> Condition {
    let created_at_lt = entity::projects::Column::CreatedAt.lt(cursor.created_at);
    let created_at_eq = entity::projects::Column::CreatedAt.eq(cursor.created_at);
    let project_id_lt = entity::projects::Column::Id.lt(cursor.project_id.as_uuid());

    let same_time_then_id = Condition::all()
        .add(created_at_eq.clone())
        .add(project_id_lt.clone());
    let created_or_id = Condition::any().add(created_at_lt).add(same_time_then_id);

    match cursor.active_selected_at {
        Some(active_selected_at) => {
            let active_lt = entity::projects::Column::ActiveSelectedAt.lt(active_selected_at);
            let active_eq = entity::projects::Column::ActiveSelectedAt.eq(active_selected_at);
            Condition::any()
                .add(active_lt)
                .add(entity::projects::Column::ActiveSelectedAt.is_null())
                .add(Condition::all().add(active_eq).add(created_or_id))
        }
        None => Condition::all()
            .add(entity::projects::Column::ActiveSelectedAt.is_null())
            .add(created_or_id),
    }
}

pub(crate) fn project_list_cursor_from_model(
    model: &entity::projects::Model,
) -> Result<ProjectListCursor, StoreError> {
    Ok(ProjectListCursor {
        active_selected_at: model.active_selected_at,
        created_at: model.created_at,
        project_id: parse_db_project_id(model.id, "projects.id")?,
    })
}

pub(crate) async fn load_repositories_for_projects(
    conn: &sea_orm::DatabaseConnection,
    owning_account_id: AccountId,
    project_ids: Vec<uuid::Uuid>,
) -> Result<HashMap<uuid::Uuid, entity::project_repositories::Model>, StoreError> {
    if project_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let repositories = entity::project_repositories::Entity::find()
        .filter(
            entity::project_repositories::Column::OwningAccountId.eq(owning_account_id.as_uuid()),
        )
        .filter(entity::project_repositories::Column::ProjectId.is_in(project_ids))
        .all(conn)
        .await?;
    let mut by_project_id = HashMap::with_capacity(repositories.len());
    for row in repositories {
        by_project_id.insert(row.project_id, row);
    }
    Ok(by_project_id)
}

pub(crate) fn build_project_setup_records(
    projects: Vec<entity::projects::Model>,
    mut repositories: HashMap<uuid::Uuid, entity::project_repositories::Model>,
) -> Result<Vec<ProjectSetupRecord>, StoreError> {
    let mut out = Vec::with_capacity(projects.len());
    for row in projects {
        let repository = repositories
            .remove(&row.id)
            .ok_or_else(|| StoreError::DataInvariant {
                column: "project_repositories.project_id",
                cause: ValidationError::RepositoryRefInvalid,
            })?;
        let project = ProjectRecord::try_from(row)?;
        out.push(ProjectSetupRecord {
            is_active: project.active_selected_at.is_some(),
            project,
            repository: ProjectRepositoryRecord::try_from(repository)?,
            spec_count: 0,
            milestone_count: 0,
            initiative_count: 0,
        });
    }
    Ok(out)
}

pub(crate) fn map_project_transaction_error(
    err: sea_orm::TransactionError<ProjectStoreError>,
) -> ProjectStoreError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            ProjectStoreError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

pub(crate) fn map_set_active_transaction_error(
    err: sea_orm::TransactionError<SetActiveProjectError>,
) -> SetActiveProjectError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            SetActiveProjectError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

pub(crate) fn map_project_repository_insert_error(err: sea_orm::DbErr) -> ProjectStoreError {
    if is_project_repository_unique_conflict(&err) {
        return ProjectStoreError::DuplicateRepository;
    }
    ProjectStoreError::Store(StoreError::from(err))
}

pub(crate) fn should_retry_active_selection_project_store_error(err: &ProjectStoreError) -> bool {
    match err {
        ProjectStoreError::Store(StoreError::Database(db_err)) => {
            is_projects_single_active_unique_conflict(db_err)
        }
        _ => false,
    }
}

pub(crate) fn should_retry_active_selection_set_active_error(err: &SetActiveProjectError) -> bool {
    match err {
        SetActiveProjectError::Store(StoreError::Database(db_err)) => {
            is_projects_single_active_unique_conflict(db_err)
        }
        SetActiveProjectError::Store(StoreError::DataInvariant { .. })
        | SetActiveProjectError::NoAccess
        | SetActiveProjectError::NotFound => false,
    }
}
