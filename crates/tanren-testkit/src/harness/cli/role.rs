use std::process::Stdio;

use async_trait::async_trait;
use regex::Regex;
use serde::de::DeserializeOwned;
use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleTemplateView,
};
use tanren_identity_policy::{PermissionScope, PrincipalRef, RoleScope, ScopedRole};
use tanren_store::{NewRole, RoleStore};
use tokio::process::Command;
use uuid::Uuid;

use super::super::api::role_code_to_reason;
use super::super::{
    HarnessRoleTemplate, RoleHarness, RoleHarnessError, RoleHarnessResult, seed_role_admin_grants,
};
use super::CliHarness;

#[async_trait]
impl RoleHarness for CliHarness {
    async fn create_role(
        &mut self,
        req: CreateRoleRequest,
    ) -> RoleHarnessResult<CreateRoleResponse> {
        let mut args = vec![
            "role".to_owned(),
            "create".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
            "--scope-kind".to_owned(),
            role_scope_kind_label(req.scope).to_owned(),
            "--scope-id".to_owned(),
            role_scope_id(req.scope).to_string(),
            "--name".to_owned(),
            req.name.to_string(),
        ];
        for permission in req.permissions {
            args.push("--permission".to_owned());
            args.push(permission.to_string());
        }
        let output = run_cli(self, &args).await?;
        if !output.status.success() {
            return Err(translate_cli_role_error(&output.stderr));
        }
        parse_json_stdout(&output.stdout, "role create")
    }

    async fn edit_role(&mut self, req: EditRoleRequest) -> RoleHarnessResult<EditRoleResponse> {
        let mut args = vec![
            "role".to_owned(),
            "edit".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
            "--role-id".to_owned(),
            req.role.role_id.to_string(),
            "--scope-kind".to_owned(),
            role_scope_kind_label(req.role.scope).to_owned(),
            "--scope-id".to_owned(),
            role_scope_id(req.role.scope).to_string(),
            "--name".to_owned(),
            req.name.to_string(),
        ];
        for permission in req.permissions {
            args.push("--permission".to_owned());
            args.push(permission.to_string());
        }
        let output = run_cli(self, &args).await?;
        if !output.status.success() {
            return Err(translate_cli_role_error(&output.stderr));
        }
        parse_json_stdout(&output.stdout, "role edit")
    }

    async fn delete_role(
        &mut self,
        req: DeleteRoleRequest,
    ) -> RoleHarnessResult<DeleteRoleResponse> {
        let args = vec![
            "role".to_owned(),
            "delete".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
            "--role-id".to_owned(),
            req.role.role_id.to_string(),
            "--scope-kind".to_owned(),
            role_scope_kind_label(req.role.scope).to_owned(),
            "--scope-id".to_owned(),
            role_scope_id(req.role.scope).to_string(),
        ];
        let output = run_cli(self, &args).await?;
        if !output.status.success() {
            return Err(translate_cli_role_error(&output.stderr));
        }
        parse_json_stdout(&output.stdout, "role delete")
    }

    async fn apply_role(&mut self, req: ApplyRoleRequest) -> RoleHarnessResult<ApplyRoleResponse> {
        let (principal_kind, principal_id) = principal_parts(req.principal);
        let args = vec![
            "role".to_owned(),
            "apply".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
            "--role-id".to_owned(),
            req.role.role_id.to_string(),
            "--role-scope-kind".to_owned(),
            role_scope_kind_label(req.role.scope).to_owned(),
            "--role-scope-id".to_owned(),
            role_scope_id(req.role.scope).to_string(),
            "--principal-kind".to_owned(),
            principal_kind.to_owned(),
            "--principal-id".to_owned(),
            principal_id.to_string(),
            "--grant-scope-kind".to_owned(),
            permission_scope_kind_label(req.grant_scope).to_owned(),
            "--grant-scope-id".to_owned(),
            permission_scope_id(req.grant_scope).to_string(),
        ];
        let output = run_cli(self, &args).await?;
        if !output.status.success() {
            return Err(translate_cli_role_error(&output.stderr));
        }
        parse_json_stdout(&output.stdout, "role apply")
    }

    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse> {
        let (principal_kind, principal_id) = principal_parts(req.principal);
        let args = vec![
            "role".to_owned(),
            "check".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
            "--principal-kind".to_owned(),
            principal_kind.to_owned(),
            "--principal-id".to_owned(),
            principal_id.to_string(),
            "--permission".to_owned(),
            req.permission.to_string(),
            "--scope-kind".to_owned(),
            permission_scope_kind_label(req.scope).to_owned(),
            "--scope-id".to_owned(),
            permission_scope_id(req.scope).to_string(),
        ];
        let output = run_cli(self, &args).await?;
        if !output.status.success() {
            return Err(translate_cli_role_error(&output.stderr));
        }
        parse_json_stdout(&output.stdout, "role check")
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
        scope: RoleScope,
        permissions: Vec<tanren_identity_policy::PermissionName>,
    ) -> RoleHarnessResult<()> {
        let actor = self
            .role_actor
            .ok_or_else(|| RoleHarnessError::Transport("missing role actor".to_owned()))?;
        seed_role_admin_grants(self.store.as_ref(), actor, scope, permissions).await
    }

