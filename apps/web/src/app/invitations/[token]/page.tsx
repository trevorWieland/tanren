import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

import { AcceptInvitationFormShell } from "./AcceptInvitationFormShell";
import { JoinOrganizationFormShell } from "./JoinOrganizationFormShell";

interface InvitationPageProps {
  params: Promise<{ token: string }>;
}

/**
 * Server-rendered interstitial. Next.js renders this as a server component
 * (no `"use client"`) so the email link lands on a fresh same-origin
 * document. Submission of the embedded form is a same-origin POST that
 * fires the `SameSite=Strict` session cookie correctly.
 *
 * Shows two paths:
 * 1. **New account** — the existing accept-invitation form (B-0043).
 * 2. **Existing account** — sign in and join the organization (R-0006).
 */
export default async function InvitationAcceptPage(
  props: InvitationPageProps,
): Promise<ReactNode> {
  const { token } = await props.params;
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <section className="flex w-full max-w-md flex-col gap-4">
        <h1 className="text-2xl font-semibold">{m.acceptInvitation_title()}</h1>
        <p className="m-0 text-[--color-fg-muted]">
          {m.acceptInvitation_subtitle()}
        </p>
        <AcceptInvitationFormShell token={token} />
      </section>
      <hr className="w-full max-w-md border-t border-[--color-border]" />
      <section className="flex w-full max-w-md flex-col gap-4">
        <h2 className="text-xl font-semibold">{m.joinOrg_title()}</h2>
        <p className="m-0 text-[--color-fg-muted]">{m.joinOrg_subtitle()}</p>
        <JoinOrganizationFormShell token={token} />
      </section>
    </main>
  );
}
