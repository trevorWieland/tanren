import { useId, useState } from "react";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";
import type {
  OrganizationSwitcher as OrgSwitcherState,
  ProjectView,
} from "@/app/lib/account-client";
import { useOrganizationSwitcher } from "./useOrganizationSwitcher";

const PAGE_SIZE = 20;

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

  const {
    activeOrg,
    memberships,
    projects,
    projectsLoading,
    switching,
    switchOrg,
  } = useOrganizationSwitcher(data, onSwitched);

  const hasOrgs = memberships.length > 0;

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
          value={activeOrg ?? ""}
          onChange={(event) => {
            switchOrg(event.target.value);
          }}
          disabled={switching}
          className={selectClass}
        >
          {memberships.map((org) => (
            <option key={org.org_id} value={org.org_id}>
              {org.org_name}
              {activeOrg === org.org_id ? ` ${m.orgSwitcher_active()}` : ""}
            </option>
          ))}
        </select>
      </div>

      {switching && (
        <p className="text-sm text-[--color-fg-muted]">
          {m.orgSwitcher_switching()}
        </p>
      )}

      {activeOrg !== null && (
        <ProjectList
          key={activeOrg}
          projects={projects}
          loading={projectsLoading}
        />
      )}
    </section>
  );
}

interface ProjectListProps {
  projects: ProjectView[];
  loading: boolean;
}

function ProjectList({ projects, loading }: ProjectListProps): ReactNode {
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  if (loading) {
    return (
      <div>
        <h3 className="text-sm font-medium">{m.orgProjects_title()}</h3>
        <p className="text-sm text-[--color-fg-muted]">
          {m.orgProjects_loading()}
        </p>
      </div>
    );
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

  const visible = projects.slice(0, visibleCount);
  const hasMore = visibleCount < projects.length;

  return (
    <div>
      <h3 className="mb-2 text-sm font-medium">{m.orgProjects_title()}</h3>
      <ul className="m-0 list-none space-y-1 p-0">
        {visible.map((project) => (
          <li
            key={project.id}
            className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-sm"
          >
            {project.name}
          </li>
        ))}
      </ul>
      {hasMore && (
        <button
          type="button"
          onClick={() => setVisibleCount((prev) => prev + PAGE_SIZE)}
          className="mt-2 text-sm text-[--color-accent] hover:underline"
        >
          {m.orgProjects_showMore()}
        </button>
      )}
    </div>
  );
}
