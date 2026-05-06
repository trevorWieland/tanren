import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface RepositoryView {
  id: string;
  url: string;
}

export interface ProjectContentCounts {
  specs: number;
  milestones: number;
  initiatives: number;
}

export interface ProjectView {
  id: string;
  name: string;
  repository: RepositoryView;
  owner: string;
  org: string | null;
  created_at: string;
  content_counts: ProjectContentCounts;
}

export interface ActiveProjectView {
  project: ProjectView;
  activated_at: string;
}

export interface ConnectProjectInput {
  name: string;
  repository_url: string;
}

export interface CreateProjectInput {
  name: string;
  provider_host: string;
}

export type ProjectFailureCode =
  | "access_denied"
  | "duplicate_repository"
  | "validation_failed"
  | "provider_failure"
  | "provider_not_configured"
  | "unavailable"
  | "internal_error";

export interface ProjectFailure {
  code: ProjectFailureCode | string;
  summary: string;
}

interface FailureBody {
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
    throw new ProjectRequestError({ code, summary });
  }

  return (await response.json()) as T;
}

export function connectProject(
  input: ConnectProjectInput,
): Promise<ProjectView> {
  return postJson<ProjectView>("/projects/connect", input);
}

export function createProject(input: CreateProjectInput): Promise<ProjectView> {
  return postJson<ProjectView>("/projects/create", input);
}

export function getActiveProject(): Promise<ActiveProjectView | null> {
  return getJson<ActiveProjectView>("/projects/active");
}
