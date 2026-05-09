"use client";

import { useEffect, useId, useMemo, useState, useTransition } from "react";
import type { ChangeEvent, ReactNode } from "react";

import {
  AccountRequestError,
  describeFailure,
  listActiveAccounts,
  parseAccountId,
  switchActiveAccount,
  type SignedInAccountView,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export function AccountSwitcher(): ReactNode {
  const baseId = useId();
  const selectId = `${baseId}-select`;
  const errorId = `${baseId}-error`;

  const [accounts, setAccounts] = useState<SignedInAccountView[]>([]);
  const [loading, setLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  const activeAccountId = useMemo(
    () => accounts.find((entry) => entry.is_active)?.account.id ?? "",
    [accounts],
  );

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listActiveAccounts()
      .then((result) => {
        if (!cancelled) {
          setAccounts(result.accounts);
          setErrorMessage(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          if (cause instanceof AccountRequestError) {
            setErrorMessage(describeFailure(cause.failure));
          } else {
            setErrorMessage(m.failure_fallback());
          }
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  function onSwitch(event: ChangeEvent<HTMLSelectElement>): void {
    const targetAccountId = parseAccountId(event.target.value);
    if (targetAccountId === null) {
      setErrorMessage(m.failure_validation_failed());
      return;
    }
    if (targetAccountId === activeAccountId) {
      return;
    }

    setErrorMessage(null);
    startTransition(async () => {
      try {
        const response = await switchActiveAccount({
          target_account_id: targetAccountId,
        });
        setAccounts(response.accounts);
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else {
          setErrorMessage(m.failure_fallback());
        }
      }
    });
  }

  const hasAccounts = accounts.length > 0;

  return (
    <section className="w-full max-w-2xl rounded-lg border border-[--color-border] bg-[--color-bg-surface] p-4 sm:p-6">
      <header className="mb-4 flex flex-col gap-1">
        <h2 className="m-0 text-xl font-semibold">
          {m.accountSwitcher_title()}
        </h2>
        <p className="m-0 text-sm text-[--color-fg-muted]">
          {m.accountSwitcher_subtitle()}
        </p>
      </header>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
        <div className="flex flex-1 flex-col gap-1">
          <label htmlFor={selectId} className="text-sm font-medium">
            {m.accountSwitcher_label()}
          </label>
          <select
            id={selectId}
            value={activeAccountId}
            onChange={onSwitch}
            disabled={loading || pending || !hasAccounts}
            aria-describedby={errorMessage !== null ? errorId : undefined}
            className={inputClass}
          >
            {!hasAccounts ? (
              <option value="">{m.accountSwitcher_empty()}</option>
            ) : null}
            {accounts.map((entry) => (
              <option key={entry.account.id} value={entry.account.id}>
                {entry.account.display_name} ({entry.account.identifier})
              </option>
            ))}
          </select>
        </div>
        <button
          type="button"
          className={buttonClass}
          disabled={loading || pending}
          onClick={() => {
            setLoading(true);
            listActiveAccounts()
              .then((result) => {
                setAccounts(result.accounts);
                setErrorMessage(null);
              })
              .catch((cause: unknown) => {
                if (cause instanceof AccountRequestError) {
                  setErrorMessage(describeFailure(cause.failure));
                } else {
                  setErrorMessage(m.failure_fallback());
                }
              })
              .finally(() => {
                setLoading(false);
              });
          }}
        >
          {loading || pending
            ? m.accountSwitcher_loading()
            : m.accountSwitcher_refresh()}
        </button>
      </div>
      {errorMessage !== null ? (
        <p
          id={errorId}
          role="alert"
          aria-live="polite"
          className="mt-3 mb-0 text-[--color-error]"
        >
          {errorMessage}
        </p>
      ) : null}
    </section>
  );
}
