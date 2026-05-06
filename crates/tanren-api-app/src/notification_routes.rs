use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_contract::{
    EvaluateNotificationRouteRequest, ListNotificationPreferencesResponse,
    ReadPendingRoutingSnapshotResponse, SetNotificationPreferencesRequest,
    SetNotificationPreferencesResponse, SetOrganizationNotificationOverridesRequest,
    SetOrganizationNotificationOverridesResponse,
};
use tower_sessions::Session;

use crate::AppState;
use crate::cookies::require_authenticated;
use crate::errors::{ValidatedJson, map_app_error};

#[utoipa::path(
    get,
    path = "/me/notifications",
    responses(
        (status = 200, body = ListNotificationPreferencesResponse, description = "Notification preferences"),
        (status = 401, body = crate::errors::AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "notifications",
)]
pub(crate) async fn list_notification_preferences_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .list_notification_preferences(state.store.as_ref(), &actor)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/me/notifications",
    request_body = SetNotificationPreferencesRequest,
    responses(
        (status = 200, body = SetNotificationPreferencesResponse, description = "Preferences upserted"),
        (status = 400, body = crate::errors::AccountFailureBody, description = "validation_failed"),
        (status = 401, body = crate::errors::AccountFailureBody, description = "unauthenticated"),
        (status = 422, body = crate::errors::AccountFailureBody, description = "unsupported_notification_channel"),
    ),
    tag = "notifications",
)]
pub(crate) async fn set_notification_preferences_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SetNotificationPreferencesRequest>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .set_notification_preferences(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/me/notifications/org-overrides",
    request_body = SetOrganizationNotificationOverridesRequest,
    responses(
        (status = 200, body = SetOrganizationNotificationOverridesResponse, description = "Org overrides upserted"),
        (status = 400, body = crate::errors::AccountFailureBody, description = "validation_failed"),
        (status = 401, body = crate::errors::AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = crate::errors::AccountFailureBody, description = "unauthorized_organization_override"),
        (status = 422, body = crate::errors::AccountFailureBody, description = "unsupported_notification_channel"),
    ),
    tag = "notifications",
)]
pub(crate) async fn set_organization_notification_overrides_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SetOrganizationNotificationOverridesRequest>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .set_organization_notification_overrides(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/me/notifications/evaluate",
    request_body = EvaluateNotificationRouteRequest,
    responses(
        (status = 200, body = tanren_contract::EvaluateNotificationRouteResponse, description = "Route evaluated"),
        (status = 400, body = crate::errors::AccountFailureBody, description = "validation_failed"),
        (status = 401, body = crate::errors::AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "notifications",
)]
pub(crate) async fn evaluate_notification_route_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<EvaluateNotificationRouteRequest>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .evaluate_notification_route(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/me/notifications/pending-routes",
    responses(
        (status = 200, body = ReadPendingRoutingSnapshotResponse, description = "Pending routing snapshot"),
        (status = 401, body = crate::errors::AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "notifications",
)]
pub(crate) async fn read_pending_routing_snapshot_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .read_pending_routing_snapshot(state.store.as_ref(), &actor)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}
