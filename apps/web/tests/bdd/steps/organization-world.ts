import {
  type CheckOrganizationPermissionResponse,
  type CreateOrganizationResponse,
  type ListOrganizationMembersResponse,
  type ListOrganizationsResponse,
  type OrganizationAdminPermission,
  type OrganizationProofLink,
  type OrganizationSourceLink,
} from "@/lib/organization-api";

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
  proofLink: OrganizationProofLink | null;
  sourceLink: OrganizationSourceLink | null;
}

export interface OrganizationWorldState {
  organizationsByName: Map<string, OrganizationSnapshot>;
  createdOrganizationByIdempotencyKey: Map<string, string>;
  lastOperationSucceeded: boolean;
  lastCreateResponse: CreateOrganizationResponse | null;
  lastListResponse: ListOrganizationsResponse | null;
  lastCheckResponse: CheckOrganizationPermissionResponse | null;
  lastListedMembersResponse: ListOrganizationMembersResponse | null;
  lastReplayExpectedOrganizationId: string | null;
  lastReplayObservedOrganizationId: string | null;
}

export interface OrganizationWorld {
  actors: Map<string, ActorState>;
  __orgState?: OrganizationWorldState;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
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
      createdOrganizationByIdempotencyKey: new Map<string, string>(),
      lastOperationSucceeded: false,
      lastCreateResponse: null,
      lastListResponse: null,
      lastCheckResponse: null,
      lastListedMembersResponse: null,
      lastReplayExpectedOrganizationId: null,
      lastReplayObservedOrganizationId: null,
    };
  }
  return world.__orgState;
}
