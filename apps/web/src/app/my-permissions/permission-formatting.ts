import type {
  MyPermissionEntry,
  PermissionConstraintView,
  PermissionGrantSource,
  PermissionScopeView,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

export function formatGrantSource(source: PermissionGrantSource): string {
  switch (source.kind) {
    case "direct":
      return m.myPermissions_sourceDirect();
    case "role_template":
      return `${m.myPermissions_sourceRoleTemplate()}: ${source.role_template}`;
  }
}

export function formatConstraintSource(
  source: PermissionConstraintView["source"],
): string {
  switch (source) {
    case "organization_policy":
      return m.myPermissions_constraintSourceOrganizationPolicy();
    case "project_policy":
      return m.myPermissions_constraintSourceProjectPolicy();
  }
}

export function stateBadgeClass(
  state: MyPermissionEntry["effective_state"],
): string {
  if (state === "constrained") {
    return "border-[--color-error] text-[--color-error]";
  }
  return "border-[--color-success] text-[--color-success]";
}

export function scopeDisplayLabel(scope: PermissionScopeView): string {
  switch (scope.kind) {
    case "organization":
      return m.myPermissions_organizationScopeLabel();
    case "project":
      return m.myPermissions_projectScopeLabel();
  }
}

export function cursorOrNone(cursor: null | string | undefined): string {
  if (!cursor || cursor.trim() === "") {
    return m.myPermissions_metadataNone();
  }
  return cursor;
}
