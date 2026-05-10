use tanren_contract::{PermissionGrantView, RoleTemplateView};
use tanren_identity_policy::PrincipalRef;
use tanren_store::{
    PermissionGrantListCursor, PermissionGrantRecord, ROLE_GRANT_LIST_PAGE_MAX, RoleRecord,
    RoleStore,
};

use super::RoleHarnessError;

pub(crate) async fn read_all_direct_grants<S>(
    store: &S,
    principal: PrincipalRef,
) -> Result<Vec<PermissionGrantRecord>, RoleHarnessError>
where
    S: RoleStore + ?Sized,
{
    let mut cursor: Option<PermissionGrantListCursor> = None;
    let mut grants = Vec::new();
    loop {
        let page = store
            .list_direct_grants_page(principal, None, cursor.clone(), ROLE_GRANT_LIST_PAGE_MAX)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_direct_grants: {e}")))?;
        grants.extend(page.items);
        if let Some(next_cursor) = page.next_cursor {
            cursor = Some(next_cursor);
        } else {
            break;
        }
    }
    Ok(grants)
}

#[must_use]
pub(crate) fn role_template_view(record: RoleRecord) -> RoleTemplateView {
    tanren_app_services::role_view_mapper::role_template_view(record)
}

#[must_use]
pub(crate) fn permission_grant_view(record: PermissionGrantRecord) -> PermissionGrantView {
    tanren_app_services::role_view_mapper::permission_grant_view(record)
}
