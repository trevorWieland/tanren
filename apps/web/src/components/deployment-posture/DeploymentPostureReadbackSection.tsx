import type {
  DeploymentPostureGetResponse,
  SupportedDeploymentPosture,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

import type { ReactNode } from "react";

import {
  formatAvailableCapabilities,
  formatUnavailableCapabilities,
  operationalScopeLabel,
} from "./posture-view-model";

interface DeploymentPostureReadbackSectionProps {
  supported: SupportedDeploymentPosture[];
  current: DeploymentPostureGetResponse["current"];
}

export function DeploymentPostureReadbackSection({
  supported,
  current,
}: DeploymentPostureReadbackSectionProps): ReactNode {
  return (
    <section className="space-y-3 rounded border border-[--color-border] p-4">
      <h3 className="text-base font-semibold">Capability readback</h3>
      <div className="space-y-2 text-sm">
        <p className="font-semibold">{m.posture_supported()}</p>
        {supported.length === 0 ? (
          <p className="text-[--color-fg-muted]">
            No supported postures discovered.
          </p>
        ) : (
          supported.map((item) => (
            <div
              key={item.posture}
              className="rounded border border-[--color-border] p-3"
            >
              <p className="font-mono">{item.posture}</p>
              <p>
                {m.posture_available()}:{" "}
                {formatAvailableCapabilities(item.capability_summary.available)}
              </p>
              <p>
                {m.posture_unavailable()}:{" "}
                {formatUnavailableCapabilities(
                  item.capability_summary.unavailable,
                )}
              </p>
            </div>
          ))
        )}
        <p className="pt-2 font-semibold">{m.posture_current()}</p>
        {current === null ? (
          <p className="text-[--color-fg-muted]">{m.posture_notSet()}</p>
        ) : (
          <div className="rounded border border-[--color-border] p-3">
            <p className="font-mono">{current.posture}</p>
            <p className="font-mono text-xs text-[--color-fg-muted]">
              scope: {operationalScopeLabel(current.scope)}
            </p>
            <p>
              {m.posture_available()}:{" "}
              {formatAvailableCapabilities(
                current.capability_summary.available,
              )}
            </p>
            <p>
              {m.posture_unavailable()}:{" "}
              {formatUnavailableCapabilities(
                current.capability_summary.unavailable,
              )}
            </p>
          </div>
        )}
      </div>
    </section>
  );
}
