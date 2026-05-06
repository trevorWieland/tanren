import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface UpgradePreviewRequest {
  root: string;
}

export interface UpgradeApplyRequest {
  root: string;
  confirm: boolean;
}

export interface AssetAction {
  action: "create" | "update" | "remove" | "preserve";
  path: string;
  hash?: string;
  old_hash?: string;
  new_hash?: string;
}

export interface MigrationConcern {
  kind: string;
  path: string;
  detail: string;
}

export interface UpgradePreviewResponse {
  source_version: string;
  target_version: string;
  actions: AssetAction[];
  concerns: MigrationConcern[];
  preserved_user_paths: string[];
}

export type AssetFailureCode =
  | "root_not_found"
  | "manifest_missing"
  | "manifest_parse_error"
  | "unsupported_manifest_version"
  | "confirmation_required"
  | "unreported_drift"
  | "unavailable"
  | "internal_error";

export interface AssetFailure {
  code: AssetFailureCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export function describeAssetFailure(failure: AssetFailure): string {
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

export class AssetRequestError extends Error {
  readonly failure: AssetFailure;

  constructor(failure: AssetFailure) {
    super(describeAssetFailure(failure));
    this.failure = failure;
    this.name = "AssetRequestError";
  }
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AssetRequestError({
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
    throw new AssetRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

export function previewUpgrade(root: string): Promise<UpgradePreviewResponse> {
  return postJson<UpgradePreviewResponse>("/assets/upgrade/preview", {
    root,
  });
}

export function applyUpgrade(root: string): Promise<UpgradePreviewResponse> {
  return postJson<UpgradePreviewResponse>("/assets/upgrade/apply", {
    root,
    confirm: true,
  });
}
