use chrono::{DateTime, Utc};
use tanren_contract::ProjectFailureReason;
use tanren_identity_policy::{AccountId, ProviderFamily, RepositoryRef};
use tanren_store::{ProjectCommandReservation, ProjectCommandReservationResult, ProjectStore};

use crate::{AppServiceError, Clock};

pub(crate) async fn reserve_project_command<S>(
    store: &S,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository: &RepositoryRef,
    now: DateTime<Utc>,
) -> Result<ProjectCommandReservation, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let reservation = store
        .reserve_project_command(owning_account_id, provider_family, repository, now)
        .await?;
    match reservation {
        ProjectCommandReservationResult::Acquired(reservation) => Ok(reservation),
        ProjectCommandReservationResult::DuplicateRepository => Err(AppServiceError::Project(
            ProjectFailureReason::DuplicateRepository,
        )),
        ProjectCommandReservationResult::InFlight => {
            Err(AppServiceError::Project(ProjectFailureReason::InFlight))
        }
        ProjectCommandReservationResult::RateLimited => {
            Err(AppServiceError::Project(ProjectFailureReason::RateLimited))
        }
    }
}

pub(crate) async fn complete_or_fail_reservation<S, T>(
    store: &S,
    reservation: &ProjectCommandReservation,
    clock: &Clock,
    result: Result<T, AppServiceError>,
) -> Result<T, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    match result {
        Ok(value) => {
            store
                .finalize_project_command_reservation(reservation, clock.now())
                .await?;
            Ok(value)
        }
        Err(err @ AppServiceError::Project(ProjectFailureReason::DuplicateRepository)) => {
            store
                .finalize_project_command_reservation(reservation, clock.now())
                .await?;
            Err(err)
        }
        Err(err) => {
            if let Err(mark_err) = store
                .fail_project_command_reservation(reservation, clock.now())
                .await
            {
                let _ = mark_err;
            }
            Err(err)
        }
    }
}
