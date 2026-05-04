"use client";

import { useEffect, useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import { useRouter } from "next/navigation";
import * as v from "valibot";

import {
  OrganizationRequestError,
  createOrganization,
  listOrganizations,
  type CreateOrganizationResponse,
} from "../actions";
import * as m from "@/i18n/paraglide/messages";

const CreateOrgInput = v.object({
  name: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export default function NewOrganizationPage(): ReactNode {
  const router = useRouter();
  const baseId = useId();
  const nameId = `${baseId}-name`;
  const errorId = `${baseId}-error`;

  const [name, setName] = useState("");
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  const [created, setCreated] = useState<CreateOrganizationResponse | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    listOrganizations().catch((cause: unknown) => {
      if (cancelled) return;
      if (
        cause instanceof OrganizationRequestError &&
        cause.failure.code === "unauthenticated"
      ) {
        router.push("/sign-in");
      }
    });
    return () => {
      cancelled = true;
    };
  }, [router]);

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorCode(null);
    setErrorMessage(null);
    const parsed = v.safeParse(CreateOrgInput, { name });
    if (!parsed.success) {
      setErrorMessage(m.orgCreate_required());
      return;
    }
    startTransition(async () => {
      try {
        const result = await createOrganization(parsed.output);
        setCreated(result);
      } catch (cause: unknown) {
        if (
          cause instanceof OrganizationRequestError &&
          cause.failure.code === "unauthenticated"
        ) {
          router.push("/sign-in");
          return;
        }
        if (cause instanceof OrganizationRequestError) {
          setErrorCode(cause.failure.code);
          setErrorMessage(cause.message);
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.orgCreate_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.orgCreate_title()}</h1>
      {created !== null ? (
        <div data-testid="created-organization">
          <p>{created.organization.name}</p>
        </div>
      ) : (
        <form
          onSubmit={onSubmit}
          noValidate
          aria-label={m.orgCreate_formLabel()}
          className="flex w-full max-w-md flex-col gap-4"
        >
          <div className="flex flex-col gap-1">
            <label htmlFor={nameId} className="text-sm font-medium">
              {m.orgCreate_name()}
            </label>
            <input
              id={nameId}
              name="name"
              type="text"
              value={name}
              onChange={(event) => {
                setName(event.target.value);
              }}
              aria-describedby={errorActive ? errorId : undefined}
              aria-invalid={errorActive ? true : undefined}
              className={inputClass}
            />
          </div>
          <button type="submit" disabled={pending} className={buttonClass}>
            {pending ? m.orgCreate_submitting() : m.orgCreate_submit()}
          </button>
          {errorActive && (
            <p
              id={errorId}
              role="alert"
              aria-live="polite"
              className="m-0 text-[--color-error]"
            >
              {errorMessage}
            </p>
          )}
          {errorCode !== null && (
            <span data-testid="error-code" className="sr-only">
              {errorCode}
            </span>
          )}
        </form>
      )}
    </main>
  );
}
