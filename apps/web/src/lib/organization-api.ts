import { useState } from "react";

import type {
  components,
  operations,
  paths,
} from "@/lib/generated/api-contract";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
const DEFAULT_ORGANIZATION_LIST_LIMIT = 50;

type OrganizationCreatePath = "/organizations";
type OrganizationListPath = "/organizations";
type OrganizationPermissionPath = "/organizations/permissions/check";

export const ORGANIZATION_API_ROUTES = {
  create: "/organizations" as OrganizationCreatePath,
  list: "/organizations" as OrganizationListPath,
  checkPermission:
    "/organizations/permissions/check" as OrganizationPermissionPath,
} as const satisfies {
  create: keyof paths;
  list: keyof paths;
  checkPermission: keyof paths;
};

export type OrganizationAdminPermission =
  components["schemas"]["OrganizationPermission"];
export type OrganizationViewResponse =
  components["schemas"]["OrganizationView"];
export type OrganizationProofLink =
  components["schemas"]["OrganizationProofLink"];
export type OrganizationSourceLink =
  components["schemas"]["OrganizationSourceLink"];
export type OrganizationProjectSummary =
  components["schemas"]["OrganizationProjectSummary"];
export type CreateOrganizationApiRequest =
  operations["create_organization_route"]["requestBody"]["content"]["application/json"];
export type CheckOrganizationPermissionApiRequest =
  operations["check_organization_permission_route"]["requestBody"]["content"]["application/json"];
export type CreateOrganizationResponse =
  operations["create_organization_route"]["responses"][201]["content"]["application/json"];
export type ListOrganizationsResponse =
  operations["list_organizations_route"]["responses"][200]["content"]["application/json"];
export type OrganizationListCursor = components["schemas"]["MembershipId"];
export type CheckOrganizationPermissionResponse =
  operations["check_organization_permission_route"]["responses"][200]["content"]["application/json"];
export type ListOrganizationsApiRequest = NonNullable<
  paths["/organizations"]["parameters"]["query"]
>;

type ContractOrganizationFailureCode =
  components["schemas"]["OrganizationFailureCode"];
type LocalOrganizationFailureCode = "unavailable" | "invalid_response";
export type OrganizationFailureCode =
  | ContractOrganizationFailureCode
  | LocalOrganizationFailureCode;

type GeneratedOrganizationErrorResponse =
  components["schemas"]["OrganizationFailureBody"];
export type OrganizationErrorResponse =
  | GeneratedOrganizationErrorResponse
  | { code: LocalOrganizationFailureCode; summary: string };

export interface OrganizationDecodedResponse {
  ok: boolean;
  status: number;
  text: string;
  json: unknown;
  hasValidJson: boolean;
  transportFailure: boolean;
}

export interface OrganizationApiSuccess<TBody> {
  ok: true;
  status: number;
  text: string;
  json: unknown;
  body: TBody;
}

export interface OrganizationApiFailure {
  ok: false;
  status: number;
  text: string;
  json: unknown;
  error: OrganizationErrorResponse;
  transportFallback: boolean;
}

export type OrganizationApiResponse<TBody> =
  | OrganizationApiSuccess<TBody>
  | OrganizationApiFailure;

