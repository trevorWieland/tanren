import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

import { AcceptInvitationFormShell } from "./AcceptInvitationFormShell";

interface InvitationPageProps {
  params: Promise<{ token: string }>;
}

/**
 * Server-rendered interstitial. Next.js renders this as a server component
 * (no `"use client"`) so the email link lands on a fresh same-origin
 * document. Submission of the embedded form is a same-origin POST that
 * fires the `SameSite=Strict` session cookie correctly. The user supplies
 * their own email here, fixing the C1 fabricated-identifier finding.
 */
export default async function InvitationAcceptPage(
  props: InvitationPageProps,
): Promise<ReactNode> {
  const { token } = await props.params;
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.acceptInvitation_title()}</h1>
      <p className="m-0 text-[--color-fg-muted]">
        {m.acceptInvitation_subtitle()}
      </p>
      <AcceptInvitationFormShell token={token} />
    </main>
  );
}
