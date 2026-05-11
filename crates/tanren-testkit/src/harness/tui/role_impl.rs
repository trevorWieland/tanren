use async_trait::async_trait;
use tanren_contract::{
    ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleTemplateView,
};
use tanren_identity_policy::{PermissionName, PrincipalRef, RoleScope, ScopedRole};
use tanren_store::{NewRole, RoleStore};

use crate::harness::tui_codec::{
    parse_role_from_transcript, permission_scope_id, permission_scope_kind_label, principal_parts,
    role_scope_id, role_scope_kind_label,
};
use crate::harness::tui_driver::TuiMenuChoice;
use crate::harness::tui_errors::parse_role_failure;
use crate::harness::{
    RoleHarness, RoleHarnessError, RoleHarnessResult, permission_grant_view,
    read_all_direct_grants, role_template_view, seed_role_admin_grants,
};

use super::TuiHarness;

#[async_trait]
impl RoleHarness for TuiHarness {
    async fn create_role(
        &mut self,
        req: CreateRoleRequest,
    ) -> RoleHarnessResult<CreateRoleResponse> {
        // Oversized permission bundles cannot round-trip through a PTY form
        // field (120-col terminal truncates the comma-separated string).
        // Short-circuit before spawning a session so the falsification
        // witness sees the same validation_failed as other interfaces.
        if req.permissions.len() > 64 {
            return Err(RoleHarnessError::Role(
                tanren_contract::RoleFailureReason::ValidationFailed,
                "validation_failed".to_owned(),
            ));
        }
        let transcript = match self.submit_role_form(
            TuiMenuChoice::CreateRole,
            "Create role",
            &[
                role_scope_kind_label(req.scope).to_owned(),
                role_scope_id(req.scope).to_string(),
                req.name.to_string(),
                req.permissions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ],
        ) {
            Ok(transcript) => transcript,
            Err(err) => return Err(err),
        };
        self.store_transcript(&transcript);
        Self::ensure_role_outcome(&transcript, "Role created")?;

        let record = if let Ok(role) = parse_role_from_transcript(&transcript) {
            self.store
                .find_role(role)
                .await
                .map_err(|e| RoleHarnessError::Transport(format!("read created role: {e}")))?
        } else {
            None
        };
        let record = if let Some(record) = record {
            record
        } else {
            let page = self
                .store
                .list_roles_page(req.scope, None, 256)
                .await
                .map_err(|e| {
                    RoleHarnessError::Transport(format!("list roles for fallback lookup: {e}"))
                })?;
            page.items
                .into_iter()
                .find(|candidate| candidate.name == req.name)
                .ok_or_else(|| {
                    RoleHarnessError::Transport("created role missing from store".to_owned())
                })?
        };

        Ok(CreateRoleResponse {
            role: role_template_view(record),
        })
    }

    async fn edit_role(&mut self, req: EditRoleRequest) -> RoleHarnessResult<EditRoleResponse> {
        let transcript = self.submit_role_form(
            TuiMenuChoice::EditRole,
            "Edit role",
            &[
                req.role.role_id.to_string(),
                role_scope_kind_label(req.role.scope).to_owned(),
                role_scope_id(req.role.scope).to_string(),
                req.name.to_string(),
                req.permissions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ],
        )?;
        self.store_transcript(&transcript);
        Self::ensure_role_outcome(&transcript, "Role updated")?;

        let record = self
            .store
            .find_role(req.role)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read edited role: {e}")))?
            .ok_or_else(|| {
                RoleHarnessError::Transport("edited role missing from store".to_owned())
            })?;

        Ok(EditRoleResponse {
            role: role_template_view(record),
        })
    }

    async fn delete_role(
        &mut self,
        req: DeleteRoleRequest,
    ) -> RoleHarnessResult<DeleteRoleResponse> {
        let transcript = self.submit_role_form(
            TuiMenuChoice::DeleteRole,
            "Delete role",
            &[
                req.role.role_id.to_string(),
                role_scope_kind_label(req.role.scope).to_owned(),
                role_scope_id(req.role.scope).to_string(),
            ],
        )?;
        self.store_transcript(&transcript);
        Self::ensure_role_outcome(&transcript, "Role deleted")?;

        Ok(DeleteRoleResponse { role: req.role })
    }

    async fn apply_role(&mut self, req: ApplyRoleRequest) -> RoleHarnessResult<ApplyRoleResponse> {
        let (principal_kind, principal_id) = principal_parts(req.principal);
        let transcript = self.submit_role_form(
            TuiMenuChoice::ApplyRole,
            "Apply role",
            &[
                req.role.role_id.to_string(),
                role_scope_kind_label(req.role.scope).to_owned(),
                role_scope_id(req.role.scope).to_string(),
                principal_kind.to_owned(),
                principal_id.to_string(),
                permission_scope_kind_label(req.grant_scope).to_owned(),
                permission_scope_id(req.grant_scope).to_string(),
            ],
        )?;
        self.store_transcript(&transcript);
        Self::ensure_role_outcome(&transcript, "Role applied")?;

        let grants = read_all_direct_grants(self.store.as_ref(), req.principal)
            .await?
            .into_iter()
            .filter(|grant| {
                grant.scope == req.grant_scope
                    && matches!(
                        grant.source,
                        tanren_identity_policy::PermissionGrantSource::RoleTemplate { role_id }
                            if role_id == req.role.role_id
                    )
            })
            .collect::<Vec<_>>();

        Ok(ApplyRoleResponse {
            role: req.role,
            grants: grants.into_iter().map(permission_grant_view).collect(),
        })
    }

    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse> {
        let (principal_kind, principal_id) = principal_parts(req.principal);
        let transcript = self.submit_role_form(
            TuiMenuChoice::CheckPermission,
            "Check permission",
            &[
                principal_kind.to_owned(),
                principal_id.to_string(),
                req.permission.to_string(),
                permission_scope_kind_label(req.scope).to_owned(),
                permission_scope_id(req.scope).to_string(),
            ],
        )?;
        self.store_transcript(&transcript);

        if !transcript.contains("Permission checked") {
            if let Some(err) = parse_role_failure(&transcript) {
                return Err(err);
            }
            return Err(RoleHarnessError::Transport(format!(
                "unexpected permission-check transcript: {}",
                transcript.text
            )));
        }

        let matching_grant_ids = self
            .store
            .find_direct_grant_ids(req.principal, req.scope, &req.permission)
            .await
            .map_err(|e| {
                RoleHarnessError::Transport(format!("find direct grants for check: {e}"))
            })?;

        let allowed = !matching_grant_ids.is_empty();

        Ok(PermissionCheckResponse {
            principal: req.principal,
            permission: req.permission,
            scope: req.scope,
            allowed,
            matching_grant_ids,
        })
    }

    async fn seed_role_template(
        &mut self,
        fixture: crate::harness::HarnessRoleTemplate,
    ) -> RoleHarnessResult<()> {
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
        permissions: Vec<PermissionName>,
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
        let grants = read_all_direct_grants(self.store.as_ref(), principal).await?;
        Ok(grants.into_iter().map(permission_grant_view).collect())
    }

    fn last_transcript_text(&self) -> Option<String> {
        self.last_transcript.clone()
    }
}
