import { useParams } from "react-router";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

import { AcceptInvitationFormShell } from "./AcceptInvitationFormShell";

export default function InvitationAcceptPage(): ReactNode {
  const { token } = useParams();
  if (!token) {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
        <p className="text-[--color-error]">Missing invitation token.</p>
      </main>
    );
  }
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
