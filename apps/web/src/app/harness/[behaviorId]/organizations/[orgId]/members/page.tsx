import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { resolveBehaviorHarnessComponent } from "@/lib/behavior-harness-dispatch";
import { lookupBehaviorHarnessRoute } from "@/lib/generated/behavior-harness-routes";

interface OrganizationMembersHarnessPageProps {
  params: Promise<{
    behaviorId: string;
    orgId: string;
  }>;
}

export default async function OrganizationMembersHarnessPage(
  props: OrganizationMembersHarnessPageProps,
): Promise<ReactNode> {
  const params = await props.params;
  if (!lookupBehaviorHarnessRoute("organizationMembers", params.behaviorId)) {
    notFound();
  }
  const HarnessComponent = resolveBehaviorHarnessComponent(
    "organizationMembers",
  );
  if (!HarnessComponent) {
    notFound();
  }
  return <HarnessComponent />;
}