export interface OrganizationOperationState {
  status: "idle" | "pending" | "success" | "failure";
  failureCode: string | null;
  detail: string | null;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasStringField(value: unknown, key: string): boolean {
  return isObjectRecord(value) && typeof value[key] === "string";
}

function hasOnlyStringFields(value: unknown, keys: readonly string[]): boolean {
  return (
    isObjectRecord(value) && keys.every((key) => typeof value[key] === "string")
  );
}

export function isOrganizationPermission(
  value: unknown,
): value is OrganizationAdminPermission {
  return typeof value === "string" && value.trim() !== "";
}

export function assertOrganizationPermission(
  value: string,
): OrganizationAdminPermission {
  if (isOrganizationPermission(value)) {
    return value;
  }
  throw new Error(`unsupported organization permission '${value}' in scenario`);
}

export function isOrganizationErrorResponse(
  value: unknown,
): value is OrganizationErrorResponse {
  return hasStringField(value, "code") && hasStringField(value, "summary");
}

function isGeneratedOrganizationFailureBody(
  value: unknown,
): value is GeneratedOrganizationErrorResponse {
  return hasStringField(value, "code") && hasStringField(value, "summary");
}

export function isOrganizationViewResponse(
  value: unknown,
): value is OrganizationViewResponse {
  return hasOnlyStringFields(value, ["id", "name"]);
}

export function isOrganizationProofLink(
  value: unknown,
): value is OrganizationProofLink {
  return hasOnlyStringFields(value, ["behavior_id"]);
}

export function isOrganizationSourceLink(
  value: unknown,
): value is OrganizationSourceLink {
  return hasOnlyStringFields(value, ["event_family", "event_kind"]);
}

export function isOrganizationProjectSummary(
  value: unknown,
): value is OrganizationProjectSummary {
  return (
    isObjectRecord(value) &&
    typeof value["total_count"] === "number" &&
    Number.isInteger(value["total_count"]) &&
    value["total_count"] >= 0
  );
}

export function isCreateOrganizationResponse(
  value: unknown,
): value is CreateOrganizationResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (!isOrganizationViewResponse(value["organization"])) {
    return false;
  }

  const permissions = value["granted_permissions"];
  if (!Array.isArray(permissions)) {
    return false;
  }
  if (
    !permissions.every((permission) => isOrganizationPermission(permission))
  ) {
    return false;
  }

  const availablePermissions = value["available_permissions"];
  if (!Array.isArray(availablePermissions)) {
    return false;
  }
  if (
    !availablePermissions.every((permission) =>
      isOrganizationPermission(permission),
    )
  ) {
    return false;
  }

  const initialProjectCount = value["initial_project_count"];
  if (
    typeof initialProjectCount !== "number" ||
    !Number.isInteger(initialProjectCount) ||
    initialProjectCount < 0
  ) {
    return false;
  }

  if (!isOrganizationProofLink(value["proof_link"])) {
    return false;
  }

  if (!isOrganizationSourceLink(value["source_link"])) {
    return false;
  }

  return isOrganizationProjectSummary(value["project_summary"]);
}

export function isListOrganizationsResponse(
  value: unknown,
): value is ListOrganizationsResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  const organizations = value["organizations"];
  if (!Array.isArray(organizations)) {
    return false;
  }
  if (
    value["next_cursor"] !== null &&
    value["next_cursor"] !== undefined &&
    typeof value["next_cursor"] !== "string"
  ) {
    return false;
  }
  return organizations.every((organization) =>
    isOrganizationViewResponse(organization),
  );
}

export function isCheckOrganizationPermissionResponse(
  value: unknown,
): value is CheckOrganizationPermissionResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (!hasOnlyStringFields(value, ["account_id", "org_id", "permission"])) {
    return false;
  }
  if (!isOrganizationPermission(value["permission"])) {
    return false;
  }
  return typeof value["allowed"] === "boolean";
}

export function decodeOrganizationApiResponse<TBody>(
  response: OrganizationDecodedResponse,
  isSuccessBody: (payload: unknown) => payload is TBody,
  operation: string,
): OrganizationApiResponse<TBody> {
  if (response.ok && isSuccessBody(response.json)) {
    return {
      ok: true,
      status: response.status,
      text: response.text,
      json: response.json,
      body: response.json,
    };
  }

  if (
    response.hasValidJson &&
    isGeneratedOrganizationFailureBody(response.json)
  ) {
    return {
      ok: false,
      status: response.status,
      text: response.text,
      json: response.json,
      error: response.json,
      transportFallback: false,
    };
  }

  if (response.transportFailure) {
    return {
      ok: false,
      status: response.status,
      text: response.text,
      json: response.json,
      error: fallbackTransportError(operation, response.text),
      transportFallback: true,
    };
  }

  return {
    ok: false,
    status: response.status,
    text: response.text,
    json: response.json,
    error: {
      code: "invalid_response",
      summary: `${operation} returned an unexpected JSON payload`,
    },
    transportFallback: false,
  };
}

