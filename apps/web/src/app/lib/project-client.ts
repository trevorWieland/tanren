import * as v from "valibot";

import {
  parseFailureResponse,
  renderFailureEnvelope,
  unavailableFailure,
  type FailureEnvelope,
} from "@/app/lib/failure";
import {
  activeProjectInputSchema,
  connectProjectRepositoryInputSchema,
  createProjectInputSchema,
  listVisibleProjectsInputSchema,
  projectApiPaths,
  projectFailureCodes,
  type ActiveProjectInput,
  type ActiveProjectResult,
  type ConnectProjectRepositoryInput,
  type ConnectProjectRepositoryResult,
  type CreateProjectInput,
  type CreateProjectResult,
  type ListVisibleProjectsInput,
  type ProjectCollectionView,
  type ProjectFailure,
} from "@/app/lib/contracts";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

const connectProjectRepositoryResponseBoundarySchema = v.strictObject({
  project: v.object({}),
});

const createProjectResponseBoundarySchema = v.strictObject({
  project: v.object({}),
});

const projectCollectionResponseBoundarySchema = v.strictObject({
  owning_account_id: v.string(),
  projects: v.array(v.object({})),
  pagination: v.object({}),
  freshness: v.object({}),
});

const activeProjectResponseBoundarySchema = v.strictObject({
  owning_account_id: v.string(),
  active_project: v.optional(v.nullable(v.object({}))),
});

export function describeProjectFailure(failure: ProjectFailure): string {
  return renderFailureEnvelope(failure);
}

export class ProjectRequestError extends Error {
  readonly failure: ProjectFailure;

  constructor(failure: ProjectFailure) {
    super(describeProjectFailure(failure));
    this.failure = failure;
    this.name = "ProjectRequestError";
  }
}

export type {
  ActiveProjectInput,
  ActiveProjectResult,
  ConnectProjectRepositoryInput,
  ConnectProjectRepositoryResult,
  CreateProjectInput,
  CreateProjectResult,
  ListVisibleProjectsInput,
  ProjectCollectionView,
  ProjectFailure,
} from "@/app/lib/contracts";

function isProjectFailureCode(code: string): code is ProjectFailure["code"] {
  return (projectFailureCodes as readonly string[]).includes(code);
}

function toProjectFailure(failure: FailureEnvelope): ProjectFailure {
  return {
    code: isProjectFailureCode(failure.code) ? failure.code : "internal_error",
    summary: failure.summary,
  };
}

function parseProjectResponse<T>(
  schema: v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
  payload: unknown,
): T {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return payload as T;
  }
  throw new ProjectRequestError({
    code: "internal_error",
    summary: "Response body does not match project contract.",
  });
}

function parseProjectInput<T>(
  schema: v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
  payload: unknown,
): T {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return result.output as T;
  }
  throw new ProjectRequestError({
    code: "validation_failed",
    summary: "Request body does not match project contract.",
  });
}

async function postJson<T>(
  path: string,
  body: unknown,
  schema: v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
): Promise<T> {
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
    throw new ProjectRequestError(toProjectFailure(unavailableFailure(cause)));
  }

  if (!response.ok) {
    const failure = await parseFailureResponse(response);
    throw new ProjectRequestError(toProjectFailure(failure));
  }

  let payload: unknown;
  try {
    payload = (await response.json()) as unknown;
  } catch {
    throw new ProjectRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
  return parseProjectResponse(schema, payload);
}

export function activeProject(
  input: ActiveProjectInput = {},
): Promise<ActiveProjectResult> {
  const parsedInput = parseProjectInput(activeProjectInputSchema, input);
  return postJson<ActiveProjectResult>(
    projectApiPaths.active,
    parsedInput,
    activeProjectResponseBoundarySchema,
  );
}

export function connectProjectRepository(
  input: ConnectProjectRepositoryInput,
): Promise<ConnectProjectRepositoryResult> {
  const parsedInput = parseProjectInput(
    connectProjectRepositoryInputSchema,
    input,
  );
  return postJson<ConnectProjectRepositoryResult>(
    projectApiPaths.connectRepository,
    parsedInput,
    connectProjectRepositoryResponseBoundarySchema,
  );
}

export function createProject(
  input: CreateProjectInput,
): Promise<CreateProjectResult> {
  const parsedInput = parseProjectInput(createProjectInputSchema, input);
  return postJson<CreateProjectResult>(
    projectApiPaths.create,
    parsedInput,
    createProjectResponseBoundarySchema,
  );
}

export function listVisibleProjects(
  input: ListVisibleProjectsInput = {},
): Promise<ProjectCollectionView> {
  const parsedInput = parseProjectInput(listVisibleProjectsInputSchema, input);
  return postJson<ProjectCollectionView>(
    projectApiPaths.listVisible,
    parsedInput,
    projectCollectionResponseBoundarySchema,
  );
}
