import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { OrganizationSwitcher as OrgSwitcherState } from "@/app/lib/account-client";
import {
  listActiveOrgProjects,
  switchActiveOrganization,
} from "@/app/lib/account-client";

export function useOrganizationSwitcher(
  data: OrgSwitcherState,
  onSwitched?: (activeOrg: string | null) => void,
) {
  const queryClient = useQueryClient();

  const activeOrg: string | null = data.active_org ?? null;

  const projectsQuery = useQuery({
    queryKey: ["active-org-projects", activeOrg],
    queryFn: () => listActiveOrgProjects(),
    enabled: activeOrg !== null,
  });

  const switchMutation = useMutation({
    mutationFn: (orgId: string) => switchActiveOrganization({ org_id: orgId }),
    onSuccess: (result) => {
      void queryClient.invalidateQueries({ queryKey: ["organizations"] });
      void queryClient.invalidateQueries({ queryKey: ["active-org-projects"] });
      onSwitched?.(result.account.org ?? null);
    },
  });

  return {
    activeOrg,
    memberships: data.memberships,
    projects: projectsQuery.data?.projects ?? [],
    projectsLoading: projectsQuery.isLoading,
    switching: switchMutation.isPending,
    switchOrg(orgId: string): void {
      if (orgId === "") return;
      switchMutation.mutate(orgId);
    },
  };
}
