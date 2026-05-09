use std::io::Write;

use anyhow::{Context, Result};
use tanren_contract::{MyPermissionEntry, MyPermissionsResponse};

pub(crate) fn print_permissions(response: &MyPermissionsResponse) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let request_cursor = response.page.request_cursor.as_deref().unwrap_or("none");
    let next_cursor = response.page.next_cursor.as_deref().unwrap_or("none");
    writeln!(
        handle,
        "page limit={} returned={} request_cursor={} next_cursor={}",
        response.page.limit, response.page.returned, request_cursor, next_cursor,
    )
    .context("write page metadata")?;
    writeln!(
        handle,
        "read_metadata source={} generated_at={}",
        response.read_metadata.source,
        response.read_metadata.generated_at.to_rfc3339(),
    )
    .context("write read metadata")?;

    if response.organizations.is_empty() && response.projects.is_empty() {
        writeln!(handle, "permissions=none").context("write permissions result")?;
        return Ok(());
    }

    for organization in &response.organizations {
        for permission in &organization.permissions {
            write_permission_row(
                &mut handle,
                "organization",
                &organization.org_id.to_string(),
                permission,
            )
            .context("write organization permission row")?;
        }
    }
    for project in &response.projects {
        for permission in &project.permissions {
            write_permission_row(
                &mut handle,
                "project",
                &project.project_id.to_string(),
                permission,
            )
            .context("write project permission row")?;
        }
    }
    Ok(())
}

fn write_permission_row(
    handle: &mut impl Write,
    scope: &str,
    scope_id: &str,
    permission: &MyPermissionEntry,
) -> Result<()> {
    let (constraint_reason, constraint_source, constraint_source_reference) =
        permission.policy_constraint.as_ref().map_or_else(
            || ("none".to_owned(), "none".to_owned(), "none".to_owned()),
            |constraint| {
                (
                    format!("{:?}", constraint.reason),
                    format!("{:?}", constraint.source),
                    constraint.source_reference.clone(),
                )
            },
        );

    writeln!(
        handle,
        "scope={scope} scope_id={scope_id} permission={permission_name:?} effective_state={effective_state:?} source={grant_source:?} source_reference={source_reference} constraint_reason={constraint_reason} constraint_source={constraint_source} constraint_source_reference={constraint_source_reference}",
        permission_name = permission.permission,
        effective_state = permission.effective_state,
        grant_source = permission.grant_source,
        source_reference = permission.grant_source_reference,
    )
    .context("write permission row")
}
