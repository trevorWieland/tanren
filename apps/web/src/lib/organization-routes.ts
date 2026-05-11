import type {
  components,
  operations,
  paths,
} from "@/lib/generated/api-contract";
import { BEHAVIOR_HARNESS_ROUTES } from "@/lib/generated/behavior-harness-routes";

type OrganizationCreatePath = "/organizations";
type OrganizationListPath = "/organizations";
type OrganizationPermissionPath = "/organizations/permissions/check";
type OrganizationMembersPath = "/organizations/{org_id}/members";

export const ORGANIZATION_API_ROUTES = {
  create: "/organizations" as OrganizationCreatePath,
  list: "/organizations" as OrganizationListPath,
  checkPermission:
    "/organizations/permissions/check" as OrganizationPermissionPath,
  listMembers: "/organizations/{org_id}/members" as OrganizationMembersPath,
} as const satisfies {
  create: keyof paths;
  list: keyof paths;
  checkPermission: keyof paths;
  listMembers: keyof paths;
};

export const ORGANIZATION_RUNTIME_ROUTE = "/organizations" as const;
export type OrganizationCreateBehaviorId =
  components["schemas"]["OrganizationBehaviorId"];
export type OrganizationMembersBehaviorId =
  components["schemas"]["OrganizationBehaviorId"];
export type OrganizationEventFamily = "organization";
export type OrganizationCreatedEventKind = "organization_created";
const ORGANIZATION_HARNESS_ROUTE_OWNER = BEHAVIOR_HARNESS_ROUTES.organizations;
const ORGANIZATION_MEMBERS_HARNESS_ROUTE_OWNER =
  BEHAVIOR_HARNESS_ROUTES.organizationMembers;
export const ORGANIZATION_BEHAVIOR_FEATURE_PATH =
  ORGANIZATION_HARNESS_ROUTE_OWNER.featurePath;
export const ORGANIZATION_CREATE_BEHAVIOR_ID =
  ORGANIZATION_HARNESS_ROUTE_OWNER.behaviorId as OrganizationCreateBehaviorId;
export const ORGANIZATION_MEMBERS_BEHAVIOR_FEATURE_PATH =
  ORGANIZATION_MEMBERS_HARNESS_ROUTE_OWNER.featurePath;
export const ORGANIZATION_MEMBERS_BEHAVIOR_ID =
  ORGANIZATION_MEMBERS_HARNESS_ROUTE_OWNER.behaviorId as OrganizationMembersBehaviorId;
export const ORGANIZATION_EVENT_FAMILY: OrganizationEventFamily =
  "organization";
export const ORGANIZATION_CREATED_EVENT_KIND: OrganizationCreatedEventKind =
  "organization_created";
export const ORGANIZATION_WEB_HARNESS_ROUTE =
  ORGANIZATION_HARNESS_ROUTE_OWNER.route;
export const ORGANIZATION_MEMBERS_WEB_HARNESS_ROUTE =
  ORGANIZATION_MEMBERS_HARNESS_ROUTE_OWNER.route;

export interface WebSurfaceContract<
  TRoute extends string,
  TBehaviorId extends string,
  TApiCreate extends string,
  TApiList extends string,
  TApiCheckPermission extends string,
  TApiListMembers extends string,
> {
  readonly route: TRoute;
  readonly behaviorId: TBehaviorId;
  readonly apiPaths: {
    readonly create: TApiCreate;
    readonly list: TApiList;
    readonly checkPermission: TApiCheckPermission;
    readonly listMembers: TApiListMembers;
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
  typeof ORGANIZATION_API_ROUTES.checkPermission,
  typeof ORGANIZATION_API_ROUTES.listMembers
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
export type ListOrganizationMembersApiRequest = NonNullable<
  operations["list_organization_members_route"]["parameters"]["query"]
>;
export type ListOrganizationMembersApiPath =
  operations["list_organization_members_route"]["parameters"]["path"];

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
  listFreshness: "org-wire-list-freshness",
  listNextCursor: "org-wire-list-next-cursor",
  listNextPage: "org-wire-list-next-page",
} as const;

export const ORGANIZATION_PRODUCT_TEST_IDS = {
  authGate: "org-product-auth-gate",
  organizationsList: "org-product-organizations-list",
  emptyState: "org-product-empty-state",
  sourceLink: "org-product-source-link",
  freshness: "org-product-freshness",
  listNextCursor: "org-product-list-next-cursor",
} as const;

export const ORGANIZATION_MEMBERS_WIRE_TEST_IDS = {
  page: "org-members-wire-page",
  listSubmit: "org-members-wire-list-submit",
  operationStatus: "org-members-wire-operation-status",
  failureCode: "org-members-wire-failure-code",
  failureDetail: "org-members-wire-failure-detail",
  membersList: "org-members-wire-members-list",
  listFreshness: "org-members-wire-list-freshness",
  listNextCursor: "org-members-wire-list-next-cursor",
  listNextPage: "org-members-wire-list-next-page",
  listSourceLink: "org-members-wire-list-source-link",
} as const;

export const ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS = {
  authGate: "org-members-product-auth-gate",
  membersList: "org-members-product-members-list",
  emptyState: "org-members-product-empty-state",
  sourceLink: "org-members-product-source-link",
  freshness: "org-members-product-freshness",
  proofLink: "org-members-product-proof-link",
  listNextCursor: "org-members-product-list-next-cursor",
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

export type GrantSource = components["schemas"]["GrantSource"];

const GRANT_SOURCE_VALUES = [
  "direct",
  "role_template",
] as const satisfies readonly GrantSource[];

export function isGrantSource(value: unknown): value is GrantSource {
  return (
    typeof value === "string" &&
    (GRANT_SOURCE_VALUES as readonly string[]).includes(value)
  );
}

export function buildListOrganizationMembersApiRequest(
  request: ListOrganizationMembersApiRequest = {},
): ListOrganizationMembersApiRequest {
  const output: ListOrganizationMembersApiRequest = {};
  if (request.limit !== undefined) {
    output.limit = request.limit;
  }
  if (request.cursor !== undefined) {
    output.cursor = request.cursor;
  }
  return output;
}

export function buildListOrganizationMembersApiPath(
  orgId: string,
): ListOrganizationMembersApiPath {
  return { org_id: orgId };
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

export function memberRowTestId(accountId: string): string {
  return `org-members-wire-member-${accountId}`;
}

export function memberPermissionsTestId(accountId: string): string {
  return `org-members-wire-member-permissions-${accountId}`;
}

export function memberGrantSourceTestId(accountId: string): string {
  return `org-members-wire-member-grant-source-${accountId}`;
}

export function memberJoinedAtTestId(accountId: string): string {
  return `org-members-wire-member-joined-at-${accountId}`;
}
