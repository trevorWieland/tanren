import * as v from "valibot";

import type { components, paths } from "@/app/lib/api-contract.gen";

export type AccountView = components["schemas"]["AccountView"];
export type SessionView = components["schemas"]["CookieSessionEnvelope"];
export type SignUpInput = components["schemas"]["SignUpRequest"];
export type SignInInput = components["schemas"]["SignInRequest"];
export type AcceptInvitationInput =
  components["schemas"]["AcceptInvitationBody"];
export type SignUpResult = components["schemas"]["SignUpResponseCookie"];
export type SignInResult = components["schemas"]["SignInResponseCookie"];
export type AcceptInvitationResult =
  components["schemas"]["AcceptInvitationResponseCookie"];
export type ApiAccountFailureCode = components["schemas"]["AccountFailureCode"];
export const accountFailureCodes = [
  "auth_required",
  "duplicate_identifier",
  "invalid_credential",
  "invitation_not_found",
  "invitation_already_consumed",
  "invitation_expired",
  "validation_failed",
  "unavailable",
  "internal_error",
] as const satisfies readonly (ApiAccountFailureCode | "unavailable")[];

export type AccountFailureCode = ApiAccountFailureCode | "unavailable";
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
export type ConnectProjectRepositoryResult =
  components["schemas"]["ConnectProjectRepositoryResponse"];
export type CreateProjectResult =
  components["schemas"]["CreateProjectResponse"];
export type ProjectListCursor = components["schemas"]["ProjectListCursor"];
export type ProjectListFilterRequest =
  components["schemas"]["ProjectListFilterRequest"];
export type ProjectListSelectionFilter =
  components["schemas"]["ProjectListSelectionFilter"];
export type ProjectListSortRequest =
  components["schemas"]["ProjectListSortRequest"];
export type ProjectListSortOrder =
  components["schemas"]["ProjectListSortOrder"];
export type ProjectPageRequest = components["schemas"]["ProjectPageRequest"];
export type ListVisibleProjectsInput =
  components["schemas"]["ListVisibleProjectsCookieRequest"];
export type ProjectPaginationView =
  components["schemas"]["ProjectPaginationView"];
export type ProjectCollectionFreshnessView =
  components["schemas"]["ProjectCollectionFreshnessView"];
export type ProjectCollectionView =
  components["schemas"]["ProjectCollectionView"];
export type ApiProjectFailureCode = components["schemas"]["ProjectFailureCode"];
export const projectFailureCodes = [
  "auth_required",
  "duplicate_repository",
  "no_access",
  "validation_failed",
  "provider_unavailable",
  "provider_failure",
  "unavailable",
  "internal_error",
] as const satisfies readonly (ApiProjectFailureCode | "unavailable")[];

export type ProjectFailureCode = ApiProjectFailureCode | "unavailable";
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
  v.maxLength(140),
  v.check((value) => {
    const slash = value.indexOf("/");
    if (
      slash <= 0 ||
      slash >= value.length - 1 ||
      value.indexOf("/", slash + 1) !== -1
    ) {
      return false;
    }
    const owner = value.slice(0, slash);
    const name = value.slice(slash + 1);
    if (
      owner.length < 1 ||
      owner.length > 39 ||
      name.length < 1 ||
      name.length > 100
    ) {
      return false;
    }
    if (
      !/^[a-z0-9-]+$/.test(owner) ||
      owner.startsWith("-") ||
      owner.endsWith("-")
    ) {
      return false;
    }
    if (owner.includes("--")) {
      return false;
    }
    if (!/^[a-z0-9._-]+$/.test(name)) {
      return false;
    }
    if (/^[-._]|[-._]$/.test(name)) {
      return false;
    }
    return true;
  }),
);

export const designatedHostSchema = v.pipe(
  v.string(),
  v.trim(),
  v.toLowerCase(),
  v.minLength(1),
  v.maxLength(253),
  v.check((value) => {
    if (value.startsWith(".") || value.endsWith(".")) {
      return false;
    }
    return value.split(".").every((label) => {
      if (label.length < 1 || label.length > 63) {
        return false;
      }
      if (label.startsWith("-") || label.endsWith("-")) {
        return false;
      }
      return /^[a-z0-9-]+$/.test(label);
    });
  }),
);
export const selectAsActiveSchema = v.boolean();

const projectRepositoryViewSchema = v.strictObject({
  source_control_host: designatedHostSchema,
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

const projectListCursorSchema = v.strictObject({
  active_selected_at: v.optional(v.nullable(v.string())),
  created_at: v.string(),
  project_id: v.string(),
});

const projectListFilterRequestSchema = v.strictObject({
  selection: v.optional(v.picklist(["all"])),
});

const projectListSortRequestSchema = v.strictObject({
  order: v.optional(v.picklist(["active_selected_then_created_desc"])),
});

const projectPageRequestSchema = v.strictObject({
  cursor: v.optional(v.nullable(projectListCursorSchema)),
  page_size: v.optional(
    v.pipe(v.number(), v.integer(), v.minValue(1), v.maxValue(100)),
  ),
  filter: v.optional(projectListFilterRequestSchema),
  sort: v.optional(projectListSortRequestSchema),
});

export const listVisibleProjectsInputSchema = v.strictObject({
  page: v.optional(projectPageRequestSchema),
});

export const connectProjectRepositoryResponseSchema = v.strictObject({
  project: projectViewSchema,
});

export const createProjectResponseSchema = v.strictObject({
  project: projectViewSchema,
});

export const projectCollectionViewSchema = v.strictObject({
  owning_account_id: v.string(),
  projects: v.array(projectViewSchema),
  pagination: v.strictObject({
    page_size: v.pipe(v.number(), v.integer(), v.minValue(1)),
    default_page_size: v.pipe(v.number(), v.integer(), v.minValue(1)),
    max_page_size: v.pipe(v.number(), v.integer(), v.minValue(1)),
    has_more: v.boolean(),
    next_cursor: v.optional(v.nullable(projectListCursorSchema)),
  }),
  freshness: v.strictObject({
    as_of: v.optional(v.nullable(v.string())),
  }),
});
