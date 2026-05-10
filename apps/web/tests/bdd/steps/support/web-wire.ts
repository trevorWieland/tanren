import type { Page } from "@playwright/test";

export interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

export interface WebWorld {
  actors: Map<string, ActorState>;
}

export interface NormalizedFailure {
  code: string;
  summary: string;
}

export function actor(world: WebWorld, name: string): ActorState {
  let value = world.actors.get(name);
  if (!value) {
    value = {};
    world.actors.set(name, value);
  }
  return value;
}

export function uniqueEmail(prefix: string): string {
  const nonce = `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${nonce}@example.com`;
}

export function apiUrl(): string {
  return process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
}

export function normalizeFailureBody(
  raw: unknown,
  fallbackStatus: number,
): NormalizedFailure {
  if (typeof raw === "object" && raw !== null) {
    const body = raw as Record<string, unknown>;
    const code =
      typeof body["code"] === "string" ? body["code"] : "internal_error";
    const summary =
      typeof body["summary"] === "string"
        ? body["summary"]
        : `HTTP ${fallbackStatus}`;
    return { code, summary };
  }
  return { code: "internal_error", summary: `HTTP ${fallbackStatus}` };
}

export async function browserJsonRequest(
  page: Page,
  method: "GET" | "POST",
  path: string,
  body?: unknown,
): Promise<{ status: number; ok: boolean; json: unknown }> {
  if (page.url() === "about:blank") {
    await page.goto("/");
  }
  return page.evaluate(
    async ({ requestApiUrl, requestMethod, requestPath, requestBody }) => {
      const init: RequestInit = {
        method: requestMethod,
        credentials: "include",
      };
      if (requestBody !== undefined) {
        init.headers = { "content-type": "application/json" };
        init.body = JSON.stringify(requestBody);
      }
      const response = await fetch(`${requestApiUrl}${requestPath}`, init);
      let json: unknown = {};
      try {
        json = (await response.json()) as unknown;
      } catch {
        json = {};
      }
      return { status: response.status, ok: response.ok, json };
    },
    {
      requestApiUrl: apiUrl(),
      requestMethod: method,
      requestPath: path,
      requestBody: body,
    },
  );
}
