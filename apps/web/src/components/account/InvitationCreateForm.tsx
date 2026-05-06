"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  AccountRequestError,
  createOrgInvitation,
  describeFailure,
  type CreateOrgInvitationResult,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const CreateInvitationInput = v.object({
  recipient_identifier: v.pipe(
    v.string(),
    v.trim(),
    v.toLowerCase(),
    v.email(),
  ),
  permissions: v.pipe(v.string(), v.trim(), v.minLength(1)),
  expires_at: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

export interface InvitationCreateFormProps {
  orgId: string;
  onSuccess?: ((result: CreateOrgInvitationResult) => void) | undefined;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function InvitationCreateForm({
  orgId,
  onSuccess,
}: InvitationCreateFormProps): ReactNode {
  const baseId = useId();
  const recipientId = `${baseId}-recipient`;
  const permissionsId = `${baseId}-permissions`;
  const expiresAtId = `${baseId}-expires-at`;
  const errorId = `${baseId}-error`;

  const [recipient, setRecipient] = useState("");
  const [permissions, setPermissions] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    setSuccessMessage(null);
    const parsed = v.safeParse(CreateInvitationInput, {
      recipient_identifier: recipient,
      permissions,
      expires_at: expiresAt,
    });
    if (!parsed.success) {
      setErrorMessage(m.invitationCreate_required());
      return;
    }
    const permList = parsed.output.permissions
      .split(",")
      .map((p) => p.trim())
      .filter((p) => p.length > 0);
    startTransition(async () => {
      try {
        const result = await createOrgInvitation(orgId, {
          recipient_identifier: parsed.output.recipient_identifier,
          permissions: permList,
          expires_at: parsed.output.expires_at,
        });
        setSuccessMessage(m.invitationCreate_success());
        setRecipient("");
        setPermissions("");
        setExpiresAt("");
        onSuccess?.(result);
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.invitationCreate_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.invitationCreate_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <h2 className="m-0 text-lg font-semibold">
        {m.invitationCreate_title()}
      </h2>
      <div className="flex flex-col gap-1">
        <label htmlFor={recipientId} className="text-sm font-medium">
          {m.invitationCreate_recipientIdentifier()}
        </label>
        <input
          id={recipientId}
          name="recipient_identifier"
          type="email"
          autoComplete="email"
          value={recipient}
          onChange={(event) => {
            setRecipient(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label htmlFor={permissionsId} className="text-sm font-medium">
          {m.invitationCreate_permissions()}
        </label>
        <input
          id={permissionsId}
          name="permissions"
          type="text"
          placeholder={m.invitationCreate_permissionsPlaceholder()}
          value={permissions}
          onChange={(event) => {
            setPermissions(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label htmlFor={expiresAtId} className="text-sm font-medium">
          {m.invitationCreate_expiresAt()}
        </label>
        <input
          id={expiresAtId}
          name="expires_at"
          type="datetime-local"
          value={expiresAt}
          onChange={(event) => {
            setExpiresAt(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <button type="submit" disabled={pending} className={buttonClass}>
        {pending
          ? m.invitationCreate_submitting()
          : m.invitationCreate_submit()}
      </button>
      {successMessage !== null && (
        <p className="m-0 text-[--color-success]">{successMessage}</p>
      )}
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
