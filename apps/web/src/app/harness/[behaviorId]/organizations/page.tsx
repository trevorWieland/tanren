import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { resolveBehaviorHarnessComponent } from "@/lib/behavior-harness-dispatch";
import { lookupBehaviorHarnessRoute } from "@/lib/generated/behavior-harness-routes";

interface OrganizationHarnessPageProps {
  params: Promise<{
    behaviorId: string;
  }>;
}

export default async function OrganizationHarnessPage(
  props: OrganizationHarnessPageProps,
): Promise<ReactNode> {
  const params = await props.params;
  if (!lookupBehaviorHarnessRoute("organizations", params.behaviorId)) {
    notFound();
  }
  const HarnessComponent = resolveBehaviorHarnessComponent("organizations");
  if (!HarnessComponent) {
    notFound();
  }
  return <HarnessComponent />;
}
