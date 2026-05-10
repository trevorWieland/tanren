import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { ORGANIZATION_WEB_HARNESS_BEHAVIOR_SEGMENT } from "@/lib/organization-routes";
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
  if (params.behaviorId !== ORGANIZATION_WEB_HARNESS_BEHAVIOR_SEGMENT) {
    notFound();
  }
  return <OrganizationHarnessRoute />;
}
