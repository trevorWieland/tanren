import * as m from "@/i18n/paraglide/messages";
import type { components } from "@/api/generated/tanren";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export type ProjectView = components["schemas"]["ProjectView"];
export type ActiveProjectView = components["schemas"]["ActiveProjectView"];
export type ConnectProjectRequest =
  components["schemas"]["ConnectProjectRequest"];
export type CreateProjectRequest =
  components["schemas"]["CreateProjectRequest"];

export type ProjectFailureCode =
  | components["schemas"]["ProjectFailureReason"]
  | "unavailable"
  | "internal_error";

export interface ProjectFailure {
  code: ProjectFailureCode | string;
  summary: string;
}

interface ParsedErrorBody {
  code?: unknown;
  summary?: unknown;
}

export function describeProjectFailure(failure: ProjectFailure): string {
  const key = `project_failure_${failure.code}`;
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

export class ProjectRequestError extends Error {
  readonly failure: ProjectFailure;

  constructor(failure: ProjectFailure) {
    super(describeProjectFailure(failure));
    this.failure = failure;
    this.name = "ProjectRequestError";
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
    throw new ProjectRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let parsed: ParsedErrorBody = {};
    try {
      parsed = (await response.json()) as ParsedErrorBody;
    } catch {
      parsed = {};
    }
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new ProjectRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

async function getJson<T>(path: string): Promise<T | null> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "GET",
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new ProjectRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (response.status === 204) {
    return null;
  }

  if (!response.ok) {
    let parsed: ParsedErrorBody = {};
    try {
      parsed = (await response.json()) as ParsedErrorBody;
    } catch {
      parsed = {};
    }
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new ProjectRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

export function connectProject(
  input: ConnectProjectRequest,
): Promise<ProjectView> {
  return postJson<ProjectView>("/projects/connect", input);
}

export function createProject(
  input: CreateProjectRequest,
): Promise<ProjectView> {
  return postJson<ProjectView>("/projects/create", input);
}

export function getActiveProject(): Promise<ActiveProjectView | null> {
  return getJson<ActiveProjectView>("/projects/active");
}
