import {
  capabilitySummaryForPosture,
  SUPPORTED_DEPLOYMENT_POSTURES,
} from "../../../../src/app/lib/generated/deployment-posture-fixtures";
import {
  decodeCurrentDeploymentPostureResponse,
  decodeSetDeploymentPostureResponse,
  decodeSupportedDeploymentPosturesResponse,
  isDeploymentPosture as isDeploymentPostureValue,
} from "../../../../src/app/lib/generated/deployment-posture-contract";
import type {
  CurrentDeploymentPostureResponse,
  DeploymentPosture,
  DeploymentPostureCapabilitySummary,
  SetDeploymentPostureResponse,
  SupportedDeploymentPosture,
  SupportedDeploymentPosturesResponse,
} from "../../../../src/app/lib/generated/deployment-posture-contract";

export interface PostureScenarioState {
  actorAccountId?: string;
  otherAccountId?: string;
  lastSupported?: SupportedDeploymentPosture[];
  lastSet?: SetDeploymentPostureResponse;
  lastFailureSummary?: string;
}

export function isDeploymentPosture(raw: string): raw is DeploymentPosture {
  return isDeploymentPostureValue(raw);
}

export function decodeSupportedResponse(
  raw: unknown,
): SupportedDeploymentPosturesResponse {
  return decodeSupportedDeploymentPosturesResponse(raw);
}

export function decodeSetResponse(raw: unknown): SetDeploymentPostureResponse {
  return decodeSetDeploymentPostureResponse(raw);
}

export function decodeCurrentResponse(
  raw: unknown,
): CurrentDeploymentPostureResponse {
  return decodeCurrentDeploymentPostureResponse(raw);
}

export function assertCapabilitySummary(
  posture: DeploymentPosture,
  actual: DeploymentPostureCapabilitySummary,
  context: string,
): void {
  const expected = capabilitySummaryForPosture(posture);
  if (
    JSON.stringify(actual.available) !== JSON.stringify(expected.available) ||
    JSON.stringify(actual.unavailable) !== JSON.stringify(expected.unavailable)
  ) {
    throw new Error(`capability summary mismatch for ${context} (${posture})`);
  }
}

export function assertCanonicalSupportedPostures(
  supported: SupportedDeploymentPosture[],
): void {
  if (supported.length !== SUPPORTED_DEPLOYMENT_POSTURES.length) {
    throw new Error(
      `expected ${SUPPORTED_DEPLOYMENT_POSTURES.length} supported postures, got ${supported.length}`,
    );
  }

  const byPosture = new Map<DeploymentPosture, SupportedDeploymentPosture>(
    supported.map((entry) => [entry.posture, entry]),
  );
  for (const posture of SUPPORTED_DEPLOYMENT_POSTURES) {
    const entry = byPosture.get(posture);
    if (!entry) {
      throw new Error(`missing supported posture ${posture}`);
    }
    assertCapabilitySummary(
      posture,
      entry.capability_summary,
      "supported posture list",
    );
  }
}
