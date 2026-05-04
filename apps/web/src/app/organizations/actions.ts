import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface OrganizationView {
  id: string;
  name: string;
  created_at: string;
}

export interface CreateOrganizationRequest {
  name: string;
}

export interface OrgAdminPermissions {
  invite: boolean;
  manage_access: boolean;
  configure: boolean;
  set_policy: boolean;
  delete: boolean;
}

export interface CreateOrganizationResponse {
  organization: OrganizationView;
  membership_permissions: OrgAdminPermissions;
}

export interface ListOrganizationsResponse {
  organizations: OrganizationView[];
}

export type OrganizationFailureCode =
  | "duplicate_name"
  | "unauthenticated"
  | "not_authorized"
  | "validation_failed"
  | "unavailable"
  | "internal_error";

export interface OrganizationFailure {
  code: OrganizationFailureCode | string;
  summary: string;
}

export function describeOrgFailure(failure: OrganizationFailure): string {
  const key = `failure_${failure.code}`;
  const lookup = m as unknown as Record<string, (() => string) | undefined>;
  const fn = lookup[key];
  if (typeof fn === "function") {
    return fn();
  }
  if (failure.summary !== "") {
    return failure.summary;
  }
  return m.failure_fallback();
}

export class OrganizationRequestError extends Error {
  readonly failure: OrganizationFailure;

  constructor(failure: OrganizationFailure) {
    super(describeOrgFailure(failure));
    this.failure = failure;
    this.name = "OrganizationRequestError";
  }
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (response.ok) {
    return (await response.json()) as T;
  }

  let parsed: FailureBody = {};
  try {
    parsed = (await response.json()) as FailureBody;
  } catch {
    parsed = {};
  }

  if (response.status === 401) {
    throw new OrganizationRequestError({
      code: "unauthenticated",
      summary:
        typeof parsed.summary === "string"
          ? parsed.summary
          : "Authentication is required.",
    });
  }

  const code = typeof parsed.code === "string" ? parsed.code : "internal_error";
  const summary =
    typeof parsed.summary === "string"
      ? parsed.summary
      : `HTTP ${response.status}`;
  throw new OrganizationRequestError({ code, summary });
}

export async function createOrganization(
  input: CreateOrganizationRequest,
): Promise<CreateOrganizationResponse> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}/organizations`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(input),
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new OrganizationRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
  return handleResponse<CreateOrganizationResponse>(response);
}

export async function listOrganizations(): Promise<ListOrganizationsResponse> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}/organizations`, {
      method: "GET",
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new OrganizationRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
  return handleResponse<ListOrganizationsResponse>(response);
}
