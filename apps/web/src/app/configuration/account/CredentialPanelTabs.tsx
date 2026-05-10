import type { ReactNode } from "react";

export type CredentialPanelName = "list" | "add" | "update" | "remove";

export interface CredentialPanelTabDescriptor {
  enabled: boolean;
  label: string;
  name: CredentialPanelName;
}

interface CredentialPanelTabsProps {
  activePanel: CredentialPanelName;
  onSelect: (panel: CredentialPanelName) => void;
  tabs: readonly CredentialPanelTabDescriptor[];
}

export function CredentialPanelTabs({
  activePanel,
  onSelect,
  tabs,
}: CredentialPanelTabsProps): ReactNode {
  return (
    <div className="mb-3 flex flex-wrap gap-2">
      {tabs.map((tab) => {
        if (!tab.enabled) {
          return null;
        }
        return (
          <button
            key={tab.name}
            type="button"
            onClick={() => onSelect(tab.name)}
            className={`rounded-md px-3 py-1.5 text-sm ${activePanel === tab.name ? "bg-[--color-accent] text-[--color-accent-fg]" : "border border-[--color-border]"}`}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
