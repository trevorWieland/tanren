"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  ProjectRequestError,
  connectProjectRepository,
  describeProjectFailure,
  type ConnectProjectRepositoryResult,
} from "@/app/lib/project-client";
import { connectProjectFormSchema } from "@/components/project/project-form";
import * as m from "@/i18n/paraglide/messages";

export interface ConnectRepositoryFormProps {
  onSuccess?: ((result: ConnectProjectRepositoryResult) => void) | undefined;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function ConnectRepositoryForm({
  onSuccess,
}: ConnectRepositoryFormProps): ReactNode {
  const baseId = useId();
  const repositoryId = `${baseId}-repository`;
  const selectAsActiveId = `${baseId}-select-as-active`;
  const errorId = `${baseId}-error`;

  const [repository, setRepository] = useState("");
  const [selectAsActive, setSelectAsActive] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(connectProjectFormSchema, {
      repository,
      select_as_active: selectAsActive,
    });
    if (!parsed.success) {
      setErrorMessage(m.projects_connect_required());
      return;
    }

    startTransition(async () => {
      try {
        const result = await connectProjectRepository(parsed.output);
        onSuccess?.(result);
      } catch (cause: unknown) {
        if (cause instanceof ProjectRequestError) {
          setErrorMessage(describeProjectFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.projects_connect_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.projects_connect_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={repositoryId} className="text-sm font-medium">
          {m.projects_repository()}
        </label>
        <input
          id={repositoryId}
          name="repository"
          type="text"
          value={repository}
          onChange={(event) => {
            setRepository(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
          placeholder={m.projects_repositoryPlaceholder()}
        />
      </div>
      <label
        htmlFor={selectAsActiveId}
        className="flex items-center gap-2 text-sm"
      >
        <input
          id={selectAsActiveId}
          name="select_as_active"
          type="checkbox"
          checked={selectAsActive}
          onChange={(event) => {
            setSelectAsActive(event.target.checked);
          }}
        />
        {m.projects_selectAsActive()}
      </label>
      <button type="submit" disabled={pending} className={buttonClass}>
        {pending
          ? m.projects_connect_submitting()
          : m.projects_connect_submit()}
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
