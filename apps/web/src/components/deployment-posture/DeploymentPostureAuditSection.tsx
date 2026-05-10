import type { ReactNode } from "react";

interface DeploymentPostureAuditSectionProps {
  postureNotice: string | null;
  auditReference: string | null;
}

export function DeploymentPostureAuditSection({
  postureNotice,
  auditReference,
}: DeploymentPostureAuditSectionProps): ReactNode {
  if (postureNotice === null && auditReference === null) {
    return null;
  }

  return (
    <section className="space-y-2 rounded border border-[--color-border] p-4 text-sm">
      <h3 className="text-base font-semibold">Audit</h3>
      {postureNotice !== null ? (
        <p className="text-[--color-fg-muted]">{postureNotice}</p>
      ) : null}
      {auditReference !== null ? (
        <p className="font-mono text-[--color-fg-muted]">
          audit reference: {auditReference}
        </p>
      ) : null}
    </section>
  );
}
