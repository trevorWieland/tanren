import type {
  RoleAdminAction,
  RoleAdminCapabilities,
} from "../generated/role-contract";
import { parseRoleCapabilitySnapshot } from "../generated/role-contract";

import { toRolePermissionDeniedError, toRoleValidationError } from "./errors";
import { getRoleJson } from "./transport";

export interface RoleCapabilitySnapshot {
  capabilities: RoleAdminCapabilities;
  csrfToken: string;
}

export function hasRoleActionCapability(
  capabilities: RoleAdminCapabilities | null,
  action: RoleAdminAction,
): boolean {
  return capabilities?.actions.includes(action) ?? false;
}

export function requireRoleCapabilitySnapshot(
  snapshot: RoleCapabilitySnapshot | null,
): RoleCapabilitySnapshot {
  if (snapshot === null) {
    throw toRoleValidationError("role capabilities are still loading");
  }
  return snapshot;
}

export function requireRoleActionSnapshot<TAction extends RoleAdminAction>(
  snapshot: RoleCapabilitySnapshot | null,
  action: TAction,
): RoleCapabilitySnapshot {
  const resolved = requireRoleCapabilitySnapshot(snapshot);
  if (!hasRoleActionCapability(resolved.capabilities, action)) {
    throw toRolePermissionDeniedError(`missing role capability: ${action}`);
  }
  return resolved;
}

export async function fetchRoleCapabilities(): Promise<RoleCapabilitySnapshot> {
  const payload = await getRoleJson("/roles/capabilities", {
    cache: "no-store",
  });

  try {
    const snapshot = parseRoleCapabilitySnapshot(payload);
    return {
      capabilities: snapshot.capabilities,
      csrfToken: snapshot.csrf_token,
    };
  } catch (cause: unknown) {
    throw toRoleValidationError(
      cause instanceof Error ? cause.message : "unexpected capability payload",
    );
  }
}
