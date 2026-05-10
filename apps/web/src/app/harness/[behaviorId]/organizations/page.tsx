import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { lookupBehaviorHarnessRoute } from "@/lib/generated/behavior-harness-routes";
import { OrganizationHarnessRoute } from "@/routes/organizations";

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
  return <OrganizationHarnessRoute />;
}
