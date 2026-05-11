import {
  parseRoleFailure,
  type RoleFailureBody,
} from "../generated/role-contract";

export class RoleRequestError extends Error {
  readonly failure: RoleFailureBody;

  constructor(failure: RoleFailureBody) {
    super(`${failure.code}: ${failure.summary}`);
    this.failure = failure;
    this.name = "RoleRequestError";
  }
}

export function formatRoleError(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

export function toRoleTransportFailure(summary: string): RoleFailureBody {
  return {
    code: "transport_error",
    summary,
  };
}

export function toRoleValidationFailure(summary: string): RoleFailureBody {
  return {
    code: "validation_failed",
    summary,
  };
}

export function toRolePermissionDeniedFailure(
  summary: string,
): RoleFailureBody {
  return {
    code: "permission_denied",
    summary,
  };
}

export function toRoleValidationError(summary: string): RoleRequestError {
  return new RoleRequestError(toRoleValidationFailure(summary));
}

export function toRolePermissionDeniedError(summary: string): RoleRequestError {
  return new RoleRequestError(toRolePermissionDeniedFailure(summary));
}

export function parseRoleFailurePayload(payload: unknown): RoleFailureBody {
  try {
    return parseRoleFailure(payload);
  } catch {
    return toRoleTransportFailure("unexpected failure payload");
  }
}

export function toRoleRequestErrorFromFailurePayload(
  payload: unknown,
): RoleRequestError {
  return new RoleRequestError(parseRoleFailurePayload(payload));
}

export function toRoleTransportError(
  cause: unknown,
  fallbackSummary: string,
): RoleRequestError {
  return new RoleRequestError(
    toRoleTransportFailure(
      cause instanceof Error ? cause.message : fallbackSummary,
    ),
  );
}
