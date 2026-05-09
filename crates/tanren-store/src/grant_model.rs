//! Permission-grant row codec helpers.

use chrono::{DateTime, Utc};
use tanren_identity_policy::{PermissionGrantRevocation, PermissionGrantSource};
use uuid::Uuid;

use crate::{StoreError, parse_db_principal_ref};

pub(crate) fn parse_db_permission_grant_source(
    kind: &str,
    source_ref: Option<Uuid>,
) -> Result<PermissionGrantSource, StoreError> {
    match kind {
        "role_template" => match source_ref {
            Some(role_id) => Ok(PermissionGrantSource::RoleTemplate {
                role_id: tanren_identity_policy::RoleId::new(role_id),
            }),
            None => Err(StoreError::EnumDataInvariant {
                column: "permission_grant_source_ref",
                value: "null".to_owned(),
            }),
        },
        "direct_assignment" => Ok(PermissionGrantSource::DirectAssignment),
        _ => Err(StoreError::EnumDataInvariant {
            column: "permission_grant_source_kind",
            value: kind.to_owned(),
        }),
    }
}

pub(crate) fn parse_db_permission_grant_revocation(
    revoked_by_kind: Option<&str>,
    revoked_by_ref: Option<Uuid>,
    revoked_at: Option<DateTime<Utc>>,
) -> Result<Option<PermissionGrantRevocation>, StoreError> {
    match (revoked_by_kind, revoked_by_ref, revoked_at) {
        (None, None, None) => Ok(None),
        (Some(kind), Some(principal_ref), Some(at)) => {
            let revoked_by = parse_db_principal_ref(kind, principal_ref)?;
            Ok(Some(PermissionGrantRevocation {
                revoked_by,
                revoked_at: at,
            }))
        }
        _ => Err(StoreError::PolicyDataInvariant {
            column: "permission_grant_revocation",
            cause: "revocation fields must be either all null or all populated".to_owned(),
        }),
    }
}

pub(crate) fn permission_grant_source_to_parts(
    source: PermissionGrantSource,
) -> (&'static str, Option<Uuid>) {
    match source {
        PermissionGrantSource::RoleTemplate { role_id } => {
            ("role_template", Some(role_id.as_uuid()))
        }
        PermissionGrantSource::DirectAssignment => ("direct_assignment", None),
    }
}
