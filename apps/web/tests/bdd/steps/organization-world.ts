import { type OrganizationAdminPermission } from "@/lib/organization-routes";

export interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

export interface OrganizationSnapshot {
  id: string;
  name: string;
  grantedPermissions: OrganizationAdminPermission[];
  initialProjectCount: number | null;
}

export interface OrganizationViewResponse {
  id: string;
  name: string;
}

export interface CreateOrganizationResponse {
  organization: OrganizationViewResponse;
  granted_permissions: OrganizationAdminPermission[];
  initial_project_count: number;
  proof_link: { behavior_id: string };
  source_link: { event_family: string; event_kind: string };
}

export interface ListOrganizationsResponse {
  organizations: OrganizationViewResponse[];
}

export interface CheckOrganizationPermissionResponse {
  account_id: string;
  org_id: string;
  permission: OrganizationAdminPermission;
  allowed: boolean;
}

export interface OrganizationErrorResponse {
  code: string;
  summary: string;
}

export interface OrganizationWorldState {
  organizationsByName: Map<string, OrganizationSnapshot>;
  lastOperationSucceeded: boolean;
  lastCreateResponse: CreateOrganizationResponse | null;
  lastListResponse: ListOrganizationsResponse | null;
  lastCheckResponse: CheckOrganizationPermissionResponse | null;
}

export interface OrganizationWorld {
  actors: Map<string, ActorState>;
  __orgState?: OrganizationWorldState;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasOnlyStringKeys(value: unknown, keys: readonly string[]): boolean {
  if (!isObjectRecord(value)) {
    return false;
  }
  return keys.every((key) => typeof value[key] === "string");
}

export function isOrganizationPermission(
  value: unknown,
): value is OrganizationAdminPermission {
  return typeof value === "string" && value.trim() !== "";
}

export function assertOrganizationPermission(
  value: string,
): OrganizationAdminPermission {
  if (isOrganizationPermission(value)) {
    return value;
  }
  throw new Error(`unsupported organization permission '${value}' in scenario`);
}

export function isOrganizationErrorResponse(
  value: unknown,
): value is OrganizationErrorResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (typeof value["code"] !== "string") {
    return false;
  }

  if (value["summary"] !== undefined && typeof value["summary"] !== "string") {
    return false;
  }

  return true;
}

export function isOrganizationViewResponse(
  value: unknown,
): value is OrganizationViewResponse {
  return hasOnlyStringKeys(value, ["id", "name"]);
}

export function isCreateOrganizationResponse(
  value: unknown,
): value is CreateOrganizationResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (!isOrganizationViewResponse(value["organization"])) {
    return false;
  }

  const permissions = value["granted_permissions"];
  if (!Array.isArray(permissions)) {
    return false;
  }

  if (
    !permissions.every((permission) => isOrganizationPermission(permission))
  ) {
    return false;
  }

  if (typeof value["initial_project_count"] !== "number") {
    return false;
  }

  const proofLink = value["proof_link"];
  if (!hasOnlyStringKeys(proofLink, ["behavior_id"])) {
    return false;
  }

  const sourceLink = value["source_link"];
  return hasOnlyStringKeys(sourceLink, ["event_family", "event_kind"]);
}

export function isListOrganizationsResponse(
  value: unknown,
): value is ListOrganizationsResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  const organizations = value["organizations"];
  if (!Array.isArray(organizations)) {
    return false;
  }

  return organizations.every((organization) =>
    isOrganizationViewResponse(organization),
  );
}

export function isCheckOrganizationPermissionResponse(
  value: unknown,
): value is CheckOrganizationPermissionResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (!hasOnlyStringKeys(value, ["account_id", "org_id", "permission"])) {
    return false;
  }

  if (!isOrganizationPermission(value["permission"])) {
    return false;
  }

  return typeof value["allowed"] === "boolean";
}

export function requireOrganizationWorld(world: unknown): OrganizationWorld {
  if (!isObjectRecord(world)) {
    throw new Error("scenario world must be an object");
  }

  const actors = world["actors"];
  if (!(actors instanceof Map)) {
    throw new Error("scenario world is missing actors map");
  }

  return world as unknown as OrganizationWorld;
}

export function actor(world: OrganizationWorld, name: string): ActorState {
  let state = world.actors.get(name);
  if (!state) {
    state = {};
    world.actors.set(name, state);
  }
  return state;
}

export function orgState(world: OrganizationWorld): OrganizationWorldState {
  if (!world.__orgState) {
    world.__orgState = {
      organizationsByName: new Map<string, OrganizationSnapshot>(),
      lastOperationSucceeded: false,
      lastCreateResponse: null,
      lastListResponse: null,
      lastCheckResponse: null,
    };
  }
  return world.__orgState;
}
