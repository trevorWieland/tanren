import type { ReactNode } from "react";

import type { BehaviorHarnessRouteLeaf } from "@/lib/generated/behavior-harness-routes";
import { BEHAVIOR_HARNESS_ROUTES } from "@/lib/generated/behavior-harness-routes";
import { OrganizationHarnessRoute } from "@/routes/organizations";

/**
 * Mapping from generated harness route leaf keys to their web harness
 * components. The dispatch adapter resolves the behavior/interface pairing
 * from the generated harness route projection before rendering — harness
 * pages must not import route components directly.
 */
const BEHAVIOR_HARNESS_COMPONENTS: Record<
  BehaviorHarnessRouteLeaf,
  () => ReactNode
> = {
  organizations: OrganizationHarnessRoute,
};

/**
 * Resolve the web harness component for a given harness route leaf.
 * Returns `null` when no matching component is registered — callers
 * should surface a `notFound()` response in that case.
 *
 * This is the single authorized entrypoint through which harness pages
 * reach the real web harness surface. Bypassing this adapter (e.g. by
 * importing a harness component directly in a harness page file) is
 * rejected by `xtask check-web-harness-routes`.
 */
export function resolveBehaviorHarnessComponent(
  leaf: string,
): (() => ReactNode) | null {
  const route = BEHAVIOR_HARNESS_ROUTES[leaf as BehaviorHarnessRouteLeaf];
  if (!route) {
    return null;
  }
  const component =
    BEHAVIOR_HARNESS_COMPONENTS[leaf as BehaviorHarnessRouteLeaf];
  if (!component) {
    return null;
  }
  return component;
}
