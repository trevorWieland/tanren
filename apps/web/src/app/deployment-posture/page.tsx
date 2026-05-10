import type { ReactNode } from "react";

import { DeploymentPosturePanel } from "@/components/deployment-posture/DeploymentPosturePanel";

export default function DeploymentPosturePage(): ReactNode {
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-4xl flex-col gap-6 p-8">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold">
          Deployment posture operations
        </h1>
        <p className="text-sm text-[--color-fg-muted]">
          Discover capabilities first, then choose posture within an operational
          scope.
        </p>
      </header>
      <DeploymentPosturePanel />
    </main>
  );
}
