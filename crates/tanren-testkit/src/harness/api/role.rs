use async_trait::async_trait;
use serde_json::Value;
use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleTemplateView,
};
use tanren_store::{NewRole, RoleStore};

use super::super::{
    HarnessRoleTemplate, RoleHarness, RoleHarnessError, RoleHarnessResult, permission_grant_view,
    role_template_view, seed_role_admin_grants,
};
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
            .header(
                "x-csrf-token",
                self.csrf_token.as_deref().unwrap_or("missing-csrf-token"),
            )
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
            .header(
                "x-csrf-token",
                self.csrf_token.as_deref().unwrap_or("missing-csrf-token"),
            )
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
            .header(
                "x-csrf-token",
                self.csrf_token.as_deref().unwrap_or("missing-csrf-token"),
            )
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
            .header(
                "x-csrf-token",
                self.csrf_token.as_deref().unwrap_or("missing-csrf-token"),
            )
            .json(&req)
            .send()
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("POST /roles/apply: {e}")))?;
        parse_role_response(response, "POST /roles/apply").await
    }
    async fn apply_role_concurrent(
        &mut self,
        req: ApplyRoleRequest,
        count: usize,
    ) -> Vec<RoleHarnessResult<ApplyRoleResponse>> {
        let base_url = self.base_url.clone();
        let csrf = self
            .csrf_token
            .clone()
            .unwrap_or_else(|| "missing-csrf-token".to_owned());
        // Clone the harness client (cheap Arc clone) so each concurrent
        // task shares the same cookie store — the authenticated session
        // carries through without manual cookie extraction.
        let client = self.client.clone();
        let mut handles = Vec::with_capacity(count);
        for _ in 0..count {
            let url = format!("{base_url}/roles/apply");
            let body = serde_json::to_value(&req).expect("ApplyRoleRequest serializes to JSON");
            let task_client = client.clone();
            let csrf_header = csrf.clone();
            handles.push(tokio::spawn(async move {
                let response = task_client
                    .post(&url)
                    .header("x-csrf-token", &csrf_header)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| RoleHarnessError::Transport(format!("POST /roles/apply: {e}")))?;
                parse_role_response(response, "POST /roles/apply").await
            }));
        }
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(join_err) => results.push(Err(RoleHarnessError::Transport(format!(
                    "concurrent apply task panicked: {join_err}"
                )))),
            }
        }
        results
    }

    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse> {
        let url = format!("{}/permissions/check", self.base_url);
        let response = self
            .client
            .post(&url)
            .header(
                "x-csrf-token",
                self.csrf_token.as_deref().unwrap_or("missing-csrf-token"),
            )
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

    async fn seed_role_admin_for_authenticated_actor(
        &mut self,
        scope: tanren_identity_policy::RoleScope,
        permissions: Vec<tanren_identity_policy::PermissionName>,
    ) -> RoleHarnessResult<()> {
        let actor = self.role_actor.ok_or_else(|| {
            RoleHarnessError::Transport("missing authenticated role actor".to_owned())
        })?;
        seed_role_admin_grants(self.store.as_ref(), actor, scope, permissions).await
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
        let grants = super::super::read_all_direct_grants(self.store.as_ref(), principal).await?;
        Ok(grants.into_iter().map(permission_grant_view).collect())
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