export function formatOrganizationFailureDetail(
  failure: OrganizationApiFailure,
): string {
  const body = failure.text === "" ? "<empty>" : failure.text;
  return `status=${failure.status} code=${failure.error.code} body=${body}`;
}

function fallbackTransportError(
  operation: string,
  text: string,
): OrganizationErrorResponse {
  return {
    code: "unavailable",
    summary: text === "" ? `${operation} transport unavailable` : text,
  };
}

function tryParseJson(text: string): { hasValidJson: boolean; json: unknown } {
  if (text.trim() === "") {
    return { hasValidJson: true, json: null };
  }

  try {
    return { hasValidJson: true, json: JSON.parse(text) as unknown };
  } catch {
    return { hasValidJson: false, json: null };
  }
}

async function callOrganizationApi(
  method: "GET" | "POST",
  path: string,
  payload?: unknown,
): Promise<OrganizationDecodedResponse> {
  const headers: Record<string, string> = {};
  const request: RequestInit = {
    method,
    headers,
    credentials: "include",
  };
  if (payload !== undefined) {
    headers["content-type"] = "application/json";
    request.body = JSON.stringify(payload);
  }

  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, request);
  } catch (cause: unknown) {
    const message = cause instanceof Error ? cause.message : String(cause);
    return {
      ok: false,
      status: 0,
      text: message,
      json: null,
      hasValidJson: false,
      transportFailure: true,
    };
  }

  const text = await response.text();
  const parsed = tryParseJson(text);
  return {
    ok: response.ok,
    status: response.status,
    text,
    json: parsed.json,
    hasValidJson: parsed.hasValidJson,
    transportFailure: false,
  };
}

export async function createOrganizationApi(
  request: CreateOrganizationApiRequest,
): Promise<OrganizationApiResponse<CreateOrganizationResponse>> {
  const response = await callOrganizationApi(
    "POST",
    ORGANIZATION_API_ROUTES.create,
    request,
  );
  return decodeOrganizationApiResponse(
    response,
    isCreateOrganizationResponse,
    "create organization",
  );
}

export async function listOrganizationsApi(
  request: ListOrganizationsApiRequest = {},
): Promise<OrganizationApiResponse<ListOrganizationsResponse>> {
  const params = new URLSearchParams();
  params.set("limit", String(request.limit ?? DEFAULT_ORGANIZATION_LIST_LIMIT));
  if (request.cursor) {
    params.set("cursor", request.cursor);
  }
  const response = await callOrganizationApi(
    "GET",
    `${ORGANIZATION_API_ROUTES.list}?${params.toString()}`,
  );
  return decodeOrganizationApiResponse(
    response,
    isListOrganizationsResponse,
    "list organizations",
  );
}

export async function checkOrganizationPermissionApi(
  request: CheckOrganizationPermissionApiRequest,
): Promise<OrganizationApiResponse<CheckOrganizationPermissionResponse>> {
  const response = await callOrganizationApi(
    "POST",
    ORGANIZATION_API_ROUTES.checkPermission,
    request,
  );
  return decodeOrganizationApiResponse(
    response,
    isCheckOrganizationPermissionResponse,
    "check organization permission",
  );
}

export function useOrganizationOperationState(): {
  operation: OrganizationOperationState;
  operationSequence: number;
  beginOperation: () => void;
  succeedOperation: () => void;
  failOperation: (failure: OrganizationApiFailure) => void;
} {
  const [operationSequence, setOperationSequence] = useState(0);
  const [operation, setOperation] = useState<OrganizationOperationState>({
    status: "idle",
    failureCode: null,
    detail: null,
  });

  function beginOperation(): void {
    setOperationSequence((previous) => previous + 1);
    setOperation({ status: "pending", failureCode: null, detail: null });
  }

  function succeedOperation(): void {
    setOperation({ status: "success", failureCode: null, detail: null });
  }

  function failOperation(failure: OrganizationApiFailure): void {
    setOperation({
      status: "failure",
      failureCode: failure.error.code,
      detail: formatOrganizationFailureDetail(failure),
    });
  }

  return {
    operation,
    operationSequence,
    beginOperation,
    succeedOperation,
    failOperation,
  };
}