    async fn read_role_template(
        &self,
        role: ScopedRole,
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
        principal: PrincipalRef,
    ) -> RoleHarnessResult<Vec<PermissionGrantView>> {
        let grants = self
            .store
            .list_all_direct_grants(principal)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_direct_grants: {e}")))?;
        Ok(grants.into_iter().map(permission_grant_view).collect())
    }
}

fn translate_cli_role_error(stderr: &[u8]) -> RoleHarnessError {
    let text = String::from_utf8_lossy(stderr);
    let re = Regex::new(r"error:\s*([a-z_]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = role_code_to_reason(code) {
            return RoleHarnessError::Role(reason, summary);
        }
    }
    RoleHarnessError::Transport(text.into_owned())
}

async fn run_cli(harness: &CliHarness, args: &[String]) -> RoleHarnessResult<std::process::Output> {
    Command::new(&harness.binary)
        .args(args)
        .env("TANREN_SESSION_FILE", &harness.session_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| RoleHarnessError::Transport(format!("spawn tanren-cli: {e}")))
}

fn parse_json_stdout<T: DeserializeOwned>(stdout: &[u8], context: &str) -> RoleHarnessResult<T> {
    let text = String::from_utf8_lossy(stdout);
    let payload = find_json_payload(&text).ok_or_else(|| {
        RoleHarnessError::Transport(format!(
            "decode {context} response from cli stdout: no JSON object found in `{text}`"
        ))
    })?;
    serde_json::from_value(payload).map_err(|e| {
        RoleHarnessError::Transport(format!("decode {context} response from cli stdout: {e}"))
    })
}

fn find_json_payload(stdout: &str) -> Option<serde_json::Value> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return Some(value);
        }
    }
    None
}

fn role_scope_kind_label(scope: RoleScope) -> &'static str {
    match scope {
        RoleScope::Account { .. } => "account",
        RoleScope::Organization { .. } => "organization",
        RoleScope::Project { .. } => "project",
    }
}

fn role_scope_id(scope: RoleScope) -> Uuid {
    match scope {
        RoleScope::Account { account_id } => account_id.as_uuid(),
        RoleScope::Organization { org_id } => org_id.as_uuid(),
        RoleScope::Project { project_id } => project_id.as_uuid(),
    }
}

fn permission_scope_kind_label(scope: PermissionScope) -> &'static str {
    match scope {
        PermissionScope::Account { .. } => "account",
        PermissionScope::Organization { .. } => "organization",
        PermissionScope::Project { .. } => "project",
    }
}

fn permission_scope_id(scope: PermissionScope) -> Uuid {
    match scope {
        PermissionScope::Account { account_id } => account_id.as_uuid(),
        PermissionScope::Organization { org_id } => org_id.as_uuid(),
        PermissionScope::Project { project_id } => project_id.as_uuid(),
    }
}

fn principal_parts(principal: PrincipalRef) -> (&'static str, Uuid) {
    match principal {
        PrincipalRef::Account { account_id } => ("account", account_id.as_uuid()),
        PrincipalRef::Role { role_id } => ("role", role_id.as_uuid()),
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
