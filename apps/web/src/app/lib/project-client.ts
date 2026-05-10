import * as v from "valibot";

import {
  activeProjectRequestSchema,
  activeProjectResponseSchema,
  connectProjectRepositoryRequestSchema,
  connectProjectRepositoryResponseSchema,
  createProjectRequestSchema,
  createProjectResponseSchema,
  listVisibleProjectsRequestSchema,
  listVisibleProjectsResponseSchema,
} from "@/app/lib/api-contract-valibot.gen";
import {
  parseFailureResponse,
  renderFailureEnvelope,
  unavailableFailure,
  type FailureEnvelope,
} from "@/app/lib/failure";
import {
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

function parseProjectResponse<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(schema: TSchema, payload: unknown): v.InferOutput<TSchema> {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return result.output;
  }
  throw new ProjectRequestError({
    code: "internal_error",
    summary: "Response body does not match project contract.",
  });
}

function parseProjectInput<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(schema: TSchema, payload: unknown): v.InferOutput<TSchema> {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return result.output;
  }
  throw new ProjectRequestError({
    code: "validation_failed",
    summary: "Request body does not match project contract.",
  });
}

async function postJson<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(
  path: string,
  body: unknown,
  schema: TSchema,
): Promise<v.InferOutput<TSchema>> {
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
  const parsedInput = parseProjectInput(activeProjectRequestSchema, input);
  return postJson(
    projectApiPaths.active,
    parsedInput,
    activeProjectResponseSchema,
  );
}

export function connectProjectRepository(
  input: ConnectProjectRepositoryInput,
): Promise<ConnectProjectRepositoryResult> {
  const parsedInput = parseProjectInput(
    connectProjectRepositoryRequestSchema,
    input,
  );
  return postJson(
    projectApiPaths.connectRepository,
    parsedInput,
    connectProjectRepositoryResponseSchema,
  );
}

export function createProject(
  input: CreateProjectInput,
): Promise<CreateProjectResult> {
  const parsedInput = parseProjectInput(createProjectRequestSchema, input);
  return postJson(
    projectApiPaths.create,
    parsedInput,
    createProjectResponseSchema,
  );
}

export function listVisibleProjects(
  input: ListVisibleProjectsInput = {},
): Promise<ProjectCollectionView> {
  const parsedInput = parseProjectInput(
    listVisibleProjectsRequestSchema,
    input,
  );
  return postJson(
    projectApiPaths.listVisible,
    parsedInput,
    listVisibleProjectsResponseSchema,
  );
}
