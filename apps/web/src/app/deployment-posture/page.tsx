import type { ReactNode } from "react";

import { DeploymentPosturePanel } from "@/components/deployment-posture/DeploymentPosturePanel";

export default function DeploymentPosturePage(): ReactNode {
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-4xl flex-col gap-6 p-8">
      <DeploymentPosturePanel />
    </main>
  );
}
