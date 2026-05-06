import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface PreservedFile {
  path: string;
  reason: "UserOwned" | "ModifiedSinceInstall" | "AlreadyRemoved";
}

export interface UninstallPreview {
  to_remove: string[];
  preserved: PreservedFile[];
  manifest_path: string;
}

export interface UninstallPreviewResponse {
  preview: UninstallPreview;
  hosted_data_unchanged: boolean;
}

export interface UninstallResult {
  removed: string[];
  preserved: PreservedFile[];
  manifest_removed: boolean;
}

export interface UninstallApplyResponse {
  result: UninstallResult;
  hosted_data_unchanged: boolean;
}

export type UninstallFailureCode =
  | "manifest_not_found"
  | "manifest_invalid"
  | "confirmation_required"
  | "internal_error"
  | "unavailable";

export interface UninstallFailure {
  code: UninstallFailureCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export function describeUninstallFailure(failure: UninstallFailure): string {
  const key = `uninstall_failure_${failure.code}`;
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

export class UninstallRequestError extends Error {
  readonly failure: UninstallFailure;

  constructor(failure: UninstallFailure) {
    super(describeUninstallFailure(failure));
    this.failure = failure;
    this.name = "UninstallRequestError";
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
    throw new UninstallRequestError({
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
    throw new UninstallRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

export function previewUninstall(
  repoPath: string,
): Promise<UninstallPreviewResponse> {
  return postJson<UninstallPreviewResponse>("/projects/uninstall/preview", {
    repo_path: repoPath,
  });
}

export function applyUninstall(
  repoPath: string,
): Promise<UninstallApplyResponse> {
  return postJson<UninstallApplyResponse>("/projects/uninstall/apply", {
    repo_path: repoPath,
    confirm: true,
  });
}
