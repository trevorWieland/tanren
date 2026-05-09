"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  ProjectRequestError,
  createProject,
  describeProjectFailure,
  type CreateProjectResult,
} from "@/app/lib/project-client";
import * as m from "@/i18n/paraglide/messages";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const REPOSITORY_PATTERN = /^[a-z0-9._-]+\/[a-z0-9._-]+$/;

const CreateProjectInput = v.object({
  owning_account_id: v.pipe(v.string(), v.trim(), v.regex(UUID_PATTERN)),
  repository: v.pipe(
    v.string(),
    v.trim(),
    v.toLowerCase(),
    v.regex(REPOSITORY_PATTERN),
  ),
  designated_host: v.pipe(v.string(), v.trim(), v.minLength(1)),
  select_as_active: v.boolean(),
});

export interface CreateProjectFormProps {
  onSuccess?: ((result: CreateProjectResult) => void) | undefined;
}

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function CreateProjectForm({
  onSuccess,
}: CreateProjectFormProps): ReactNode {
  const baseId = useId();
  const accountId = `${baseId}-owning-account-id`;
  const repositoryId = `${baseId}-repository`;
  const designatedHostId = `${baseId}-designated-host`;
  const selectAsActiveId = `${baseId}-select-as-active`;
  const errorId = `${baseId}-error`;

  const [owningAccountId, setOwningAccountId] = useState("");
  const [repository, setRepository] = useState("");
  const [designatedHost, setDesignatedHost] = useState("");
  const [selectAsActive, setSelectAsActive] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(CreateProjectInput, {
      owning_account_id: owningAccountId,
      repository,
      designated_host: designatedHost,
      select_as_active: selectAsActive,
    });
    if (!parsed.success) {
      setErrorMessage(m.projects_create_required());
      return;
    }

    startTransition(async () => {
      try {
        const result = await createProject(parsed.output);
        onSuccess?.(result);
      } catch (cause: unknown) {
        if (cause instanceof ProjectRequestError) {
          setErrorMessage(describeProjectFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.projects_create_failed());
        }
      }
    });
  }

  const errorActive = errorMessage !== null;

  return (
    <form
      onSubmit={onSubmit}
      noValidate
      aria-label={m.projects_create_formLabel()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={accountId} className="text-sm font-medium">
          {m.projects_owningAccountId()}
        </label>
        <input
          id={accountId}
          name="owning_account_id"
          type="text"
          value={owningAccountId}
          onChange={(event) => {
            setOwningAccountId(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
          placeholder={m.projects_owningAccountIdPlaceholder()}
        />
      </div>
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
      <div className="flex flex-col gap-1">
        <label htmlFor={designatedHostId} className="text-sm font-medium">
          {m.projects_designatedHost()}
        </label>
        <input
          id={designatedHostId}
          name="designated_host"
          type="text"
          value={designatedHost}
          onChange={(event) => {
            setDesignatedHost(event.target.value);
          }}
          aria-describedby={errorActive ? errorId : undefined}
          aria-invalid={errorActive ? true : undefined}
          className={inputClass}
          placeholder={m.projects_designatedHostPlaceholder()}
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
        {pending ? m.projects_create_submitting() : m.projects_create_submit()}
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
