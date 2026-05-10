"use client";

import { useRouter } from "next/navigation";
import type { JSX } from "react";

import { buildAccountHomeRoute } from "@/app/lib/navigation";
import { AcceptInvitationForm } from "@/components/account/AcceptInvitationForm";

export interface AcceptInvitationFormShellProps {
  token: string;
}

/**
 * Client wrapper around `AcceptInvitationForm` that redirects to
 * the web-owned account home surface after a successful acceptance.
 * The account home can hand off to configuration, but configuration is
 * not the default identity flow destination.
 * Lives next to the server-rendered `page.tsx` so the page itself can
 * stay a server component (preserving the same-origin POST property
 * documented there).
 */
export function AcceptInvitationFormShell({
  token,
}: AcceptInvitationFormShellProps): JSX.Element {
  const router = useRouter();
  return (
    <AcceptInvitationForm
      token={token}
      onSuccess={() => {
        router.push(buildAccountHomeRoute({ from: "invitation" }));
      }}
    />
  );
}
