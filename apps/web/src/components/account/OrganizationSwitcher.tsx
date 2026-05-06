"use client";

import { useId, useState, useTransition } from "react";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";
import {
  type OrganizationSwitcher as OrgSwitcherState,
  type ProjectView,
  listActiveOrgProjects,
  switchActiveOrganization,
} from "@/app/lib/account-client";

export interface OrganizationSwitcherProps {
  data: OrgSwitcherState;
  onSwitched?: (activeOrg: string | null) => void;
}

const selectClass =
  "w-full rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent] sm:w-auto";

export function OrganizationSwitcher({
  data,
  onSwitched,
}: OrganizationSwitcherProps): ReactNode {
  const baseId = useId();
  const selectId = `${baseId}-org`;

  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [projectsLoaded, setProjectsLoaded] = useState(false);
  const [pending, startTransition] = useTransition();

  function loadProjects(): void {
    startTransition(async () => {
      try {
        const result = await listActiveOrgProjects();
        setProjects(result.projects);
        setProjectsLoaded(true);
      } catch {
        setProjects([]);
        setProjectsLoaded(true);
      }
    });
  }

  function onChange(orgId: string): void {
    if (orgId === "") {
      return;
    }
    startTransition(async () => {
      try {
        const result = await switchActiveOrganization({ org_id: orgId });
        onSwitched?.(result.account.org ?? null);
        setProjectsLoaded(false);
        loadProjects();
      } catch {
        // error handled silently; switcher retains current state
      }
    });
  }

  const hasOrgs = data.memberships.length > 0;

  if (!hasOrgs) {
    return (
      <section aria-label={m.orgSwitcher_label()} className="w-full max-w-md">
        <p className="text-sm text-[--color-fg-muted]">
          {m.orgSwitcher_noOrgs()}
        </p>
      </section>
    );
  }

  return (
    <section
      aria-label={m.orgSwitcher_label()}
      className="flex w-full max-w-md flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <label htmlFor={selectId} className="text-sm font-medium">
          {m.orgSwitcher_label()}
        </label>
        <select
          id={selectId}
          value={data.active_org ?? ""}
          onChange={(event) => {
            onChange(event.target.value);
          }}
          disabled={pending}
          className={selectClass}
        >
          {data.memberships.map((org) => (
            <option key={org.org_id} value={org.org_id}>
              {org.org_name}
              {data.active_org === org.org_id
                ? ` ${m.orgSwitcher_active()}`
                : ""}
            </option>
          ))}
        </select>
      </div>

      {pending && (
        <p className="text-sm text-[--color-fg-muted]">
          {m.orgSwitcher_switching()}
        </p>
      )}

      {data.active_org !== null && (
        <ProjectList
          projects={projects}
          loaded={projectsLoaded}
          onLoad={loadProjects}
        />
      )}
    </section>
  );
}

interface ProjectListProps {
  projects: ProjectView[];
  loaded: boolean;
  onLoad: () => void;
}

function ProjectList({
  projects,
  loaded,
  onLoad,
}: ProjectListProps): ReactNode {
  if (!loaded) {
    onLoad();
  }

  if (!loaded) {
    return null;
  }

  if (projects.length === 0) {
    return (
      <div>
        <h3 className="text-sm font-medium">{m.orgProjects_title()}</h3>
        <p className="text-sm text-[--color-fg-muted]">
          {m.orgProjects_none()}
        </p>
      </div>
    );
  }

  return (
    <div>
      <h3 className="mb-2 text-sm font-medium">{m.orgProjects_title()}</h3>
      <ul className="m-0 list-none space-y-1 p-0">
        {projects.map((project) => (
          <li
            key={project.id}
            className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-sm"
          >
            {project.name}
          </li>
        ))}
      </ul>
    </div>
  );
}
