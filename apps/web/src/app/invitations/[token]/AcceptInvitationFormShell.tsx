import { useNavigate } from "react-router";
import type { ReactNode } from "react";

import { AcceptInvitationForm } from "@/components/account/AcceptInvitationForm";

export interface AcceptInvitationFormShellProps {
  token: string;
}

export function AcceptInvitationFormShell({
  token,
}: AcceptInvitationFormShellProps): ReactNode {
  const navigate = useNavigate();
  return (
    <AcceptInvitationForm
      token={token}
      onSuccess={() => {
        navigate("/");
      }}
    />
  );
}
