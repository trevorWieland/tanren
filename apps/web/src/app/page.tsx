"use client";

import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";
import {
  listRecipientInvitations,
  type OrgInvitationView,
} from "@/app/lib/account-client";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

const statusColor: Record<string, string> = {
  pending:
    "rounded-sm border border-[--color-accent] bg-[--color-accent] px-2 py-0.5 text-xs font-medium text-[--color-accent-fg]",
  accepted:
    "rounded-sm border border-[--color-success] bg-[--color-success] px-2 py-0.5 text-xs font-medium text-[--color-fg-inverse]",
  revoked:
    "rounded-sm border border-[--color-border] bg-[--color-bg-elevated] px-2 py-0.5 text-xs font-medium text-[--color-fg-muted]",
};

const permChip =
  "inline-block rounded-sm bg-[--color-bg-elevated] px-2 py-0.5 text-xs font-medium text-[--color-fg-default]";

function statusLabel(status: string): string {
  switch (status) {
    case "pending":
      return m.invitationStatus_pending();
    case "accepted":
      return m.invitationStatus_accepted();
    case "revoked":
      return m.invitationStatus_revoked();
    default:
      return status;
  }
}

function RecipientInvitations({
  identifier,
}: {
  identifier: string;
}): ReactNode {
  const [invitations, setInvitations] = useState<OrgInvitationView[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    listRecipientInvitations(identifier)
      .then((result) => {
        if (!cancelled) {
          setInvitations(result.invitations);
          setLoaded(true);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLoaded(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [identifier]);

  if (!loaded || invitations.length === 0) {
    if (!loaded) return null;
    return (
      <section aria-label={m.recipientInvitations_title()}>
        <p className="m-0 text-[--color-fg-muted]">
          {m.recipientInvitations_none()}
        </p>
      </section>
    );
  }

  return (
    <section aria-label={m.recipientInvitations_title()}>
      <h2 className="mb-3 text-lg font-semibold">
        {m.recipientInvitations_title()}
      </h2>
      <ul
        className="list-none space-y-2 p-0 m-0"
        data-testid="recipient-invitations"
      >
        {invitations.map((inv) => (
          <li
            key={inv.token}
            className="flex flex-col gap-1.5 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-2"
            data-invitation-token={inv.token}
            data-invitation-status={inv.status}
          >
            <div className="flex items-center gap-2">
              <span className="text-sm text-[--color-fg-muted]">
                {inv.org_id}
              </span>
              <span
                className={statusColor[inv.status] ?? statusColor["pending"]}
              >
                {statusLabel(inv.status)}
              </span>
            </div>
            {inv.permissions.length > 0 && (
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-[--color-fg-muted]">
                  {m.invitationPermissions_label()}:
                </span>
                {inv.permissions.map((perm) => (
                  <span key={perm} className={permChip}>
                    {perm}
                  </span>
                ))}
              </div>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lookupEmail, setLookupEmail] = useState("");
  const [recipientId, setRecipientId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_URL}/health`, { credentials: "include" })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return (await response.json()) as HealthReport;
      })
      .then((data) => {
        if (!cancelled) {
          setReport(data);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  function handleLookup(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const trimmed = lookupEmail.trim();
    if (trimmed.length > 0) {
      setRecipientId(trimmed);
    }
  }

  const inputClass =
    "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

  const buttonClass =
    "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
      <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>
      <section className="min-w-[20rem] rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4 font-mono">
        {report !== null ? (
          <pre className="m-0">{JSON.stringify(report, null, 2)}</pre>
        ) : error !== null ? (
          <span className="text-[--color-error]">
            {m.app_health_unreachable()}: {error}
          </span>
        ) : (
          <span className="text-[--color-fg-muted]">
            {m.app_health_loading()}
          </span>
        )}
      </section>
      <form
        onSubmit={handleLookup}
        className="flex items-end gap-3"
        aria-label={m.recipientInvitations_title()}
      >
        <div className="flex flex-col gap-1">
          <input
            type="email"
            placeholder={m.recipientInvitations_lookupPlaceholder()}
            value={lookupEmail}
            onChange={(event) => {
              setLookupEmail(event.target.value);
            }}
            className={inputClass}
          />
        </div>
        <button type="submit" className={buttonClass}>
          {m.recipientInvitations_lookup()}
        </button>
      </form>
      {recipientId !== null && (
        <RecipientInvitations identifier={recipientId} />
      )}
    </main>
  );
}
