const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export interface ProjectView {
  id: string;
  name: string;
  state: ProjectStateSummary;
  needs_attention: boolean;
  attention_specs: AttentionSpecView[];
  created_at: string;
}

export type ProjectStateSummary =
  | "active"
  | "paused"
  | "completed"
  | "archived";

export interface AttentionSpecView {
  id: string;
  name: string;
  reason: string;
}

export interface ScopedViewsResponse {
  project_id: string;
  specs: string[];
  loops: string[];
  milestones: string[];
  view_state: unknown;
}

export interface SwitchProjectResponse {
  project: ProjectView;
  scoped: {
    project_id: string;
    specs: string[];
    loops: string[];
    milestones: string[];
  };
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export class ProjectRequestError extends Error {
  readonly code: string;

  constructor(code: string, summary: string) {
    super(summary);
    this.code = code;
    this.name = "ProjectRequestError";
  }
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      ...init,
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new ProjectRequestError(
      "unavailable",
      cause instanceof Error ? cause.message : String(cause),
    );
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
    throw new ProjectRequestError(code, summary);
  }

  return (await response.json()) as T;
}

export function listProjects(): Promise<ProjectView[]> {
  return fetchJson<ProjectView[]>("/projects");
}

export function switchProject(
  projectId: string,
): Promise<SwitchProjectResponse> {
  return fetchJson<SwitchProjectResponse>(
    `/projects/${encodeURIComponent(projectId)}/switch`,
    { method: "POST" },
  );
}

export function getAttentionSpec(
  projectId: string,
  specId: string,
): Promise<AttentionSpecView> {
  return fetchJson<AttentionSpecView>(
    `/projects/${encodeURIComponent(projectId)}/specs/${encodeURIComponent(specId)}/attention`,
  );
}

export function getActiveProjectViews(): Promise<ScopedViewsResponse> {
  return fetchJson<ScopedViewsResponse>("/projects/active/views");
}
