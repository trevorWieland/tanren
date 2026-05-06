"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  AccountRequestError,
  createCredential,
  describeFailure,
  listCredentials,
  removeCredential,
  updateCredential,
  type CredentialKind,
  type RedactedCredentialMetadata,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const CreateInput = v.object({
  kind: v.picklist([
    "api_key",
    "source_control_token",
    "webhook_signing_key",
    "oidc_client_secret",
    "opaque_secret",
  ]),
  name: v.pipe(v.string(), v.trim(), v.minLength(1)),
  value: v.pipe(v.string(), v.minLength(1)),
});

const UpdateInput = v.object({
  value: v.pipe(v.string(), v.minLength(1)),
});

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

const dangerButtonClass =
  "rounded-md border border-[--color-border] bg-transparent px-3 py-1 text-sm text-[--color-error] transition-colors hover:bg-[--color-error] hover:text-white disabled:opacity-60";

const CREDENTIAL_KINDS: CredentialKind[] = [
  "api_key",
  "source_control_token",
  "webhook_signing_key",
  "oidc_client_secret",
  "opaque_secret",
];

interface CredentialRowProps {
  credential: RedactedCredentialMetadata;
  onChanged: () => void;
}

function CredentialRow({
  credential,
  onChanged,
}: CredentialRowProps): ReactNode {
  const [editing, setEditing] = useState(false);
  const [updateValue, setUpdateValue] = useState("");
  const [pending, startTransition] = useTransition();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function handleRemove(): void {
    setErrorMessage(null);
    startTransition(async () => {
      try {
        await removeCredential(credential.id);
        onChanged();
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        }
      }
    });
  }

  function handleUpdate(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(UpdateInput, { value: updateValue });
    if (!parsed.success) {
      setErrorMessage(m.credential_updateRequired());
      return;
    }
    startTransition(async () => {
      try {
        await updateCredential(credential.id, { value: parsed.output.value });
        setEditing(false);
        setUpdateValue("");
        onChanged();
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        }
      }
    });
  }

  return (
    <div className="flex flex-col gap-2 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1">
          <span className="text-base font-medium">{credential.name}</span>
          <span className="text-sm text-[--color-fg-muted]">
            {credential.kind}
            {credential.provider !== null ? ` · ${credential.provider}` : ""}
          </span>
          {credential.description !== null && (
            <span className="text-sm text-[--color-fg-muted]">
              {credential.description}
            </span>
          )}
          <span className="text-xs text-[--color-fg-muted]">
            {m.credential_createdAt()}:{" "}
            {new Date(credential.created_at).toLocaleString()}
            {credential.updated_at !== null && (
              <>
                {" "}
                · {m.credential_updatedAt()}:{" "}
                {new Date(credential.updated_at).toLocaleString()}
              </>
            )}
          </span>
          {credential.present && (
            <span className="text-xs text-[--color-fg-muted]">● Stored</span>
          )}
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => {
              setEditing(!editing);
              setErrorMessage(null);
            }}
            className={dangerButtonClass}
          >
            {editing ? "Cancel" : m.credential_update()}
          </button>
          <button
            type="button"
            onClick={handleRemove}
            disabled={pending}
            className={dangerButtonClass}
          >
            {pending ? m.credential_removing() : m.credential_remove()}
          </button>
        </div>
      </div>

      {editing && (
        <form
          onSubmit={handleUpdate}
          noValidate
          className="flex items-end gap-2"
        >
          <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
            <label className="text-sm font-medium">
              {m.credential_value()}
            </label>
            <input
              type="password"
              autoComplete="new-password"
              value={updateValue}
              onChange={(event) => {
                setUpdateValue(event.target.value);
              }}
              className={inputClass}
            />
          </div>
          <button type="submit" disabled={pending} className={buttonClass}>
            {pending ? m.credential_updating() : m.credential_update()}
          </button>
        </form>
      )}

      {errorMessage !== null && (
        <p role="alert" className="m-0 text-sm text-[--color-error]">
          {errorMessage}
        </p>
      )}
    </div>
  );
}

