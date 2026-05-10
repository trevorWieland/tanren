"use client";

import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ChangeEvent, ReactNode } from "react";

import {
  AccountRequestError,
  describeFailure,
  getActiveAccountId,
  listActiveAccounts,
  parseAccountId,
  switchActiveAccount,
  type ListActiveAccountsResult,
  type SignedInAccountView,
  type SwitchActiveAccountInput,
  type SwitchActiveAccountResult,
} from "@/app/lib/account-client";
import type { AccountId } from "@/app/lib/generated/account-contract";
import * as m from "@/i18n/paraglide/messages";

const inputClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

const buttonClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export interface AccountSwitcherProps {
  listAccounts?: () => Promise<ListActiveAccountsResult>;
  switchAccount?: (
    input: SwitchActiveAccountInput,
  ) => Promise<SwitchActiveAccountResult>;
}

export function AccountSwitcher({
  listAccounts = listActiveAccounts,
  switchAccount = switchActiveAccount,
}: AccountSwitcherProps = {}): ReactNode {
  const baseId = useId();
  const selectId = `${baseId}-select`;
  const errorId = `${baseId}-error`;

  const [accounts, setAccounts] = useState<SignedInAccountView[]>([]);
  const [loading, setLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const isMountedRef = useRef(true);

  const activeAccountId = useMemo(
    () => getActiveAccountId(accounts) ?? "",
    [accounts],
  );

  useEffect(() => {
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const refreshAccounts = useCallback(
    async (targetAccountId?: AccountId): Promise<void> => {
      setLoading(true);
      setErrorMessage(null);
      try {
        const result =
          targetAccountId === undefined
            ? await listAccounts()
            : await switchAccount({ target_account_id: targetAccountId });
        if (!isMountedRef.current) {
          return;
        }
        setAccounts(result.accounts);
      } catch (cause: unknown) {
        if (!isMountedRef.current) {
          return;
        }
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else {
          setErrorMessage(m.failure_fallback());
        }
      } finally {
        if (isMountedRef.current) {
          setLoading(false);
        }
      }
    },
    [listAccounts, switchAccount],
  );

  useEffect(() => {
    void refreshAccounts();
  }, [refreshAccounts]);

  function onSwitch(event: ChangeEvent<HTMLSelectElement>): void {
    const targetAccountId = parseAccountId(event.target.value);
    if (targetAccountId === null) {
      setErrorMessage(m.failure_validation_failed());
      return;
    }
    if (targetAccountId === activeAccountId) {
      return;
    }

    void refreshAccounts(targetAccountId);
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
            disabled={loading || !hasAccounts}
            aria-describedby={errorMessage !== null ? errorId : undefined}
            className={inputClass}
          >
            {!hasAccounts ? (
              <option value="">{m.accountSwitcher_empty()}</option>
            ) : null}
            {accounts.map((entry) => (
              <option key={entry.account.id} value={entry.account.id}>
                {entry.account.display_name}
              </option>
            ))}
          </select>
        </div>
        <button
          type="button"
          className={buttonClass}
          disabled={loading}
          onClick={() => {
            void refreshAccounts();
          }}
        >
          {loading ? m.accountSwitcher_loading() : m.accountSwitcher_refresh()}
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
