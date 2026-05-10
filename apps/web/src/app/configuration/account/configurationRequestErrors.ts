import { AccountRequestError } from "@/app/lib/account-client";
import type { AccountFailure } from "@/app/lib/api-contracts";

export function formatFailure(failure: AccountFailure): string {
  return `code=${failure.code}; summary=${failure.summary}`;
}

export function formatRequestError(error: unknown): string {
  if (error instanceof AccountRequestError) {
    return formatFailure(error.failure);
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
