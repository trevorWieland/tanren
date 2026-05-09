import { ORGANIZATION_ADMIN_PERMISSIONS } from "@/lib/organization-routes";

export const ALL_ADMIN_PERMISSIONS = [...ORGANIZATION_ADMIN_PERMISSIONS];

export interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

export interface OrganizationSnapshot {
  id: string;
  grantedPermissions: string[];
  initialProjectCount: number | null;
}

export interface OrganizationWorldState {
  organizationsByName: Map<string, OrganizationSnapshot>;
  lastOperationSucceeded: boolean;
}

export interface OrganizationWorld {
  actors: Map<string, ActorState>;
  __orgState?: OrganizationWorldState;
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
    };
  }
  return world.__orgState;
}
