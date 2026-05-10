import type {
  components,
  operations,
  paths,
} from "@/lib/generated/api-contract";
import { BEHAVIOR_HARNESS_ROUTES } from "@/lib/generated/behavior-harness-routes";

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
export type OrganizationCreateBehaviorId =
  components["schemas"]["OrganizationBehaviorId"];
export type OrganizationEventFamily = "organization";
export type OrganizationCreatedEventKind = "organization_created";
const ORGANIZATION_HARNESS_ROUTE_OWNER = BEHAVIOR_HARNESS_ROUTES.organizations;
export const ORGANIZATION_BEHAVIOR_FEATURE_PATH =
  ORGANIZATION_HARNESS_ROUTE_OWNER.featurePath;
export const ORGANIZATION_CREATE_BEHAVIOR_ID =
  ORGANIZATION_HARNESS_ROUTE_OWNER.behaviorId as OrganizationCreateBehaviorId;
export const ORGANIZATION_EVENT_FAMILY: OrganizationEventFamily =
  "organization";
export const ORGANIZATION_CREATED_EVENT_KIND: OrganizationCreatedEventKind =
  "organization_created";
export const ORGANIZATION_WEB_HARNESS_ROUTE =
  ORGANIZATION_HARNESS_ROUTE_OWNER.route;

export interface WebSurfaceContract<
  TRoute extends string,
  TBehaviorId extends string,
  TApiCreate extends string,
  TApiList extends string,
  TApiCheckPermission extends string,
> {
  readonly route: TRoute;
  readonly behaviorId: TBehaviorId;
  readonly apiPaths: {
    readonly create: TApiCreate;
    readonly list: TApiList;
    readonly checkPermission: TApiCheckPermission;
  };
  readonly harnessRoute: `/harness/${string}`;
  readonly featurePath: string;
  readonly eventFamily: "organization";
  readonly createdEventKind: "organization_created";
  readonly requiresAuth: true;
}

export const ORGANIZATION_WEB_SURFACE_CONTRACT = {
  route: ORGANIZATION_RUNTIME_ROUTE,
  behaviorId: ORGANIZATION_CREATE_BEHAVIOR_ID,
  apiPaths: ORGANIZATION_API_ROUTES,
  harnessRoute: ORGANIZATION_WEB_HARNESS_ROUTE,
  featurePath: ORGANIZATION_BEHAVIOR_FEATURE_PATH,
  eventFamily: ORGANIZATION_EVENT_FAMILY,
  createdEventKind: ORGANIZATION_CREATED_EVENT_KIND,
  requiresAuth: true as const,
} as const satisfies WebSurfaceContract<
  typeof ORGANIZATION_RUNTIME_ROUTE,
  OrganizationCreateBehaviorId,
  typeof ORGANIZATION_API_ROUTES.create,
  typeof ORGANIZATION_API_ROUTES.list,
  typeof ORGANIZATION_API_ROUTES.checkPermission
>;
export type OrganizationAdminPermission =
  components["schemas"]["OrganizationPermission"];
export type CreateOrganizationApiRequest =
  operations["create_organization_route"]["requestBody"]["content"]["application/json"];
export type CheckOrganizationPermissionApiRequest =
  operations["check_organization_permission_route"]["requestBody"]["content"]["application/json"];
export type ListOrganizationsApiRequest = NonNullable<
  operations["list_organizations_route"]["parameters"]["query"]
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

export const ORGANIZATION_PRODUCT_TEST_IDS = {
  authGate: "org-product-auth-gate",
  organizationsList: "org-product-organizations-list",
  emptyState: "org-product-empty-state",
  sourceLink: "org-product-source-link",
  freshness: "org-product-freshness",
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
