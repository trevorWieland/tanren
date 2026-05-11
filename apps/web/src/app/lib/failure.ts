import * as m from "@/i18n/paraglide/messages";

export interface FailureEnvelope<TCode extends string = string> {
  code: TCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

function hasFailureBodyShape(value: unknown): value is FailureBody {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  return true;
}

export function parseFailureEnvelope(
  body: unknown,
  status: number,
): FailureEnvelope<string> {
  if (!hasFailureBodyShape(body)) {
    return {
      code: "internal_error",
      summary: `HTTP ${status}`,
    };
  }
  const code = typeof body.code === "string" ? body.code : "internal_error";
  const summary =
    typeof body.summary === "string" ? body.summary : `HTTP ${status}`;
  return { code, summary };
}

export async function parseFailureResponse(
  response: Response,
): Promise<FailureEnvelope<string>> {
  try {
    const body = (await response.json()) as unknown;
    return parseFailureEnvelope(body, response.status);
  } catch {
    return {
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    };
  }
}

export function unavailableFailure(
  cause: unknown,
): FailureEnvelope<"unavailable"> {
  return {
    code: "unavailable",
    summary: cause instanceof Error ? cause.message : String(cause),
  };
}

export function renderFailureEnvelope(failure: FailureEnvelope): string {
  const key = `failure_${failure.code}`;
  const lookup = m as unknown as Record<string, (() => string) | undefined>;
  const localized = lookup[key];
  if (typeof localized === "function") {
    return localized();
  }
  if (failure.summary !== "") {
    return failure.summary;
  }
  return m.failure_fallback();
}
