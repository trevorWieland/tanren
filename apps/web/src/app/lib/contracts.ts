import * as v from "valibot";

import type { components, paths } from "@/app/lib/api-contract.gen";

const REPOSITORY_PATTERN = /^[a-z0-9._-]+\/[a-z0-9._-]+$/;

export type AccountView = components["schemas"]["AccountView"];
export type SessionView = components["schemas"]["SessionEnvelope"];
export type SignUpInput = components["schemas"]["SignUpRequest"];
export type SignInInput = components["schemas"]["SignInRequest"];
export type AcceptInvitationInput =
  components["schemas"]["AcceptInvitationBody"];
export type SignUpResult = components["schemas"]["SignUpResponseCookie"];
export type SignInResult = components["schemas"]["SignInResponseCookie"];
export type AcceptInvitationResult =
  components["schemas"]["AcceptInvitationResponseCookie"];
export const accountFailureCodes = [
  "duplicate_identifier",
  "invalid_credential",
  "invitation_not_found",
  "invitation_already_consumed",
  "invitation_expired",
  "validation_failed",
  "unavailable",
  "internal_error",
] as const;

export type AccountFailureCode = (typeof accountFailureCodes)[number];
export type AccountFailure = Omit<
  components["schemas"]["AccountFailureBody"],
  "code"
> & {
  code: AccountFailureCode;
};

export const accountApiPaths = {
  signUp: "/accounts",
  signIn: "/sessions",
  revokeSession: "/sessions/revoke",
  acceptInvitation: "/invitations/{token}/accept",
} as const satisfies Record<string, keyof paths>;

export type ProjectView = components["schemas"]["ProjectView"];
export type ConnectProjectRepositoryInput =
  components["schemas"]["ConnectProjectRepositoryCookieRequest"];
export type CreateProjectInput =
  components["schemas"]["CreateProjectCookieRequest"];
export type ListVisibleProjectsInput =
  components["schemas"]["ListVisibleProjectsCookieRequest"];
export type ConnectProjectRepositoryResult =
  components["schemas"]["ConnectProjectRepositoryResponse"];
export type CreateProjectResult =
  components["schemas"]["CreateProjectResponse"];
export type ProjectCollectionView =
  components["schemas"]["ProjectCollectionView"];
export const projectFailureCodes = [
  "auth_required",
  "duplicate_repository",
  "no_access",
  "validation_failed",
  "provider_failure",
  "unavailable",
  "internal_error",
] as const;

export type ProjectFailureCode = (typeof projectFailureCodes)[number];
export type ProjectFailure = Omit<
  components["schemas"]["ProjectFailureBody"],
  "code"
> & {
  code: ProjectFailureCode;
};

export const projectApiPaths = {
  connectRepository: "/projects/connect-repository",
  create: "/projects/create",
  listVisible: "/projects/list",
} as const satisfies Record<string, keyof paths>;

export const repositoryRefSchema = v.pipe(
  v.string(),
  v.trim(),
  v.toLowerCase(),
  v.regex(REPOSITORY_PATTERN),
);

export const designatedHostSchema = v.pipe(
  v.string(),
  v.trim(),
  v.minLength(1),
);
export const selectAsActiveSchema = v.boolean();

const projectRepositoryViewSchema = v.strictObject({
  repository: repositoryRefSchema,
});

const projectSelectionViewSchema = v.strictObject({
  is_active: v.boolean(),
  selected_at: v.optional(v.nullable(v.string())),
});

const projectCountsViewSchema = v.strictObject({
  specs: v.number(),
  milestones: v.number(),
  initiatives: v.number(),
});

export const projectViewSchema = v.strictObject({
  id: v.string(),
  owning_account_id: v.string(),
  repository: projectRepositoryViewSchema,
  selection: projectSelectionViewSchema,
  counts: projectCountsViewSchema,
  created_at: v.string(),
});

export const connectProjectRepositoryInputSchema = v.strictObject({
  repository: repositoryRefSchema,
  select_as_active: selectAsActiveSchema,
});

export const createProjectInputSchema = v.strictObject({
  repository: repositoryRefSchema,
  designated_host: designatedHostSchema,
  select_as_active: selectAsActiveSchema,
});

export const listVisibleProjectsInputSchema = v.strictObject({});

export const connectProjectRepositoryResponseSchema = v.strictObject({
  project: projectViewSchema,
});

export const createProjectResponseSchema = v.strictObject({
  project: projectViewSchema,
});

export const projectCollectionViewSchema = v.strictObject({
  owning_account_id: v.string(),
  projects: v.array(projectViewSchema),
});
