import { useEffect, useState } from "react";

import type { RoleAdminCapabilities } from "@/app/lib/generated/role-contract";
import {
  fetchRoleCapabilities,
  formatRoleError,
  type RoleCapabilitySnapshot,
} from "@/app/lib/role-client";

interface UseRoleCapabilitiesResult {
  snapshot: RoleCapabilitySnapshot | null;
  capabilities: RoleAdminCapabilities | null;
  csrfToken: string | null;
  errorMessage: string | null;
}

export function useRoleCapabilities(): UseRoleCapabilitiesResult {
  const [snapshot, setSnapshot] = useState<RoleCapabilitySnapshot | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchRoleCapabilities()
      .then((next) => {
        if (!cancelled) {
          setSnapshot(next);
          setErrorMessage(null);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setSnapshot(null);
          setErrorMessage(formatRoleError(reason));
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return {
    snapshot,
    capabilities: snapshot?.capabilities ?? null,
    csrfToken: snapshot?.csrfToken ?? null,
    errorMessage,
  };
}
