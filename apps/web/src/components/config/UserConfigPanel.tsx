"use client";

import { useId, useState, useTransition } from "react";
import type { FormEvent, ReactNode } from "react";
import * as v from "valibot";

import {
  AccountRequestError,
  describeFailure,
  listUserConfig,
  removeUserConfig,
  setUserConfig,
  type UserConfigEntry,
  type UserSettingKey,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const SetConfigInput = v.object({
  key: v.picklist(["preferred_harness", "preferred_provider"]),
  value: v.pipe(v.string(), v.trim(), v.minLength(1)),
});

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

const dangerButtonClass =
  "rounded-md border border-[--color-border] bg-transparent px-3 py-1 text-sm text-[--color-error] transition-colors hover:bg-[--color-error] hover:text-white disabled:opacity-60";

interface SettingRowProps {
  entry: UserConfigEntry;
  onRemoved: () => void;
}

function SettingRow({ entry, onRemoved }: SettingRowProps): ReactNode {
  const [pending, startTransition] = useTransition();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function handleRemove(): void {
    setErrorMessage(null);
    startTransition(async () => {
      try {
        await removeUserConfig(entry.key);
        onRemoved();
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
    <div className="flex items-center justify-between gap-3 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="text-sm font-medium">{entry.key}</span>
        <span className="truncate text-base">{entry.value}</span>
        <span className="text-xs text-[--color-fg-muted]">
          {m.userConfig_updatedAt()}:{" "}
          {new Date(entry.updated_at).toLocaleString()}
        </span>
        {errorMessage !== null && (
          <p role="alert" className="m-0 text-sm text-[--color-error]">
            {errorMessage}
          </p>
        )}
      </div>
      <button
        type="button"
        onClick={handleRemove}
        disabled={pending}
        className={dangerButtonClass}
      >
        {pending ? m.userConfig_removing() : m.userConfig_remove()}
      </button>
    </div>
  );
}

export function UserConfigPanel(): ReactNode {
  const baseId = useId();
  const keyId = `${baseId}-key`;
  const valueId = `${baseId}-value`;
  const errorId = `${baseId}-error`;

  const [entries, setEntries] = useState<UserConfigEntry[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [key, setKey] = useState<UserSettingKey>("preferred_harness");
  const [value, setValue] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  const [loadPending, startLoadTransition] = useTransition();

  function loadEntries(): void {
    startLoadTransition(async () => {
      try {
        const resp = await listUserConfig();
        setEntries(resp.entries);
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
    loadEntries();
  }

  function onSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    setErrorMessage(null);
    const parsed = v.safeParse(SetConfigInput, { key, value });
    if (!parsed.success) {
      setErrorMessage(m.failure_validation_failed());
      return;
    }
    startTransition(async () => {
      try {
        const resp = await setUserConfig(
          parsed.output.key,
          parsed.output.value,
        );
        setEntries((prev) => {
          const idx = prev.findIndex((e) => e.key === resp.entry.key);
          if (idx >= 0) {
            const next = [...prev];
            next[idx] = resp.entry;
            return next;
          }
          return [...prev, resp.entry];
        });
        setValue("");
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else if (cause instanceof Error) {
          setErrorMessage(cause.message);
        } else {
          setErrorMessage(m.failure_fallback());
        }
      }
    });
  }

  function handleRemoved(): void {
    loadEntries();
  }

  const errorActive = errorMessage !== null;

  return (
    <section className="flex w-full max-w-2xl flex-col gap-4">
      <h2 className="text-xl font-semibold">{m.userConfig_title()}</h2>

      <form
        onSubmit={onSubmit}
        noValidate
        className="flex flex-wrap items-end gap-3"
      >
        <div className="flex flex-col gap-1">
          <label htmlFor={keyId} className="text-sm font-medium">
            {m.userConfig_key()}
          </label>
          <select
            id={keyId}
            value={key}
            onChange={(event) => {
              setKey(event.target.value as UserSettingKey);
            }}
            className={inputClass}
          >
            <option value="preferred_harness">preferred_harness</option>
            <option value="preferred_provider">preferred_provider</option>
          </select>
        </div>
        <div className="flex min-w-[12rem] flex-1 flex-col gap-1">
          <label htmlFor={valueId} className="text-sm font-medium">
            {m.userConfig_value()}
          </label>
          <input
            id={valueId}
            name="value"
            type="text"
            value={value}
            onChange={(event) => {
              setValue(event.target.value);
            }}
            aria-describedby={errorActive ? errorId : undefined}
            aria-invalid={errorActive ? true : undefined}
            className={inputClass}
          />
        </div>
        <button type="submit" disabled={pending} className={buttonClass}>
          {pending ? m.userConfig_saving() : m.userConfig_set()}
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

      {entries.length === 0 ? (
        <p className="text-[--color-fg-muted]">{m.userConfig_empty()}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {entries.map((entry) => (
            <SettingRow
              key={entry.key}
              entry={entry}
              onRemoved={handleRemoved}
            />
          ))}
        </div>
      )}
    </section>
  );
}
