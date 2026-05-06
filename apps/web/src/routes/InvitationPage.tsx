import { useNavigate, useParams } from "react-router-dom";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

import { AcceptInvitationForm } from "@/components/account/AcceptInvitationForm";
import { JoinOrganizationForm } from "@/components/account/JoinOrganizationForm";

export function InvitationPage(): ReactNode {
  const { token } = useParams<{ token: string }>();
  const navigate = useNavigate();

  if (!token) {
    return null;
  }

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <section className="flex w-full max-w-md flex-col gap-4">
        <h1 className="text-2xl font-semibold">{m.acceptInvitation_title()}</h1>
        <p className="m-0 text-[--color-fg-muted]">
          {m.acceptInvitation_subtitle()}
        </p>
        <AcceptInvitationForm
          token={token}
          onSuccess={() => {
            navigate("/");
          }}
        />
      </section>
      <hr className="w-full max-w-md border-t border-[--color-border]" />
      <section className="flex w-full max-w-md flex-col gap-4">
        <h2 className="text-xl font-semibold">{m.joinOrg_title()}</h2>
        <p className="m-0 text-[--color-fg-muted]">{m.joinOrg_subtitle()}</p>
        <JoinOrganizationForm
          token={token}
          onSuccess={() => {
            navigate("/");
          }}
        />
      </section>
    </main>
  );
}
