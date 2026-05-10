const DEFAULT_API_URL = "http://127.0.0.1:8081";
const TEST_HOOKS_BASE_PATH = "/test-hooks";
const GLOBAL_SETUP_MARKER_ENV = "TANREN_BDD_TEST_HOOKS_MODE";
const GLOBAL_SETUP_MARKER_VALUE = "spawned";

const GLOBAL_SETUP_INSTRUCTION =
  "Playwright BDD global setup must spawn tanren-api with `cargo run -q -p tanren-api --features test-hooks` so /test-hooks routes exist.";

interface PermissionEventsCheckpointResponse {
  event_ids: string[];
}

interface ResolveAccountResponse {
  account_id: string;
}

interface RequestContext {
  operation: string;
  path: string;
}

let verifiedGlobalSetup = false;

function apiBaseUrl(): string {
  return process.env["NEXT_PUBLIC_API_URL"] ?? DEFAULT_API_URL;
}

function requireGlobalSetupMarker(): void {
  const marker = process.env[GLOBAL_SETUP_MARKER_ENV];
  if (marker !== GLOBAL_SETUP_MARKER_VALUE) {
    throw new Error(
      `${GLOBAL_SETUP_MARKER_ENV}=${marker ?? "<unset>"}; expected ${GLOBAL_SETUP_MARKER_VALUE}. ${GLOBAL_SETUP_INSTRUCTION}`,
    );
  }
}

function ensureJsonObject(
  input: unknown,
  context: RequestContext,
): Record<string, unknown> {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new Error(
      `${context.operation} returned invalid JSON body type from ${context.path}`,
    );
  }
  return input as Record<string, unknown>;
}

function parseCheckpointResponse(
  body: unknown,
  context: RequestContext,
): PermissionEventsCheckpointResponse {
  const json = ensureJsonObject(body, context);
  if (!Array.isArray(json["event_ids"])) {
    throw new Error(
      `${context.operation} response missing event_ids array at ${context.path}`,
    );
  }
  const eventIds = json["event_ids"];
  if (!eventIds.every((id) => typeof id === "string")) {
    throw new Error(
      `${context.operation} response contains non-string event_ids at ${context.path}`,
    );
  }
  return { event_ids: eventIds };
}

function parseResolveAccountResponse(
  body: unknown,
  context: RequestContext,
): ResolveAccountResponse {
  const json = ensureJsonObject(body, context);
  if (typeof json["account_id"] !== "string") {
    throw new Error(
      `${context.operation} response missing string account_id at ${context.path}`,
    );
  }
  return { account_id: json["account_id"] };
}

function formatFailure(
  response: Response,
  responseBody: string,
  context: RequestContext,
): string {
  const prefix = `${context.operation} failed (${response.status} ${response.statusText}) at ${context.path}`;
  if (response.status === 404) {
    return `${prefix}; test-hooks routes were not found. ${GLOBAL_SETUP_INSTRUCTION}`;
  }
  if (responseBody.trim() === "") {
    return prefix;
  }
  return `${prefix}; body=${responseBody}`;
}

async function postTestHook(
  path: string,
  body: unknown,
  operation: string,
): Promise<unknown> {
  const endpointPath = `${TEST_HOOKS_BASE_PATH}${path}`;
  const url = `${apiBaseUrl()}${endpointPath}`;
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });

  const rawBody = await response.text();
  if (!response.ok) {
    throw new Error(
      formatFailure(response, rawBody, { operation, path: endpointPath }),
    );
  }

  if (rawBody === "") {
    return undefined;
  }

  try {
    return JSON.parse(rawBody);
  } catch (error) {
    throw new Error(
      `${operation} returned invalid JSON at ${endpointPath}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function verifyGlobalSetup(): Promise<void> {
  if (verifiedGlobalSetup) {
    return;
  }

  requireGlobalSetupMarker();

  parseCheckpointResponse(
    await postTestHook(
      "/permissions/checkpoints/events",
      { limit: 1 },
      "verify test-hook checkpoint route",
    ),
    {
      operation: "verify test-hook checkpoint route",
      path: `${TEST_HOOKS_BASE_PATH}/permissions/checkpoints/events`,
    },
  );
  verifiedGlobalSetup = true;
}

export interface SeedActorPermissionFixturesBody {
  account_email: string;
  organization_policy_reason: string;
}

export async function seedActorPermissionFixtures(
  payload: SeedActorPermissionFixturesBody,
): Promise<void> {
  await verifyGlobalSetup();
  await postTestHook(
    "/permissions/fixtures/actor",
    payload,
    "seed actor permission fixtures",
  );
}

export async function snapshotPermissionEventsCheckpoint(
  limit = 200,
): Promise<Set<string>> {
  await verifyGlobalSetup();
  const body = parseCheckpointResponse(
    await postTestHook(
      "/permissions/checkpoints/events",
      { limit },
      "snapshot permission events checkpoint",
    ),
    {
      operation: "snapshot permission events checkpoint",
      path: `${TEST_HOOKS_BASE_PATH}/permissions/checkpoints/events`,
    },
  );
  return new Set(body.event_ids);
}

export async function assertNoPermissionMutationEvents(
  eventIdsBefore: Set<string>,
  limit = 200,
): Promise<void> {
  await verifyGlobalSetup();
  await postTestHook(
    "/permissions/assertions/no-request-or-grant-events",
    {
      event_ids_before: [...eventIdsBefore],
      limit,
    },
    "assert no permission mutation events",
  );
}

export async function resolveAccountIdByEmail(
  accountEmail: string,
): Promise<string> {
  await verifyGlobalSetup();
  const body = parseResolveAccountResponse(
    await postTestHook(
      "/accounts/resolve",
      { account_email: accountEmail },
      "resolve account id",
    ),
    {
      operation: "resolve account id",
      path: `${TEST_HOOKS_BASE_PATH}/accounts/resolve`,
    },
  );
  return body.account_id;
}
