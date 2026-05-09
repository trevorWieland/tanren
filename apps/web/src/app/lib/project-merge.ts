import type { ProjectView } from "@/app/lib/contracts";

function clearActiveSelection(project: ProjectView): ProjectView {
  return {
    ...project,
    selection: {
      ...project.selection,
      is_active: false,
      selected_at: null,
    },
  };
}

export function normalizeActiveProjects(
  projects: ProjectView[],
): ProjectView[] {
  let seenActive = false;
  return projects.map((project) => {
    if (!project.selection.is_active) {
      return project;
    }
    if (!seenActive) {
      seenActive = true;
      return project;
    }
    return clearActiveSelection(project);
  });
}

export function mergeProjectIntoVisibleProjects(
  projects: ProjectView[],
  next: ProjectView,
): ProjectView[] {
  const withoutCurrent = projects.filter((project) => project.id !== next.id);
  if (!next.selection.is_active) {
    return normalizeActiveProjects([next, ...withoutCurrent]);
  }
  return [next, ...withoutCurrent.map(clearActiveSelection)];
}
