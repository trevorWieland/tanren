"use client";

import type { ReactNode } from "react";

import { UserConfigPanel } from "@/components/config/UserConfigPanel";
import { CredentialPanel } from "@/components/config/CredentialPanel";

export default function UserConfigPage(): ReactNode {
  return (
    <main className="flex min-h-screen flex-col items-center gap-8 p-8">
      <UserConfigPanel />
      <CredentialPanel />
    </main>
  );
}
