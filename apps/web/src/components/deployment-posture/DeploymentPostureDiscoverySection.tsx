import type { ReactNode } from "react";

import type { OperationalScopeOption } from "./posture-view-model";

interface DeploymentPostureDiscoverySectionProps {
  discoveryReady: boolean;
  discoveryLoading: boolean;
  onDiscover: () => void;
  scopeOptions: OperationalScopeOption[];
  selectedScopeKey: string;
  onSelectScope: (scopeKey: string) => void;
}

export function DeploymentPostureDiscoverySection({
  discoveryReady,
  discoveryLoading,
  onDiscover,
  scopeOptions,
  selectedScopeKey,
  onSelectScope,
}: DeploymentPostureDiscoverySectionProps): ReactNode {
  return (
    <section className="space-y-3 rounded border border-[--color-border] p-4">
      <h3 className="text-base font-semibold">Scope discovery</h3>
      <p className="text-sm text-[--color-fg-muted]">
        Discover supported posture capabilities for the authenticated actor and
        select an operational scope.
      </p>
      <button
        type="button"
        onClick={onDiscover}
        className="rounded border border-[--color-border] px-3 py-2 text-sm"
      >
        {discoveryLoading
          ? "Discovering capabilities…"
          : "Discover capabilities"}
      </button>
      <div className="space-y-1">
        <label
          className="text-sm font-medium"
          htmlFor="posture-operational-scope"
        >
          Operational scope
        </label>
        <select
          id="posture-operational-scope"
          disabled={!discoveryReady || scopeOptions.length === 0}
          value={selectedScopeKey}
          onChange={(event) => {
            onSelectScope(event.target.value);
          }}
          className="w-full rounded border border-[--color-border] bg-transparent px-3 py-2 text-sm disabled:opacity-60"
        >
          {scopeOptions.map((scopeOption) => (
            <option key={scopeOption.key} value={scopeOption.key}>
              {scopeOption.label}
            </option>
          ))}
        </select>
      </div>
    </section>
  );
}
