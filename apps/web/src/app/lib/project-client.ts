import * as v from "valibot";

import {
  parseFailureResponse,
  renderFailureEnvelope,
  unavailableFailure,
  type FailureEnvelope,
} from "@/app/lib/failure";
import {
  connectProjectRepositoryInputSchema,
  connectProjectRepositoryResponseSchema,
  createProjectInputSchema,
  createProjectResponseSchema,
  listVisibleProjectsInputSchema,
  projectApiPaths,
  projectCollectionViewSchema,
  projectFailureCodes,
  type ConnectProjectRepositoryInput,
  type ConnectProjectRepositoryResult,
  type CreateProjectInput,
  type CreateProjectResult,
  type ListVisibleProjectsInput,
  type ProjectCollectionView,
  type ProjectFailure,
} from "@/app/lib/contracts";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

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
    return result.output as T;
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
    connectProjectRepositoryResponseSchema,
  );
}

export function createProject(
  input: CreateProjectInput,
): Promise<CreateProjectResult> {
  const parsedInput = parseProjectInput(createProjectInputSchema, input);
  return postJson<CreateProjectResult>(
    projectApiPaths.create,
    parsedInput,
    createProjectResponseSchema,
  );
}

export function listVisibleProjects(
  input: ListVisibleProjectsInput = {},
): Promise<ProjectCollectionView> {
  const parsedInput = parseProjectInput(listVisibleProjectsInputSchema, input);
  return postJson<ProjectCollectionView>(
    projectApiPaths.listVisible,
    parsedInput,
    projectCollectionViewSchema,
  );
}
