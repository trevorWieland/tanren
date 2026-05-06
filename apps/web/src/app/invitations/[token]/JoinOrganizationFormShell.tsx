"use client";

import { useRouter } from "next/navigation";
import type { ReactNode } from "react";

import { JoinOrganizationForm } from "@/components/account/JoinOrganizationForm";

export interface JoinOrganizationFormShellProps {
  token: string;
}

export function JoinOrganizationFormShell({
  token,
}: JoinOrganizationFormShellProps): ReactNode {
  const router = useRouter();
  return (
    <JoinOrganizationForm
      token={token}
      onSuccess={() => {
        router.push("/");
      }}
    />
  );
}
