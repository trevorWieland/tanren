import type {
  components,
  operations,
  paths,
} from "@/lib/generated/api-contract";

type OrganizationCreatePath = "/organizations";
type OrganizationListPath = "/organizations";
type OrganizationPermissionPath = "/organizations/permissions/check";

export const ORGANIZATION_API_ROUTES = {
  create: "/organizations" as OrganizationCreatePath,
  list: "/organizations" as OrganizationListPath,
  checkPermission:
    "/organizations/permissions/check" as OrganizationPermissionPath,
} as const satisfies {
  create: keyof paths;
  list: keyof paths;
  checkPermission: keyof paths;
};

export const ORGANIZATION_RUNTIME_ROUTE = "/organizations" as const;
export const ORGANIZATION_WEB_HARNESS_ROUTE =
  "/harness/b-0066/organizations" as const;
export type OrganizationAdminPermission =
  components["schemas"]["OrganizationPermission"];
export type CreateOrganizationApiRequest =
  operations["create_organization_route"]["requestBody"]["content"]["application/json"];
export type CheckOrganizationPermissionApiRequest =
  operations["check_organization_permission_route"]["requestBody"]["content"]["application/json"];
export type ListOrganizationsApiRequest = NonNullable<
  paths["/organizations"]["parameters"]["query"]
>;

export const ORGANIZATION_WIRE_TEST_IDS = {
  page: "org-wire-page",
  createNameInput: "org-wire-create-name",
  createIdempotencyKeyInput: "org-wire-create-idempotency-key",
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

export const ORGANIZATION_ADMIN_PERMISSION_OPTIONS = [
  "invite",
  "manage_access",
  "configure",
  "set_policy",
  "delete",
] as const satisfies readonly OrganizationAdminPermission[];

export function isOrganizationAdminPermission(
  value: unknown,
): value is OrganizationAdminPermission {
  return (
    typeof value === "string" &&
    ORGANIZATION_ADMIN_PERMISSION_OPTIONS.includes(
      value as OrganizationAdminPermission,
    )
  );
}

export function buildCreateOrganizationApiRequest(
  name: string,
  idempotencyKey?: string,
): CreateOrganizationApiRequest {
  return {
    name,
    idempotency_key:
      idempotencyKey !== undefined && idempotencyKey.trim() !== ""
        ? idempotencyKey
        : null,
  };
}

export function buildCheckOrganizationPermissionApiRequest(
  orgId: string,
  permission: OrganizationAdminPermission,
): CheckOrganizationPermissionApiRequest {
  return {
    org_id: orgId,
    permission,
  };
}

export function buildConfigurePermissionApiRequest(
  orgId: string,
): CheckOrganizationPermissionApiRequest {
  return buildCheckOrganizationPermissionApiRequest(orgId, "configure");
}

export function buildListOrganizationsApiRequest(
  request: ListOrganizationsApiRequest = {},
): ListOrganizationsApiRequest {
  const output: ListOrganizationsApiRequest = {};
  if (request.limit !== undefined) {
    output.limit = request.limit;
  }
  if (request.cursor !== undefined) {
    output.cursor = request.cursor;
  }
  return output;
}

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
