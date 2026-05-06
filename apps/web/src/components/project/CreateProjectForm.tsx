"use client";

import { useId, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";
import { useMutation } from "@tanstack/react-query";

import {
  ProjectRequestError,
  createProject,
  describeProjectFailure,
} from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

const CreateInput = v.object({
  name: v.pipe(v.string(), v.trim(), v.minLength(1)),
  provider_host: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

export interface CreateProjectFormProps {
  onSuccess?: () => void;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function CreateProjectForm({
  onSuccess,
}: CreateProjectFormProps): ReactNode {
  const baseId = useId();
  const nameId = `${baseId}-name`;
  const hostId = `${baseId}-host`;
  const errorId = `${baseId}-error`;

  const [name, setName] = useState("");
  const [providerHost, setProviderHost] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: createProject,
    onSuccess: () => {
      onSuccess?.();
    },
    onError: (cause: Error) => {
      if (cause instanceof ProjectRequestError) {
        setErrorMessage(
          `${cause.failure.code}: ${describeProjectFailure(cause.failure)}`,
        );
      } else {
        setErrorMessage(
          cause instanceof Error ? cause.message : m.projectCreate_failed(),
        );
      }
    },
  });

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(CreateInput, {
      name,
      provider_host: providerHost,
    });
    if (!parsed.success) {
      setErrorMessage(m.projectCreate_required());
      return;
    }
    mutation.mutate(parsed.output);
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.projectCreate_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={nameId} className="text-sm font-medium">
          {m.projectCreate_name()}
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
      <div className="flex flex-col gap-1">
        <label htmlFor={hostId} className="text-sm font-medium">
          {m.projectCreate_providerHost()}
        </label>
        <input
          id={hostId}
          name="provider_host"
          type="text"
          value={providerHost}
          onChange={(event) => {
            setProviderHost(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <button
        type="submit"
        disabled={mutation.isPending}
        className={buttonClass}
      >
        {mutation.isPending
          ? m.projectCreate_submitting()
          : m.projectCreate_submit()}
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
