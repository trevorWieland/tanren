"use client";

import type { ReactNode } from "react";
import { useParams } from "next/navigation";

import { InvitationCreateForm } from "@/components/account/InvitationCreateForm";
import { InvitationList } from "@/components/account/InvitationList";
import * as m from "@/i18n/paraglide/messages";

export default function OrgInvitationsPage(): ReactNode {
  const params = useParams<{ orgId: string }>();
  const orgId = params.orgId;

  return (
    <main className="flex min-h-screen flex-col items-center gap-8 p-8">
      <h1 className="text-2xl font-semibold">{m.invitationList_title()}</h1>
      <InvitationCreateForm orgId={orgId} />
      <InvitationList orgId={orgId} />
    </main>
  );
}
