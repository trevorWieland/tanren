import type {
  DeploymentPosture,
  SupportedDeploymentPosture,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

import type { ReactNode } from "react";

interface DeploymentPostureMutationSectionProps {
  canMutate: boolean;
  canReadCurrent: boolean;
  savingPosture: boolean;
  loadingCurrent: boolean;
  selectedPosture: string;
  supported: SupportedDeploymentPosture[];
  onSelectPosture: (value: string) => void;
  onLoadCurrent: () => void;
  onSavePosture: () => void;
}

export function DeploymentPostureMutationSection({
  canMutate,
  canReadCurrent,
  savingPosture,
  loadingCurrent,
  selectedPosture,
  supported,
  onSelectPosture,
  onLoadCurrent,
  onSavePosture,
}: DeploymentPostureMutationSectionProps): ReactNode {
  const postureOptions: DeploymentPosture[] = supported.map(
    (item) => item.posture,
  );

  return (
    <section className="space-y-3 rounded border border-[--color-border] p-4">
      <h3 className="text-base font-semibold">Set posture</h3>
      <div className="grid gap-3 sm:grid-cols-[2fr_auto_auto]">
        <select
          value={selectedPosture}
          disabled={!canMutate || postureOptions.length === 0}
          onChange={(event) => {
            onSelectPosture(event.target.value);
          }}
          className="w-full rounded border border-[--color-border] bg-transparent px-3 py-2 text-sm disabled:opacity-60"
        >
          {postureOptions.map((posture) => (
            <option key={posture} value={posture}>
              {posture}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={!canReadCurrent || loadingCurrent}
          onClick={onLoadCurrent}
          className="rounded border border-[--color-border] px-3 py-2 text-sm disabled:opacity-60"
        >
          {loadingCurrent ? m.posture_loading() : m.posture_load()}
        </button>
        <button
          type="button"
          disabled={!canMutate || savingPosture}
          onClick={onSavePosture}
          className="rounded border border-[--color-border] px-3 py-2 text-sm disabled:opacity-60"
        >
          {savingPosture ? m.posture_saving() : m.posture_save()}
        </button>
      </div>
    </section>
  );
}
