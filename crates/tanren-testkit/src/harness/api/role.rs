use async_trait::async_trait;
use serde_json::Value;
use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleTemplateView,
};
use tanren_store::{NewRole, RoleStore};

use super::super::{HarnessRoleTemplate, RoleHarness, RoleHarnessError, RoleHarnessResult};
use super::{ApiHarness, role_code_to_reason};

#[async_trait]
impl RoleHarness for ApiHarness {
    async fn create_role(
        &mut self,
        req: CreateRoleRequest,
    ) -> RoleHarnessResult<CreateRoleResponse> {
        let url = format!("{}/roles", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /roles: {e}")))?;
        parse_role_response(response, "POST /roles").await
    }

    async fn edit_role(&mut self, req: EditRoleRequest) -> RoleHarnessResult<EditRoleResponse> {
        let url = format!("{}/roles/edit", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /roles/edit: {e}")))?;
        parse_role_response(response, "POST /roles/edit").await
    }

    async fn delete_role(
        &mut self,
        req: DeleteRoleRequest,
    ) -> RoleHarnessResult<DeleteRoleResponse> {
        let url = format!("{}/roles/delete", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /roles/delete: {e}")))?;
        parse_role_response(response, "POST /roles/delete").await
    }

    async fn apply_role(&mut self, req: ApplyRoleRequest) -> RoleHarnessResult<ApplyRoleResponse> {
        let url = format!("{}/roles/apply", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /roles/apply: {e}")))?;
        parse_role_response(response, "POST /roles/apply").await
    }

    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse> {
        let url = format!("{}/permissions/check", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /permissions/check: {e}")))?;
        parse_role_response(response, "POST /permissions/check").await
    }

    async fn seed_role_template(&mut self, fixture: HarnessRoleTemplate) -> RoleHarnessResult<()> {
        self.store
            .create_role(NewRole {
                id: fixture.id,
                scope: fixture.scope,
                name: fixture.name,
                permissions: fixture.permissions,
                created_at: fixture.created_at,
                updated_at: fixture.updated_at,
            })
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("seed_role_template: {e}")))?;
        Ok(())
    }

    async fn read_role_template(
        &self,
        role: tanren_identity_policy::ScopedRole,
    ) -> RoleHarnessResult<Option<RoleTemplateView>> {
        let maybe = self
            .store
            .find_role(role)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_role_template: {e}")))?;
        Ok(maybe.map(role_template_view))
    }

    async fn read_direct_grants(
        &self,
        principal: tanren_identity_policy::PrincipalRef,
    ) -> RoleHarnessResult<Vec<PermissionGrantView>> {
        let grants = self
            .store
            .list_all_direct_grants(principal)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_direct_grants: {e}")))?;
        Ok(grants.into_iter().map(permission_grant_view).collect())
    }
}

fn role_template_view(record: tanren_store::RoleRecord) -> RoleTemplateView {
    RoleTemplateView {
        id: record.id,
        scope: record.scope,
        name: record.name,
        permissions: record.permissions,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn permission_grant_view(record: tanren_store::PermissionGrantRecord) -> PermissionGrantView {
    PermissionGrantView {
        id: record.id,
        principal: record.principal,
        scope: record.scope,
        permission: record.permission,
        source_role_id: record.source_role_id,
        granted_at: record.granted_at,
    }
}

async fn parse_role_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    context: &str,
) -> RoleHarnessResult<T> {
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| RoleHarnessError::Transport(format!("decode body for {context}: {e}")))?;
    if !status.is_success() {
        return Err(role_failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| RoleHarnessError::Transport(format!("decode success body for {context}: {e}")))
}

fn role_failure_from_body(json: &Value) -> RoleHarnessError {
    let code = json
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error")
        .to_owned();
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = role_code_to_reason(&code) {
        RoleHarnessError::Role(reason, summary)
    } else {
        RoleHarnessError::Transport(format!("{code}: {summary}"))
    }
}
