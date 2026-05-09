use tanren_identity_policy::PrincipalRef;
use tanren_store::{
    PermissionGrantListCursor, PermissionGrantRecord, ROLE_GRANT_LIST_PAGE_MAX, RoleStore,
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
