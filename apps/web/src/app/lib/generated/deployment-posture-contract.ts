/**
 * Generated from `tanren-contract` deployment-posture wire shapes.
 *
 * Source of truth: `crates/tanren-contract/src/deployment_posture.rs`.
 */

export type DeploymentPosture = "hosted" | "self_hosted" | "local_only";

export type DeploymentPostureCapability =
  | "managed_control_plane"
  | "provider_integrations"
  | "remote_runtime_dispatch"
  | "local_runtime_dispatch";

export type DeploymentPostureCapabilityUnavailableReason =
  | "requires_managed_control_plane"
  | "requires_provider_integrations"
  | "requires_remote_runtime_dispatch"
  | "requires_local_runtime_dispatch";

export interface DeploymentPostureUnavailableCapability {
  capability: DeploymentPostureCapability;
  reason: DeploymentPostureCapabilityUnavailableReason;
}

export interface DeploymentPostureCapabilitySummary {
  available: DeploymentPostureCapability[];
  unavailable: DeploymentPostureUnavailableCapability[];
}

export type DeploymentPostureScope =
  | { scope: "account"; account_id: string }
  | { scope: "project"; project_id: string }
  | { scope: "installation"; installation_id: string };

export interface SetDeploymentPostureRequest {
  scope: DeploymentPostureScope;
  posture: DeploymentPosture;
}

export interface SetDeploymentPostureResponse {
  scope: DeploymentPostureScope;
  posture: DeploymentPosture;
  capability_summary: DeploymentPostureCapabilitySummary;
}

export interface SupportedDeploymentPosture {
  posture: DeploymentPosture;
  capability_summary: DeploymentPostureCapabilitySummary;
}

export interface SupportedDeploymentPosturesResponse {
  supported: SupportedDeploymentPosture[];
}

export interface DeploymentPostureReadModel {
  scope: DeploymentPostureScope;
  posture: DeploymentPosture;
  capability_summary: DeploymentPostureCapabilitySummary;
}

export interface CurrentDeploymentPostureResponse {
  current: DeploymentPostureReadModel | null;
}
