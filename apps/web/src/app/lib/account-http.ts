import type { AccountRequestFailureCode } from "@/app/lib/generated/account-contract";
import { parseAccountRequestFailureCode } from "@/app/lib/generated/account-contract";
import type { WindowContextStrategy } from "@/app/lib/window-context";
import { identityHeader, windowIdHeaderName } from "@/app/lib/window-context";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

// -- Error types --

export interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export interface AccountHttpError {
  code: AccountRequestFailureCode;
  summary: string;
}

// -- Typed helpers --

type JsonDecoder<T> = (payload: unknown) => T | null;

/**
 * Perform a GET request and decode the JSON response. Attaches window
 * context identity and cookie credentials automatically.
 */
export async function getJson<TResponse>(
  path: string,
  decode: JsonDecoder<TResponse>,
  window: WindowContextStrategy,
): Promise<TResponse> {
  const url = `${API_URL}${path}`;
  const headers = identityHeader(window);
  let response: Response;
  try {
    response = await fetch(url, {
      method: "GET",
      headers,
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw httpError(
      "unavailable",
      cause instanceof Error ? cause.message : String(cause),
    );
  }
  return decodeJsonResponse(response, decode, window);
}

/**
 * Perform a POST request with a JSON body and decode the JSON response.
 * Attaches window context identity and cookie credentials automatically.
 */
export async function postJson<TRequest, TResponse>(
  path: string | ((request: TRequest) => string),
  request: TRequest,
  decode: JsonDecoder<TResponse>,
  window: WindowContextStrategy,
  encodeBody?: (request: TRequest) => unknown,
): Promise<TResponse> {
  const url =
    typeof path === "function"
      ? `${API_URL}${path(request)}`
      : `${API_URL}${path}`;
  const headers: Record<string, string> = {
    "content-type": "application/json",
    ...identityHeader(window),
  };
  const body = encodeBody !== undefined ? encodeBody(request) : request;
  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers,
      credentials: "include",
      body: JSON.stringify(body),
    });
  } catch (cause: unknown) {
    throw httpError(
      "unavailable",
      cause instanceof Error ? cause.message : String(cause),
    );
  }
  return decodeJsonResponse(response, decode, window);
}

/**
 * Perform a POST request that returns an empty response (204). Attaches
 * window context identity and cookie credentials automatically.
 */
export async function postEmpty<TRequest>(
  path: string,
  request: TRequest,
  window: WindowContextStrategy,
): Promise<void> {
  const url = `${API_URL}${path}`;
  const headers: Record<string, string> = {
    "content-type": "application/json",
    ...identityHeader(window),
  };
  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers,
      credentials: "include",
      body: JSON.stringify(request),
    });
  } catch (cause: unknown) {
    throw httpError(
      "unavailable",
      cause instanceof Error ? cause.message : String(cause),
    );
  }
  if (!response.ok) {
    throw await decodeHttpError(response, window);
  }
}

// -- Shared internals --

function httpError(
  code: AccountRequestFailureCode,
  summary: string,
): AccountHttpError {
  return { code, summary };
}

function isWindowContextRejection(
  code: AccountRequestFailureCode,
  summary: string,
): boolean {
  if (code !== "validation_failed") {
    return false;
  }
  const normalized = summary.toLowerCase();
  return (
    normalized.includes("window id") || normalized.includes(windowIdHeaderName)
  );
}

async function decodeFailureBody(
  response: Response,
): Promise<FailureBody | null> {
  if (response.status === 204) {
    return null;
  }
  try {
    return decodeFailurePayload(await response.json());
  } catch {
    return null;
  }
}

function decodeFailurePayload(payload: unknown): FailureBody | null {
  if (
    payload === null ||
    typeof payload !== "object" ||
    Array.isArray(payload)
  ) {
    return null;
  }
  const source = payload as Record<string, unknown>;
  return {
    code: source["code"],
    summary: source["summary"],
  };
}

async function decodeHttpError(
  response: Response,
  window: WindowContextStrategy,
): Promise<AccountHttpError> {
  const parsed = await decodeFailureBody(response);
  const code = parseAccountRequestFailureCode(parsed?.code) ?? "internal_error";
  const summary =
    typeof parsed?.summary === "string"
      ? parsed.summary
      : `HTTP ${response.status}`;
  if (isWindowContextRejection(code, summary)) {
    window.rotate();
  }
  return httpError(code, summary);
}

async function decodeJsonResponse<TResponse>(
  response: Response,
  decode: JsonDecoder<TResponse>,
  window: WindowContextStrategy,
): Promise<TResponse> {
  if (!response.ok) {
    throw await decodeHttpError(response, window);
  }
  let payload: unknown;
  try {
    payload = await response.json();
  } catch {
    throw httpError("internal_error", "Invalid response body.");
  }
  const decoded = decode(payload);
  if (decoded === null) {
    throw httpError("internal_error", "Invalid response body.");
  }
  return decoded;
}
