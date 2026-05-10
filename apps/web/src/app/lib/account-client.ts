import * as m from "@/i18n/paraglide/messages";
import type {
  InterfaceError,
  MyAccountCapabilitiesResponse,
  MyPermissionEntry,
  MyPermissionsResponse,
  PermissionConstraintView,
  PermissionGrantSource,
  operations,
  paths,
} from "@/app/lib/generated-interface-contracts";
import { isInterfaceErrorCode } from "@/app/lib/generated-interface-contracts";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

type OperationShape = {
  parameters: {
    query?: unknown;
  };
  responses: Record<number, unknown>;
  requestBody?: {
    content: {
      "application/json": unknown;
    };
  };
};

type JsonRequestBody<T extends OperationShape> = T extends {
  requestBody: {
    content: {
      "application/json": infer Body;
    };
  };
}
  ? Body
  : never;

type JsonResponseBody<
  T extends OperationShape,
  Status extends keyof T["responses"],
> = T["responses"][Status] extends {
  content: {
    "application/json": infer Body;
  };
}
  ? Body
  : never;

type SuccessStatusCode<T extends OperationShape> = Extract<
  keyof T["responses"],
  200 | 201 | 202 | 203 | 204 | 205 | 206 | 207 | 208 | 226
>;

type ErrorStatusCode<T extends OperationShape> = Exclude<
  Extract<keyof T["responses"], number>,
  SuccessStatusCode<T>
>;

type OperationSuccessBody<T extends OperationShape> = JsonResponseBody<
  T,
  SuccessStatusCode<T>
>;

type OperationErrorBody<T extends OperationShape> = {
  [Status in ErrorStatusCode<T>]: JsonResponseBody<T, Status>;
}[ErrorStatusCode<T>];

type SignUpOperation = operations["sign_up_route"];
type SignInOperation = operations["sign_in_route"];
type AcceptInvitationOperation = operations["accept_invitation_route"];
type RevokeOperation = operations["revoke_route"];
type MyPermissionsOperation = paths["/me/permissions"]["get"];
type MyCapabilitiesOperation = paths["/me/capabilities"]["get"];

export type SignUpInput = JsonRequestBody<SignUpOperation>;
export type SignInInput = JsonRequestBody<SignInOperation>;
export type AcceptInvitationInput =
  JsonRequestBody<AcceptInvitationOperation> & {
    invitation_token: string;
  };

export type SignUpResult = OperationSuccessBody<SignUpOperation>;
export type SignInResult = OperationSuccessBody<SignInOperation>;
export type AcceptInvitationResult =
  OperationSuccessBody<AcceptInvitationOperation>;

export type AccountView = SignInResult["account"];

/**
 * Cookie transport: API sets an HTTP-only cookie via tower-sessions on
 * sign-up/sign-in/accept-invitation. The body carries metadata only —
 * the session token itself is never readable from JavaScript.
 */
export type SessionView = SignInResult["session"];

export type MyPermissionsQuery = NonNullable<
  MyPermissionsOperation["parameters"]["query"]
>;
export type MyCapabilitiesQuery = NonNullable<
  MyCapabilitiesOperation["parameters"]["query"]
>;

export type PermissionScopeView =
  | {
      kind: "organization";
      scope_id: string;
      permissions: MyPermissionEntry[];
    }
  | {
      kind: "project";
      scope_id: string;
      permissions: MyPermissionEntry[];
    };
export type {
  InterfaceError,
  MyAccountCapabilitiesResponse,
  MyPermissionsResponse,
  MyPermissionEntry,
  PermissionConstraintView,
  PermissionGrantSource,
};

type AccountOperationError =
  | OperationErrorBody<SignUpOperation>
  | OperationErrorBody<SignInOperation>
  | OperationErrorBody<AcceptInvitationOperation>
  | OperationErrorBody<MyPermissionsOperation>
  | OperationErrorBody<MyCapabilitiesOperation>
  | OperationErrorBody<RevokeOperation>;

