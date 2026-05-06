import type { components } from "@/generated/api-types";
import * as v from "valibot";

import { API_URL } from "@/app/lib/api-config";

export type ProjectView = components["schemas"]["ProjectView"];
export type ProjectStateSummary = components["schemas"]["ProjectStateSummary"];
export type AttentionSpecView = components["schemas"]["AttentionSpecView"];
export type ScopedViewsResponse = components["schemas"]["ScopedViewsResponse"];
export type SwitchProjectResponse =
  components["schemas"]["SwitchProjectResponse"];

const attentionSpecViewSchema = v.object({
  id: v.string(),
  name: v.string(),
  reason: v.string(),
});

const projectViewSchema = v.object({
  id: v.string(),
  name: v.string(),
  state: v.picklist(["active", "paused", "completed", "archived"]),
  needs_attention: v.boolean(),
  attention_specs: v.array(attentionSpecViewSchema),
  created_at: v.string(),
});

const projectViewArraySchema = v.array(projectViewSchema);

const scopedViewsResponseSchema = v.object({
  project_id: v.string(),
  specs: v.array(v.string()),
  loops: v.array(v.string()),
  milestones: v.array(v.string()),
  view_state: v.optional(v.unknown()),
});

const switchProjectResponseSchema = v.object({
  project: projectViewSchema,
  scoped: v.object({
    project_id: v.string(),
    specs: v.array(v.string()),
    loops: v.array(v.string()),
    milestones: v.array(v.string()),
  }),
});

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

function validateAtBoundary<T>(
  schema: v.BaseSchema<unknown, T, v.BaseIssue<unknown>>,
  data: unknown,
): T {
  const result = v.safeParse(schema, data);
  if (result.success) {
    return result.output as T;
  }
  const detail = result.issues
    .map((i) => i.message)
    .filter((m) => m)
    .join("; ");
  throw new ProjectRequestError(
    "validation_error",
    detail || "Response validation failed",
  );
}

async function fetchValidated<T>(
  path: string,
  schema: v.BaseSchema<unknown, T, v.BaseIssue<unknown>>,
  init?: RequestInit,
): Promise<T> {
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

  const data: unknown = await response.json();
  return validateAtBoundary(schema, data);
}

export function listProjects(): Promise<ProjectView[]> {
  return fetchValidated("/projects", projectViewArraySchema);
}

export function switchProject(
  projectId: string,
): Promise<SwitchProjectResponse> {
  return fetchValidated(
    `/projects/${encodeURIComponent(projectId)}/switch`,
    switchProjectResponseSchema,
    { method: "POST" },
  );
}

export function getAttentionSpec(
  projectId: string,
  specId: string,
): Promise<AttentionSpecView> {
  return fetchValidated(
    `/projects/${encodeURIComponent(projectId)}/specs/${encodeURIComponent(specId)}/attention`,
    attentionSpecViewSchema,
  );
}

export function getActiveProjectViews(): Promise<ScopedViewsResponse> {
  return fetchValidated("/projects/active/views", scopedViewsResponseSchema);
}
