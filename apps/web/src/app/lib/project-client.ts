import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface ProjectRepositoryView {
  repository: string;
}

export interface ProjectSelectionView {
  is_active: boolean;
  selected_at: string | null;
}

export interface ProjectCountsView {
  specs: number;
  milestones: number;
  initiatives: number;
}

export interface ProjectView {
  id: string;
  owning_account_id: string;
  repository: ProjectRepositoryView;
  selection: ProjectSelectionView;
  counts: ProjectCountsView;
  created_at: string;
}

export interface ConnectProjectRepositoryInput {
  repository: string;
  select_as_active: boolean;
}

export interface ConnectProjectRepositoryResult {
  project: ProjectView;
}

export interface CreateProjectInput {
  repository: string;
  designated_host: string;
  select_as_active: boolean;
}

export interface CreateProjectResult {
  project: ProjectView;
}

export type ListVisibleProjectsInput = Record<string, never>;

export interface ProjectCollectionView {
  owning_account_id: string;
  projects: ProjectView[];
}

export type ProjectFailureCode =
  | "auth_required"
  | "duplicate_repository"
  | "no_access"
  | "validation_failed"
  | "provider_failure"
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
      // Cookie transport: session token remains HTTP-only and never lands
      // in JavaScript-visible storage.
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

export function connectProjectRepository(
  input: ConnectProjectRepositoryInput,
): Promise<ConnectProjectRepositoryResult> {
  return postJson<ConnectProjectRepositoryResult>(
    "/projects/connect-repository",
    input,
  );
}

export function createProject(
  input: CreateProjectInput,
): Promise<CreateProjectResult> {
  return postJson<CreateProjectResult>("/projects/create", input);
}

export function listVisibleProjects(
  input: ListVisibleProjectsInput = {},
): Promise<ProjectCollectionView> {
  return postJson<ProjectCollectionView>("/projects/list", input);
}
