import type {
  DeploymentPostureListResponse,
  DeploymentPostureScope,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

export interface OperationalScopeOption {
  key: string;
  label: string;
  scope: DeploymentPostureScope;
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isUuid(raw: string): boolean {
  return UUID_PATTERN.test(raw);
}

export function operationalScopeId(scope: DeploymentPostureScope): string {
  if (scope.scope === "account") {
    return scope.account_id;
  }
  if (scope.scope === "project") {
    return scope.project_id;
  }
  return scope.installation_id;
}

export function operationalScopeKey(scope: DeploymentPostureScope): string {
  return `${scope.scope}:${operationalScopeId(scope)}`;
}

export function operationalScopeLabel(scope: DeploymentPostureScope): string {
  return `${scope.scope}: ${operationalScopeId(scope)}`;
}

export function buildOperationalScopeOptions(
  activeAccountId: string | null,
): OperationalScopeOption[] {
  if (!activeAccountId || !isUuid(activeAccountId)) {
    return [];
  }
  const scope: DeploymentPostureScope = {
    scope: "account",
    account_id: activeAccountId,
  };
  return [
    {
      key: operationalScopeKey(scope),
      label: operationalScopeLabel(scope),
      scope,
    },
  ];
}

export function formatAvailableCapabilities(caps: string[]): string {
  return caps.length === 0 ? m.posture_none() : caps.join(", ");
}

export function formatUnavailableCapabilities(
  unavailable: DeploymentPostureListResponse["supported"][number]["capability_summary"]["unavailable"],
): string {
  if (unavailable.length === 0) {
    return m.posture_none();
  }
  return unavailable
    .map((capability) => `${capability.capability} (${capability.reason})`)
    .join(", ");
}
