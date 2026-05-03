"use client";

import { useRouter } from "next/navigation";
import type { ReactNode } from "react";

import { AcceptInvitationForm } from "@/components/account/AcceptInvitationForm";

export interface AcceptInvitationFormShellProps {
  token: string;
}

/**
 * Client wrapper around `AcceptInvitationForm` that redirects to `/`
 * after a successful acceptance, mirroring the sign-up / sign-in pages.
 * Lives next to the server-rendered `page.tsx` so the page itself can
 * stay a server component (preserving the same-origin POST property
 * documented there).
 */
export function AcceptInvitationFormShell({
  token,
}: AcceptInvitationFormShellProps): ReactNode {
  const router = useRouter();
  return (
    <AcceptInvitationForm
      token={token}
      onSuccess={() => {
        router.push("/");
      }}
    />
  );
}
