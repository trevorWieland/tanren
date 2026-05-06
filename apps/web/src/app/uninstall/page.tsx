"use client";

import { useRouter, useSearchParams } from "next/navigation";
import type { ReactNode } from "react";

import { UninstallPanel } from "@/components/project/UninstallPanel";
import * as m from "@/i18n/paraglide/messages";

export default function UninstallPage(): ReactNode {
  const router = useRouter();
  const searchParams = useSearchParams();
  const repoPath = searchParams.get("repo") ?? undefined;

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.uninstall_title()}</h1>
      <p className="max-w-md text-center text-[--color-fg-muted]">
        {m.uninstall_subtitle()}
      </p>
      <UninstallPanel
        repoPath={repoPath}
        onApplied={() => {
          router.push("/");
        }}
      />
    </main>
  );
}
