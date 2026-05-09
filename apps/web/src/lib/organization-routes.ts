import {
  ORGANIZATION_API_ROUTES,
  type OrganizationAdminPermission,
} from "@/lib/organization-api";

export const ORGANIZATION_WEB_ROUTE = "/organizations" as const;

export { ORGANIZATION_API_ROUTES };
export type { OrganizationAdminPermission };

export const ORGANIZATION_WIRE_TEST_IDS = {
  page: "org-wire-page",
  createNameInput: "org-wire-create-name",
  createSubmit: "org-wire-create-submit",
  listSubmit: "org-wire-list-submit",
  permissionOrgIdInput: "org-wire-permission-org-id",
  permissionSelect: "org-wire-permission-select",
  permissionSubmit: "org-wire-permission-submit",
  operationStatus: "org-wire-operation-status",
  operationSequence: "org-wire-operation-sequence",
  failureCode: "org-wire-failure-code",
  failureDetail: "org-wire-failure-detail",
  organizationsList: "org-wire-organizations-list",
} as const;

export function normalizeOrganizationName(raw: string): string {
  return raw.trim().split(/\s+/).join(" ").toLowerCase();
}

export function organizationNameToWireKey(name: string): string {
  const normalized = normalizeOrganizationName(name)
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .replace(/-+$/, "");
  return normalized === "" ? "organization" : normalized;
}

export function organizationRowTestId(name: string): string {
  return `org-wire-org-${organizationNameToWireKey(name)}`;
}

export function organizationIdTestId(name: string): string {
  return `org-wire-org-id-${organizationNameToWireKey(name)}`;
}

export function organizationPermissionsTestId(name: string): string {
  return `org-wire-org-permissions-${organizationNameToWireKey(name)}`;
}

export function organizationInitialProjectsTestId(name: string): string {
  return `org-wire-org-initial-projects-${organizationNameToWireKey(name)}`;
}
