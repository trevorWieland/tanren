import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface SignUpInput {
  email: string;
  password: string;
  display_name: string;
}

export interface SignInInput {
  email: string;
  password: string;
}

export interface AcceptInvitationInput {
  email: string;
  invitation_token: string;
  password: string;
  display_name: string;
}

export interface AccountView {
  id: string;
  identifier: string;
  display_name: string;
  org: string | null;
}

export type DeploymentPosture = "hosted" | "self_hosted" | "local_only";

export type DeploymentPostureCapability =
  | "managed_control_plane"
  | "provider_integrations"
  | "remote_runtime_dispatch"
  | "local_runtime_dispatch";

export interface DeploymentPostureCapabilitySummary {
  available: DeploymentPostureCapability[];
  unavailable: DeploymentPostureCapability[];
}

export type DeploymentPostureScope =
  | { scope: "account"; account_id: string }
  | { scope: "project"; project_id: string }
  | { scope: "installation"; installation_id: string };

export interface SupportedDeploymentPosture {
  posture: DeploymentPosture;
  capability_summary: DeploymentPostureCapabilitySummary;
}

export interface SetDeploymentPostureResponse {
  scope: DeploymentPostureScope;
  posture: DeploymentPosture;
  capability_summary: DeploymentPostureCapabilitySummary;
}

export interface SetDeploymentPostureRequest {
  scope: DeploymentPostureScope;
  posture: string;
}

export interface DeploymentPostureListResponse {
  supported: SupportedDeploymentPosture[];
}

export interface DeploymentPostureGetResponse {
  current: SetDeploymentPostureResponse | null;
}

let deploymentPostureListCache: Promise<DeploymentPostureListResponse> | null =
  null;

/**
 * Cookie transport: API sets an HTTP-only cookie via tower-sessions on
 * sign-up/sign-in/accept-invitation. The body carries metadata only —
 * the session token itself is never readable from JavaScript.
 */
export interface SessionView {
  account_id: string;
  expires_at: string;
}

export interface SignUpResult {
  account: AccountView;
  session: SessionView;
}

export interface SignInResult {
  account: AccountView;
  session: SessionView;
}

export interface AcceptInvitationResult {
  account: AccountView;
  session: SessionView;
  joined_org: string;
}

/**
 * Stable wire codes from `AccountFailureReason` in `tanren-contract`.
 * Kept in lock-step with the Rust enum so BDD web steps can match on the
 * same taxonomy regardless of transport.
 */
export type AccountFailureCode =
  | "duplicate_identifier"
  | "invalid_credential"
  | "invitation_not_found"
  | "invitation_already_consumed"
  | "invitation_expired"
  | "unsupported_posture"
  | "permission_denied"
  | "scope_not_found"
  | "validation_failed"
  | "unavailable"
  | "internal_error";

export interface AccountFailure {
  code: AccountFailureCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

/**
 * Map an `AccountFailure` to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic "Request failed" string,
 * so unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: AccountFailure): string {
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

export class AccountRequestError extends Error {
  readonly failure: AccountFailure;

  constructor(failure: AccountFailure) {
    super(describeFailure(failure));
    this.failure = failure;
    this.name = "AccountRequestError";
  }
}

async function requestJson<T>(
  method: "GET" | "POST",
  path: string,
  body?: unknown,
): Promise<T> {
  let response: Response;
  try {
    const request: RequestInit = {
      method,
      // Cookie transport: send/receive HTTP-only session cookie on every
      // request. Replaces localStorage token storage (M2).
      credentials: "include",
    };
    if (body !== undefined) {
      request.headers = { "content-type": "application/json" };
      request.body = JSON.stringify(body);
    }
    response = await fetch(`${API_URL}${path}`, request);
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let parsed: FailureBody = {};
    try {
      parsed = (await response.json()) as FailureBody;
    } catch {
      parsed = {};
    }
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new AccountRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  return requestJson<T>("POST", path, body);
}

async function getJson<T>(path: string): Promise<T> {
  return requestJson<T>("GET", path);
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return postJson<SignUpResult>("/accounts", input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return postJson<SignInResult>("/sessions", input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = `/invitations/${encodeURIComponent(token)}/accept`;
  return postJson<AcceptInvitationResult>(path, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
}

export function listDeploymentPostures(): Promise<DeploymentPostureListResponse> {
  return getJson<DeploymentPostureListResponse>("/deployment-postures");
}

export function listDeploymentPosturesCached(): Promise<DeploymentPostureListResponse> {
  if (deploymentPostureListCache === null) {
    deploymentPostureListCache = listDeploymentPostures();
  }
  return deploymentPostureListCache;
}

export function getDeploymentPosture(
  scopeKind: "account" | "project" | "installation",
  scopeId: string,
): Promise<DeploymentPostureGetResponse> {
  const path = `/deployment-postures/${encodeURIComponent(scopeKind)}/${encodeURIComponent(scopeId)}`;
  return getJson<DeploymentPostureGetResponse>(path);
}

export function setDeploymentPosture(
  request: SetDeploymentPostureRequest,
): Promise<SetDeploymentPostureResponse> {
  return postJson<SetDeploymentPostureResponse>(
    "/deployment-postures",
    request,
  );
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}/sessions/revoke`, {
      method: "POST",
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
  if (!response.ok) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
}
