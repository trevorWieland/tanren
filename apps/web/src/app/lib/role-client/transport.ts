import {
  toRoleRequestErrorFromFailurePayload,
  toRoleTransportError,
  toRoleValidationError,
} from "./errors";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "";
const ALLOWLISTED_LOOPBACK_API_HOSTS = new Set([
  "127.0.0.1",
  "localhost",
  "::1",
]);

export const ROLE_REQUEST_TIMEOUT_MS = 10_000;

export interface RoleHttpRequestOptions {
  csrfToken?: string;
  signal?: AbortSignal;
  timeoutMs?: number;
  cache?: RequestCache;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function normalizeAbsoluteApiBase(url: URL): string {
  if (url.search.length > 0 || url.hash.length > 0) {
    throw toRoleValidationError(
      "NEXT_PUBLIC_API_URL must not include query params or fragments",
    );
  }
  const pathname = url.pathname === "/" ? "" : trimTrailingSlash(url.pathname);
  return `${url.origin}${pathname}`;
}

function isAllowlistedAbsoluteApiOrigin(url: URL): boolean {
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return false;
  }
  if (
    typeof window !== "undefined" &&
    url.origin.toLowerCase() === window.location.origin.toLowerCase()
  ) {
    return true;
  }
  return ALLOWLISTED_LOOPBACK_API_HOSTS.has(url.hostname.toLowerCase());
}

function resolveApiBasePath(): string {
  const trimmed = API_URL.trim();
  if (trimmed.length === 0) {
    return "";
  }
  if (trimmed.startsWith("/")) {
    return trimTrailingSlash(trimmed);
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw toRoleValidationError(
      "NEXT_PUBLIC_API_URL must be relative or an absolute HTTP(S) URL",
    );
  }

  if (!isAllowlistedAbsoluteApiOrigin(parsed)) {
    throw toRoleValidationError(
      `NEXT_PUBLIC_API_URL origin is not allowlisted: ${parsed.origin}`,
    );
  }

  return normalizeAbsoluteApiBase(parsed);
}

function resolveRoleApiPath(path: string): string {
  return `${resolveApiBasePath()}${path}`;
}

function isAbortError(cause: unknown): boolean {
  return cause instanceof DOMException && cause.name === "AbortError";
}

function toTimeoutError(timeoutMs: number): DOMException {
  return new DOMException(
    `request timed out after ${timeoutMs}ms`,
    "TimeoutError",
  );
}

function composeAbortSignal(
  timeoutMs: number,
  externalSignal?: AbortSignal,
): { signal: AbortSignal; cleanup: () => void } {
  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(toTimeoutError(timeoutMs)),
    timeoutMs,
  );

  if (externalSignal?.aborted) {
    controller.abort(externalSignal.reason);
  }

  const onExternalAbort = (): void => {
    controller.abort(externalSignal?.reason);
  };
  externalSignal?.addEventListener("abort", onExternalAbort, { once: true });

  return {
    signal: controller.signal,
    cleanup: () => {
      clearTimeout(timeout);
      externalSignal?.removeEventListener("abort", onExternalAbort);
    },
  };
}

async function requestRoleJson(
  method: "GET" | "POST",
  path: string,
  body: unknown,
  options?: RoleHttpRequestOptions,
): Promise<unknown> {
  const timeoutMs = options?.timeoutMs ?? ROLE_REQUEST_TIMEOUT_MS;
  const timedSignal = composeAbortSignal(timeoutMs, options?.signal);
  let response: Response;
  try {
    response = await fetch(resolveRoleApiPath(path), {
      method,
      headers: {
        ...(method === "POST" ? { "content-type": "application/json" } : {}),
        ...(options?.csrfToken === undefined
          ? {}
          : { "x-csrf-token": options.csrfToken }),
      },
      ...(method === "POST" ? { body: JSON.stringify(body) } : {}),
      ...(options?.cache === undefined ? {} : { cache: options.cache }),
      credentials: "include",
      signal: timedSignal.signal,
    });
  } catch (cause: unknown) {
    timedSignal.cleanup();
    if (isAbortError(cause)) {
      throw cause;
    }
    throw toRoleTransportError(cause, String(cause));
  }
  timedSignal.cleanup();

  let payload: unknown = undefined;
  try {
    payload = await response.json();
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw toRoleRequestErrorFromFailurePayload(payload);
  }

  return payload;
}

export function getRoleJson(
  path: string,
  options?: Omit<RoleHttpRequestOptions, "csrfToken">,
): Promise<unknown> {
  return requestRoleJson("GET", path, undefined, options);
}

export function postRoleJson(
  path: string,
  body: unknown,
  options?: RoleHttpRequestOptions,
): Promise<unknown> {
  return requestRoleJson("POST", path, body, options);
}
