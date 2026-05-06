import { useId, useState } from "react";
import type { ReactNode } from "react";
import { useMutation } from "@tanstack/react-query";
import * as v from "valibot";

import {
  AccountRequestError,
  describeFailure,
  joinOrganization,
  signIn,
  type JoinOrganizationResult,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const JoinOrgInput = v.object({
  email: v.pipe(v.string(), v.trim(), v.toLowerCase(), v.email()),
  password: v.pipe(v.string(), v.minLength(1)),
});

export interface JoinOrganizationFormProps {
  token: string;
  onSuccess?: ((result: JoinOrganizationResult) => void) | undefined;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

function joinMutationFn({
  email,
  password,
  token,
}: {
  email: string;
  password: string;
  token: string;
}): Promise<JoinOrganizationResult> {
  return signIn({ email, password }).then(() => joinOrganization(token));
}

function toErrorMessage(cause: unknown): string {
  if (cause instanceof AccountRequestError) {
    return describeFailure(cause.failure);
  }
  if (cause instanceof Error) {
    return cause.message;
  }
  return m.joinOrg_failed();
}

export function JoinOrganizationForm({
  token,
  onSuccess,
}: JoinOrganizationFormProps): ReactNode {
  const baseId = useId();
  const emailId = `${baseId}-email`;
  const passwordId = `${baseId}-password`;
  const errorId = `${baseId}-error`;

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: joinMutationFn,
    onSuccess: (result) => {
      onSuccess?.(result);
    },
  });

  const pending = mutation.isPending;
  const apiError = mutation.error ? toErrorMessage(mutation.error) : null;
  const errorMessage = validationError ?? apiError;

  function onSubmit(): void {
    setValidationError(null);
    mutation.reset();
    const parsed = v.safeParse(JoinOrgInput, { email, password });
    if (!parsed.success) {
      setValidationError(m.joinOrg_required());
      return;
    }
    mutation.mutate({
      email: parsed.output.email,
      password: parsed.output.password,
      token,
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      noValidate
      aria-label={m.joinOrg_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={emailId} className="text-sm font-medium">
          {m.signIn_email()}
        </label>
        <input
          id={emailId}
          name="email"
          type="email"
          autoComplete="email"
          value={email}
          onChange={(event) => {
            setEmail(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label htmlFor={passwordId} className="text-sm font-medium">
          {m.signIn_password()}
        </label>
        <input
          id={passwordId}
          name="password"
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(event) => {
            setPassword(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <button type="submit" disabled={pending} className={buttonClass}>
        {pending ? m.joinOrg_submitting() : m.joinOrg_submit()}
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
    </form>
  );
}