export type AccountFailure = AccountOperationError;

function scopeLabel(scope: PermissionScopeView): string {
  switch (scope.kind) {
    case "organization":
      return scope.scope_id;
    case "project":
      return scope.scope_id;
  }
}

export function permissionScopes(
  response: MyPermissionsResponse,
): PermissionScopeView[] {
  const organizations: PermissionScopeView[] = response.organizations.map(
    (section) => ({
      kind: "organization",
      scope_id: section.org_id,
      permissions: section.permissions,
    }),
  );
  const projects: PermissionScopeView[] = response.projects.map((section) => ({
    kind: "project",
    scope_id: section.project_id,
    permissions: section.permissions,
  }));
  return [...organizations, ...projects].sort((left, right) =>
    scopeLabel(left).localeCompare(scopeLabel(right)),
  );
}

/**
 * Map a request failure to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic fallback string, so
 * unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: AccountFailure): string {
  switch (failure.code) {
    case "duplicate_identifier":
      return m.failure_duplicate_identifier();
    case "invalid_credential":
      return m.failure_invalid_credential();
    case "validation_failed":
      return m.failure_validation_failed();
    case "invitation_not_found":
      return m.failure_invitation_not_found();
    case "invitation_already_consumed":
      return m.failure_invitation_already_consumed();
    case "invitation_expired":
      return m.failure_invitation_expired();
    case "auth_required":
      return m.failure_auth_required();
    case "permission_denied":
      return m.failure_permission_denied();
    case "unavailable":
      return m.failure_unavailable();
    case "internal_error":
      return m.failure_internal_error();
    default:
      if (failure.summary !== "") {
        return failure.summary;
      }
      return m.failure_fallback();
  }
}

function safeSingleLine(value: string): string {
  return value.replaceAll(/\s+/g, " ").trim();
}

function summarizeUnknownInterfaceCode(
  rawCode: string,
  summary: string,
  fallbackStatus: number,
): string {
  const codeText = safeSingleLine(rawCode);
  const summaryText = safeSingleLine(summary);
  const statusText =
    fallbackStatus > 0 ? ` status=HTTP ${fallbackStatus};` : "";
  if (summaryText !== "") {
    return `Unknown interface error code.${statusText} raw_code=${codeText}; server_summary=${summaryText}`;
  }
  return `Unknown interface error code.${statusText} raw_code=${codeText}`;
}

export class AccountRequestError extends Error {
  readonly failure: AccountFailure;

  constructor(failure: AccountFailure) {
    super(describeFailure(failure));
    this.failure = failure;
    this.name = "AccountRequestError";
  }
}

function normalizeInterfaceError(
  payload: unknown,
  fallbackStatus: number,
): AccountFailure {
  if (
    typeof payload === "object" &&
    payload !== null &&
    "code" in payload &&
    "summary" in payload
  ) {
    const code = (payload as { code: unknown }).code;
    const summary = (payload as { summary: unknown }).summary;
    if (typeof code === "string" && typeof summary === "string") {
      if (isInterfaceErrorCode(code)) {
        return { code, summary };
      }
      return {
        code: "drift_detected",
        summary: summarizeUnknownInterfaceCode(code, summary, fallbackStatus),
      };
    }
  }
  return {
    code: "internal_error",
    summary: fallbackStatus > 0 ? `HTTP ${fallbackStatus}` : "",
  };
}

type RouteTemplate = Extract<keyof paths, string>;
type ConcreteRoutePath<Path extends RouteTemplate> =
  Path extends "/invitations/{token}/accept"
    ? `/invitations/${string}/accept`
    : Path;
type RoutePath = {
  [Path in RouteTemplate]: ConcreteRoutePath<Path>;
}[RouteTemplate];
type RequestPath = RoutePath | `${RoutePath}?${string}`;

async function requestJson<T>(
  path: RequestPath,
  method: "GET" | "POST",
  body?: unknown,
): Promise<T> {
  const requestInit: RequestInit =
    method === "POST"
      ? {
          method,
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body),
          credentials: "include",
        }
      : {
          method,
          credentials: "include",
        };
  let response: Response;
  try {
    // Cookie transport: send/receive HTTP-only session cookie on every
    // request. Replaces localStorage token storage (M2).
    response = await fetch(`${API_URL}${path}`, requestInit);
  } catch (cause: unknown) {
    const failure: InterfaceError = {
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    };
    throw new AccountRequestError(failure);
  }

  if (!response.ok) {
    let parsed: unknown = null;
    try {
      parsed = await response.json();
    } catch {
      parsed = null;
    }
    throw new AccountRequestError(
      normalizeInterfaceError(parsed, response.status),
    );
  }

  return (await response.json()) as T;
}

async function postJson<T>(path: RoutePath, body: unknown): Promise<T> {
  return requestJson<T>(path, "POST", body);
}

async function postNoContent(path: RoutePath): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "POST",
      credentials: "include",
    });
  } catch (cause: unknown) {
    const failure: InterfaceError = {
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    };
    throw new AccountRequestError(failure);
  }

  if (!response.ok) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
}

const SIGN_UP_PATH: Extract<keyof paths, "/accounts"> = "/accounts";
const SIGN_IN_PATH: Extract<keyof paths, "/sessions"> = "/sessions";
const INVITATION_ACCEPT_TEMPLATE: Extract<
  keyof paths,
  "/invitations/{token}/accept"
> = "/invitations/{token}/accept";
const MY_PERMISSIONS_PATH: Extract<keyof paths, "/me/permissions"> =
  "/me/permissions";
const MY_PERMISSIONS_METHOD = "GET" as const;
const MY_CAPABILITIES_PATH: Extract<keyof paths, "/me/capabilities"> =
  "/me/capabilities";
const MY_CAPABILITIES_METHOD = "GET" as const;
const SIGN_OUT_PATH: Extract<keyof paths, "/sessions/revoke"> =
  "/sessions/revoke";

function acceptInvitationPath(
  token: string,
): ConcreteRoutePath<typeof INVITATION_ACCEPT_TEMPLATE> {
  return `/invitations/${encodeURIComponent(token)}/accept`;
}

function queryPath<Path extends RoutePath>(
  path: Path,
  query: URLSearchParams,
): Path | `${Path}?${string}` {
  const search = query.toString();
  if (search === "") {
    return path;
  }
  return `${path}?${search}`;
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return postJson<SignUpResult>(SIGN_UP_PATH, input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return postJson<SignInResult>(SIGN_IN_PATH, input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = acceptInvitationPath(token);
  return postJson<AcceptInvitationResult>(path, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
}

export function myPermissions(
  query: MyPermissionsQuery = {},
): Promise<OperationSuccessBody<MyPermissionsOperation>> {
  const params = new URLSearchParams();
  if (query.limit !== undefined) {
    params.set("limit", String(query.limit));
  }
  if (query.cursor && query.cursor.trim() !== "") {
    params.set("cursor", query.cursor);
  }
  return requestJson<OperationSuccessBody<MyPermissionsOperation>>(
    queryPath(MY_PERMISSIONS_PATH, params),
    MY_PERMISSIONS_METHOD,
  );
}

export function myAccountCapabilities(
  query: MyCapabilitiesQuery = {},
): Promise<OperationSuccessBody<MyCapabilitiesOperation>> {
  const params = new URLSearchParams();
  if (query.account_id && query.account_id.trim() !== "") {
    params.set("account_id", query.account_id);
  }
  return requestJson<OperationSuccessBody<MyCapabilitiesOperation>>(
    queryPath(MY_CAPABILITIES_PATH, params),
    MY_CAPABILITIES_METHOD,
  );
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  return postNoContent(SIGN_OUT_PATH);
}
