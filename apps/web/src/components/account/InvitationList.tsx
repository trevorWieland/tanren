"use client";

import { useCallback, useEffect, useState, useTransition } from "react";
import type { ReactNode } from "react";

import {
  type OrgInvitationView,
  listOrgInvitations,
  revokeOrgInvitation,
  type ListOrgInvitationsResult,
  AccountRequestError,
  describeFailure,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

const chipBase = "inline-block rounded-sm px-2 py-0.5 text-xs font-medium";

const statusColors: Record<string, string> = {
  pending: `${chipBase} bg-[--color-accent]/20 text-[--color-accent]`,
  accepted: `${chipBase} bg-[--color-success]/20 text-[--color-success]`,
  revoked: `${chipBase} bg-[--color-fg-muted]/20 text-[--color-fg-muted]`,
};

const permChip = `${chipBase} bg-[--color-bg-elevated] text-[--color-fg-default]`;

function statusLabel(status: string): string {
  if (status === "pending") return m.invitationStatus_pending();
  if (status === "accepted") return m.invitationStatus_accepted();
  if (status === "revoked") return m.invitationStatus_revoked();
  return status;
}

export interface InvitationListProps {
  orgId: string;
  initialInvitations?: OrgInvitationView[] | undefined;
}

export function InvitationList({
  orgId,
  initialInvitations,
}: InvitationListProps): ReactNode {
  const [invitations, setInvitations] = useState<OrgInvitationView[]>(
    initialInvitations ?? [],
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [revokingToken, setRevokingToken] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();

  const load = useCallback(() => {
    startTransition(async () => {
      try {
        const result: ListOrgInvitationsResult =
          await listOrgInvitations(orgId);
        setInvitations(result.invitations);
        setErrorMessage(null);
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else {
          setErrorMessage(m.invitationList_error());
        }
      }
    });
  }, [orgId]);

  useEffect(() => {
    if (initialInvitations !== undefined) return;
    load();
  }, [initialInvitations, load]);

  function handleRevoke(token: string): void {
    setRevokingToken(token);
    startTransition(async () => {
      try {
        const result = await revokeOrgInvitation(orgId, token);
        setInvitations((prev) =>
          prev.map((inv) => (inv.token === token ? result.invitation : inv)),
        );
      } catch (cause: unknown) {
        if (cause instanceof AccountRequestError) {
          setErrorMessage(describeFailure(cause.failure));
        } else {
          setErrorMessage(m.invitationList_error());
        }
      } finally {
        setRevokingToken(null);
      }
    });
  }

  return (
    <section aria-label={m.invitationList_formLabel()}>
      <h2 className="m-0 mb-4 text-lg font-semibold">
        {m.invitationList_title()}
      </h2>
      {errorMessage !== null && (
        <p role="alert" className="m-0 mb-3 text-[--color-error]">
          {errorMessage}
        </p>
      )}
      {invitations.length === 0 && !pending ? (
        <p className="m-0 text-[--color-fg-muted]">
          {m.invitationList_empty()}
        </p>
      ) : (
        <ul className="m-0 list-none space-y-3 p-0">
          {invitations.map((inv) => (
            <li
              key={inv.token}
              data-invitation-token={inv.token}
              data-invitation-status={inv.status}
              className="flex flex-col gap-2 rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3"
            >
              <div className="flex items-center gap-2">
                <span className="text-sm text-[--color-fg-muted]">
                  {inv.recipient_identifier}
                </span>
                <span className={statusColors[inv.status] ?? chipBase}>
                  {statusLabel(inv.status)}
                </span>
              </div>
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
              {inv.status === "pending" && (
                <button
                  type="button"
                  disabled={revokingToken === inv.token}
                  onClick={() => {
                    handleRevoke(inv.token);
                  }}
                  className="self-start rounded-md border border-[--color-border] bg-[--color-bg-elevated] px-3 py-1 text-xs text-[--color-fg-default] transition-colors hover:bg-[--color-accent-hover] hover:text-[--color-accent-fg] disabled:opacity-60"
                >
                  {revokingToken === inv.token
                    ? m.invitationList_revoking()
                    : m.invitationList_revoke()}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
