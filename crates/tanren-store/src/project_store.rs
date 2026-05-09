//! `SeaORM`-backed implementation of project setup/listing persistence.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use tanren_identity_policy::{
    AccountId, ProjectId, ProviderFamily, RepositoryRef, ValidationError,
};

use crate::entity;
use crate::{
    NewProject, NewProjectRepository, ProjectRecord, ProjectRepositoryRecord, ProjectSetupRecord,
    ProjectStore, ProjectStoreError, Store, StoreError, parse_db_project_id,
};

#[async_trait]
impl ProjectStore for Store {
    async fn account_exists(&self, account_id: AccountId) -> Result<bool, StoreError> {
        let row = entity::accounts::Entity::find_by_id(account_id.as_uuid())
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    async fn insert_project(&self, new: NewProject) -> Result<ProjectRecord, StoreError> {
        let model = entity::projects::ActiveModel {
            id: Set(new.id.as_uuid()),
            owning_account_id: Set(new.owning_account_id.as_uuid()),
            created_at: Set(new.created_at),
            active_selected_at: Set(new.active_selected_at),
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
        self.conn
            .transaction::<_, ProjectSetupRecord, ProjectStoreError>(|txn| {
                let project = project.clone();
                let repository = repository.clone();
                Box::pin(async move {
                    let inserted_project = entity::projects::ActiveModel {
                        id: Set(project.id.as_uuid()),
                        owning_account_id: Set(project.owning_account_id.as_uuid()),
                        created_at: Set(project.created_at),
                        active_selected_at: Set(None),
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
                        entity::projects::Entity::update_many()
                            .col_expr(
                                entity::projects::Column::ActiveSelectedAt,
                                sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
                            )
                            .filter(
                                entity::projects::Column::OwningAccountId
                                    .eq(project.owning_account_id.as_uuid()),
                            )
                            .filter(entity::projects::Column::Id.ne(project.id.as_uuid()))
                            .exec(txn)
                            .await
                            .map_err(StoreError::from)
                            .map_err(ProjectStoreError::Store)?;
                        entity::projects::Entity::update_many()
                            .col_expr(
                                entity::projects::Column::ActiveSelectedAt,
                                sea_orm::sea_query::Expr::value(Some(project.created_at)),
                            )
                            .filter(
                                entity::projects::Column::OwningAccountId
                                    .eq(project.owning_account_id.as_uuid()),
                            )
                            .filter(entity::projects::Column::Id.eq(project.id.as_uuid()))
                            .exec(txn)
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
                    let repository_record = ProjectRepositoryRecord::try_from(inserted_repository)
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
            .await
            .map_err(map_project_transaction_error)
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
    ) -> Result<Vec<ProjectSetupRecord>, StoreError> {
        let projects = entity::projects::Entity::find()
            .filter(entity::projects::Column::OwningAccountId.eq(owning_account_id.as_uuid()))
            .order_by_desc(entity::projects::Column::ActiveSelectedAt)
            .order_by_desc(entity::projects::Column::CreatedAt)
            .all(&self.conn)
            .await?;

        let mut out = Vec::with_capacity(projects.len());
        for row in projects {
            let project = ProjectRecord::try_from(row)?;
            let repo = entity::project_repositories::Entity::find_by_id(project.id.as_uuid())
                .one(&self.conn)
                .await?
                .ok_or_else(|| StoreError::DataInvariant {
                    column: "project_repositories.project_id",
                    cause: ValidationError::RepositoryRefInvalid,
                })?;
            out.push(ProjectSetupRecord {
                is_active: project.active_selected_at.is_some(),
                project,
                repository: ProjectRepositoryRecord::try_from(repo)?,
                spec_count: 0,
                milestone_count: 0,
                initiative_count: 0,
            });
        }
        Ok(out)
    }

    async fn set_active_project(
        &self,
        owning_account_id: AccountId,
        project_id: ProjectId,
        selected_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.conn
            .transaction::<_, (), StoreError>(|txn| {
                Box::pin(async move {
                    entity::projects::Entity::update_many()
                        .col_expr(
                            entity::projects::Column::ActiveSelectedAt,
                            sea_orm::sea_query::Expr::value(None::<DateTime<Utc>>),
                        )
                        .filter(
                            entity::projects::Column::OwningAccountId
                                .eq(owning_account_id.as_uuid()),
                        )
                        .exec(txn)
                        .await?;
                    entity::projects::Entity::update_many()
                        .col_expr(
                            entity::projects::Column::ActiveSelectedAt,
                            sea_orm::sea_query::Expr::value(Some(selected_at)),
                        )
                        .filter(
                            entity::projects::Column::OwningAccountId
                                .eq(owning_account_id.as_uuid()),
                        )
                        .filter(entity::projects::Column::Id.eq(project_id.as_uuid()))
                        .exec(txn)
                        .await?;
                    Ok(())
                })
            })
            .await
            .map_err(map_transaction_error)
    }
}

fn map_transaction_error(err: sea_orm::TransactionError<StoreError>) -> StoreError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => StoreError::from(db_err),
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

fn map_project_transaction_error(
    err: sea_orm::TransactionError<ProjectStoreError>,
) -> ProjectStoreError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            ProjectStoreError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

fn map_project_repository_insert_error(err: sea_orm::DbErr) -> ProjectStoreError {
    let lower = err.to_string().to_lowercase();
    if lower.contains("unique") || lower.contains("duplicate") {
        return ProjectStoreError::DuplicateRepository;
    }
    ProjectStoreError::Store(StoreError::from(err))
}
