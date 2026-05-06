"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  ProjectRequestError,
  connectProject,
  describeProjectFailure,
  type ProjectView,
} from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

const ConnectInput = v.object({
  name: v.pipe(v.string(), v.trim(), v.minLength(1)),
  repository_url: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

export interface ConnectProjectFormProps {
  onSuccess?: ((result: ProjectView) => void) | undefined;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function ConnectProjectForm({
  onSuccess,
}: ConnectProjectFormProps): ReactNode {
  const baseId = useId();
  const nameId = `${baseId}-name`;
  const urlId = `${baseId}-url`;
  const errorId = `${baseId}-error`;

  const [name, setName] = useState("");
  const [repositoryUrl, setRepositoryUrl] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(ConnectInput, {
      name,
      repository_url: repositoryUrl,
    });
    if (!parsed.success) {
      setErrorMessage(m.projectConnect_required());
      return;
    }
    startTransition(async () => {
      try {
        const result = await connectProject(parsed.output);
        onSuccess?.(result);
      } catch (cause: unknown) {
        if (cause instanceof ProjectRequestError) {
          setErrorMessage(
            `${cause.failure.code}: ${describeProjectFailure(cause.failure)}`,
          );
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.projectConnect_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.projectConnect_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={nameId} className="text-sm font-medium">
          {m.projectConnect_name()}
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
        <label htmlFor={urlId} className="text-sm font-medium">
          {m.projectConnect_repositoryUrl()}
        </label>
        <input
          id={urlId}
          name="repository_url"
          type="url"
          value={repositoryUrl}
          onChange={(event) => {
            setRepositoryUrl(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
        />
      </div>
      <button type="submit" disabled={pending} className={buttonClass}>
        {pending ? m.projectConnect_submitting() : m.projectConnect_submit()}
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
