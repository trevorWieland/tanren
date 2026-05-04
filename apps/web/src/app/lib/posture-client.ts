import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "";

export type Posture = "hosted" | "self_hosted" | "local_only";

export type CapabilityCategory =
  | "compute"
  | "storage"
  | "networking"
  | "collaboration"
  | "secrets"
  | "provider_integration";

export type CapabilityAvailability =
  | { status: "available" }
  | { status: "unavailable"; reason: string };

export interface CapabilitySummary {
  category: CapabilityCategory;
  availability: CapabilityAvailability;
}

export interface PostureView {
  posture: Posture;
  capabilities: CapabilitySummary[];
}

export interface PostureChangeView {
  actor: string;
  at: string;
  from: Posture;
  to: Posture;
}

export interface ListPosturesResponse {
  postures: PostureView[];
}

export interface GetPostureResponse {
  current: PostureView;
}

export interface SetPostureResponse {
  current: PostureView;
  change: PostureChangeView;
}

export type PostureFailureCode =
  | "unsupported_posture"
  | "permission_denied"
  | "not_configured"
  | "unauthorized"
  | "validation_failed"
  | "unavailable"
  | "internal_error";

export interface PostureFailure {
  code: PostureFailureCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export function describePostureFailure(failure: PostureFailure): string {
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

export class PostureRequestError extends Error {
  readonly failure: PostureFailure;

  constructor(failure: PostureFailure) {
    super(describePostureFailure(failure));
    this.failure = failure;
    this.name = "PostureRequestError";
  }
}

async function putJson<T>(path: string, body: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new PostureRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let raw: unknown;
    try {
      raw = await response.json();
    } catch {
      raw = {};
    }
    const parsed = (raw ?? {}) as FailureBody;
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new PostureRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

export function setPosture(posture: string): Promise<SetPostureResponse> {
  return putJson<SetPostureResponse>("/v0/posture", { posture });
}