export function CredentialPanel(): ReactNode {
  const baseId = useId();
  const kindId = `${baseId}-kind`;
  const nameId = `${baseId}-name`;
  const descId = `${baseId}-desc`;
  const providerId = `${baseId}-provider`;
  const valueId = `${baseId}-value`;
  const errorId = `${baseId}-error`;

  const [credentials, setCredentials] = useState<RedactedCredentialMetadata[]>(
    [],
  );
  const [loaded, setLoaded] = useState(false);
  const [kind, setKind] = useState<CredentialKind>("api_key");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [provider, setProvider] = useState("");
  const [secretValue, setSecretValue] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  const [loadPending, startLoadTransition] = useTransition();

  function loadCredentials(): void {
    startLoadTransition(async () => {
      try {
        const resp = await listCredentials();
        setCredentials(resp.credentials);
        setLoaded(true);
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        }
      }
    });
  }

  if (!loaded && !loadPending) {
    loadCredentials();
  }

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(CreateInput, { kind, name, value: secretValue });
    if (!parsed.success) {
      setErrorMessage(m.credential_required());
      return;
    }
    startTransition(async () => {
      try {
        const resp = await createCredential({
          kind: parsed.output.kind,
          name: parsed.output.name,
          value: parsed.output.value,
          ...(description.trim() !== ""
            ? { description: description.trim() }
            : {}),
          ...(provider.trim() !== "" ? { provider: provider.trim() } : {}),
        });
        setCredentials((prev) => [...prev, resp.credential]);
        setName("");
        setDescription("");
        setProvider("");
        setSecretValue("");
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.credential_failed());
        }
      }
    });
  }

  function handleChanged(): void {
    loadCredentials();
  }

  const errorActive = errorMessage !== null;

  return (
    <section className="flex w-full max-w-2xl flex-col gap-4">
      <h2 className="text-xl font-semibold">{m.credential_title()}</h2>

      <form
        onSubmit={onSubmit}
        noValidate
        className="flex flex-col gap-3 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-4"
      >
        <div className="flex flex-wrap gap-3">
          <div className="flex flex-col gap-1">
            <label htmlFor={kindId} className="text-sm font-medium">
              {m.credential_kind()}
            </label>
            <select
              id={kindId}
              value={kind}
              onChange={(event) => {
                setKind(event.target.value as CredentialKind);
              }}
              className={inputClass}
            >
              {CREDENTIAL_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </div>
          <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
            <label htmlFor={nameId} className="text-sm font-medium">
              {m.credential_name()}
            </label>
            <input
              id={nameId}
              type="text"
              autoComplete="off"
              value={name}
              onChange={(event) => {
                setName(event.target.value);
              }}
              aria-describedby={errorActive ? errorId : undefined}
              aria-invalid={errorActive ? true : undefined}
              className={inputClass}
            />
          </div>
        </div>
        <div className="flex flex-wrap gap-3">
          <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
            <label htmlFor={descId} className="text-sm font-medium">
              {m.credential_description()}
            </label>
            <input
              id={descId}
              type="text"
              autoComplete="off"
              value={description}
              onChange={(event) => {
                setDescription(event.target.value);
              }}
              className={inputClass}
            />
          </div>
          <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
            <label htmlFor={providerId} className="text-sm font-medium">
              {m.credential_provider()}
            </label>
            <input
              id={providerId}
              type="text"
              autoComplete="off"
              value={provider}
              onChange={(event) => {
                setProvider(event.target.value);
              }}
              className={inputClass}
            />
          </div>
        </div>
        <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
          <label htmlFor={valueId} className="text-sm font-medium">
            {m.credential_value()}
          </label>
          <input
            id={valueId}
            type="password"
            autoComplete="new-password"
            value={secretValue}
            onChange={(event) => {
              setSecretValue(event.target.value);
            }}
            aria-describedby={errorActive ? errorId : undefined}
            aria-invalid={errorActive ? true : undefined}
            className={inputClass}
          />
        </div>
        <button type="submit" disabled={pending} className={buttonClass}>
          {pending ? m.credential_adding() : m.credential_add()}
        </button>
      </form>

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

      {credentials.length === 0 ? (
        <p className="text-[--color-fg-muted]">{m.credential_empty()}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {credentials.map((cred) => (
            <CredentialRow
              key={cred.id}
              credential={cred}
              onChanged={handleChanged}
            />
          ))}
        </div>
      )}
    </section>
  );
}
