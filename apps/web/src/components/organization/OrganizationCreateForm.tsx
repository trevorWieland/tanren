"use client";

import { useId, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import { AccountRequestError, describeFailure } from "@/app/lib/account-client";
import type { CreateOrganizationResponse } from "@/lib/contract-types";
import { useCreateOrganization } from "@/lib/use-organization-queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import * as m from "@/i18n/paraglide/messages";

const OrganizationNameInput = v.object({
  name: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

export interface OrganizationCreateFormProps {
  onSuccess?: ((result: CreateOrganizationResponse) => void) | undefined;
}

export function OrganizationCreateForm({
  onSuccess,
}: OrganizationCreateFormProps): ReactNode {
  const baseId = useId();
  const nameId = `${baseId}-name`;
  const errorId = `${baseId}-error`;

  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const createMutation = useCreateOrganization();
  const pending = createMutation.isPending;

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(OrganizationNameInput, { name });
    if (!parsed.success) {
      setErrorMessage(m.orgCreate_required());
      return;
    }
    createMutation.mutate(parsed.output.name, {
      onSuccess(result) {
        onSuccess?.(result);
      },
      onError(cause: Error) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.orgCreate_failed());
        }
      },
    });
  }

  const errorActive = errorMessage !== null;

  return (
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
        <Input
          id={nameId}
          name="name"
          type="text"
          autoComplete="organization"
          value={name}
          onChange={(event) => {
            setName(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
        />
      </div>
      <Button type="submit" disabled={pending}>
        {pending ? m.orgCreate_submitting() : m.orgCreate_submit()}
      </Button>
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
