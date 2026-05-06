import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { signOut } from "@/app/lib/account-client";
import {
  type ProjectView,
  type ScopedViewsResponse,
  ProjectRequestError,
  getActiveProjectViews,
  listProjects,
  switchProject,
} from "@/app/lib/project-client";

export const projectKeys = {
  all: ["projects"] as const,
  list: () => [...projectKeys.all, "list"] as const,
  activeViews: () => [...projectKeys.all, "active-views"] as const,
};

function isUnauthenticated(error: unknown): boolean {
  return (
    error instanceof ProjectRequestError && error.code === "unauthenticated"
  );
}

export function useProjectList() {
  const query = useQuery({
    queryKey: projectKeys.list(),
    queryFn: listProjects,
  });
  return {
    ...query,
    authError: query.error !== null && isUnauthenticated(query.error),
  };
}

async function fetchActiveViewsSafe(): Promise<ScopedViewsResponse | null> {
  try {
    return await getActiveProjectViews();
  } catch (error: unknown) {
    if (isUnauthenticated(error)) {
      throw error;
    }
    return null;
  }
}

export function useActiveProjectViews() {
  return useQuery({
    queryKey: projectKeys.activeViews(),
    queryFn: fetchActiveViewsSafe,
  });
}

export function useActiveProject(): ProjectView | null {
  const { data: projects } = useProjectList();
  const { data: scopedViews } = useActiveProjectViews();
  if (!projects || !scopedViews) return null;
  return projects.find((p) => p.id === scopedViews.project_id) ?? null;
}

export function useSwitchProjectMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: switchProject,
    onSuccess: async (data) => {
      queryClient.setQueryData(projectKeys.activeViews(), {
        project_id: data.scoped.project_id,
        specs: data.scoped.specs,
        loops: data.scoped.loops,
        milestones: data.scoped.milestones,
      } satisfies ScopedViewsResponse);
      await queryClient.invalidateQueries({ queryKey: projectKeys.list() });
    },
  });
}

export function useSignOutMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: signOut,
    onSettled: () => {
      queryClient.clear();
    },
  });
}
