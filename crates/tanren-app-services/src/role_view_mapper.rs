//! Shared record-to-contract view mappers for role-template surfaces.

use tanren_contract::{PermissionGrantView, RoleTemplateView};
use tanren_store::{PermissionGrantRecord, RoleRecord};

/// Build the external role-template view from a store role record.
#[must_use]
pub fn role_template_view(record: RoleRecord) -> RoleTemplateView {
    RoleTemplateView {
        id: record.id,
        scope: record.scope,
        name: record.name,
        permissions: record.permissions,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

/// Build the external direct-grant view from a store grant record.
#[must_use]
pub fn permission_grant_view(record: PermissionGrantRecord) -> PermissionGrantView {
    PermissionGrantView {
        id: record.id,
        principal: record.principal,
        scope: record.scope,
        permission: record.permission,
        source: record.source,
        revocation: record.revocation,
        granted_at: record.granted_at,
    }
}
