"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import {
  AccountRequestError,
  describeFailure,
  getActiveAccountScopeId,
  getDeploymentPosture,
  listDeploymentPosturesCached,
  setDeploymentPosture,
  type DeploymentPostureGetResponse,
  type SupportedDeploymentPosture,
} from "@/app/lib/account-client";
import { isDeploymentPosture } from "@/app/lib/generated/deployment-posture-contract";
import * as m from "@/i18n/paraglide/messages";

import { DeploymentPostureAuditSection } from "./DeploymentPostureAuditSection";
import { DeploymentPostureDiscoverySection } from "./DeploymentPostureDiscoverySection";
import { DeploymentPostureMutationSection } from "./DeploymentPostureMutationSection";
import { DeploymentPostureReadbackSection } from "./DeploymentPostureReadbackSection";
import {
  buildOperationalScopeOptions,
  operationalScopeId,
  type OperationalScopeOption,
} from "./posture-view-model";

function describeRequestError(reason: unknown): string {
  if (reason instanceof AccountRequestError) {
    return reason.message;
  }
  if (reason instanceof Error) {
    return reason.message;
  }
  return String(reason);
}

export function DeploymentPosturePanel(): ReactNode {
  const [supported, setSupported] = useState<SupportedDeploymentPosture[]>([]);
  const [scopeOptions, setScopeOptions] = useState<OperationalScopeOption[]>(
    [],
  );
  const [selectedScopeKey, setSelectedScopeKey] = useState("");
  const [selectedPosture, setSelectedPosture] = useState<string>("");
  const [current, setCurrent] =
    useState<DeploymentPostureGetResponse["current"]>(null);
  const [postureError, setPostureError] = useState<string | null>(null);
  const [postureNotice, setPostureNotice] = useState<string | null>(null);
  const [auditReference, setAuditReference] = useState<string | null>(null);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryReady, setDiscoveryReady] = useState(false);
  const [loadingCurrent, setLoadingCurrent] = useState(false);
  const [savingPosture, setSavingPosture] = useState(false);

  const selectedScope = useMemo(
    () =>
      scopeOptions.find(
        (scopeOption) => scopeOption.key === selectedScopeKey,
      ) ?? null,
    [scopeOptions, selectedScopeKey],
  );

  const discoverCapabilities = useCallback(async (): Promise<void> => {
    setPostureError(null);
    setPostureNotice(null);
    setAuditReference(null);
    setDiscovering(true);
    setDiscoveryReady(false);
    try {
      const activeAccountId = getActiveAccountScopeId();
      const options = buildOperationalScopeOptions(activeAccountId);
      if (options.length === 0) {
        setSupported([]);
        setScopeOptions([]);
        setSelectedScopeKey("");
        setCurrent(null);
        setPostureError(
          "No authenticated account scope is available. Sign in, then discover capabilities.",
        );
        return;
      }

      const response = await listDeploymentPosturesCached();
      setSupported(response.supported);
      setScopeOptions(options);
      setSelectedScopeKey((currentValue) => {
        const stillExists = options.some(
          (scopeOption) => scopeOption.key === currentValue,
        );
        if (stillExists) {
          return currentValue;
        }
        return options[0]?.key ?? "";
      });
      setSelectedPosture((currentValue) => {
        const currentSupported = response.supported.some(
          (item) => item.posture === currentValue,
        );
        if (currentSupported) {
          return currentValue;
        }
        return response.supported[0]?.posture ?? "";
      });
      setDiscoveryReady(true);
    } catch (reason: unknown) {
      setPostureError(describeRequestError(reason));
    } finally {
      setDiscovering(false);
    }
  }, []);

  useEffect(() => {
    void discoverCapabilities();
  }, [discoverCapabilities]);

  const loadCurrent = async (): Promise<void> => {
    if (!selectedScope) {
      setPostureError("Select an operational scope before reading posture.");
      return;
    }
    setPostureError(null);
    setPostureNotice(null);
    setAuditReference(null);
    setLoadingCurrent(true);
    try {
      const response = await getDeploymentPosture(
        selectedScope.scope.scope,
        operationalScopeId(selectedScope.scope),
      );
      setCurrent(response.current);
      if (response.current === null) {
        setPostureNotice(m.posture_notSet());
      }
    } catch (reason: unknown) {
      setPostureError(describeRequestError(reason));
    } finally {
      setLoadingCurrent(false);
    }
  };

  const savePosture = async (): Promise<void> => {
    if (!selectedScope) {
      setPostureError("Select an operational scope before setting posture.");
      return;
    }
    setPostureError(null);
    setPostureNotice(null);
    setAuditReference(null);

    if (
      !isDeploymentPosture(selectedPosture) ||
      !supported.some((item) => item.posture === selectedPosture)
    ) {
      setPostureError(
        describeFailure({
          code: "validation_failed",
          summary: `Unsupported deployment posture \`${selectedPosture}\`.`,
        }),
      );
      return;
    }

    setSavingPosture(true);
    try {
      const response = await setDeploymentPosture({
        scope: selectedScope.scope,
        posture: selectedPosture,
      });
      setCurrent({
        posture: response.posture,
        scope: response.scope,
        capability_summary: response.capability_summary,
      });
      setAuditReference(response.audit_reference);
      setPostureNotice(m.posture_saved());
    } catch (reason: unknown) {
      setPostureError(describeRequestError(reason));
    } finally {
      setSavingPosture(false);
    }
  };

  const canMutate =
    discoveryReady && selectedScope !== null && supported.length > 0;
  const canReadCurrent = discoveryReady && selectedScope !== null;

  return (
    <section className="space-y-4 rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
      <h2 className="text-xl font-semibold">{m.posture_title()}</h2>
      <p className="text-sm text-[--color-fg-muted]">{m.posture_subtitle()}</p>
      <DeploymentPostureDiscoverySection
        discoveryReady={discoveryReady}
        discoveryLoading={discovering}
        onDiscover={() => {
          void discoverCapabilities();
        }}
        scopeOptions={scopeOptions}
        selectedScopeKey={selectedScopeKey}
        onSelectScope={setSelectedScopeKey}
      />
      <DeploymentPostureMutationSection
        canMutate={canMutate}
        canReadCurrent={canReadCurrent}
        savingPosture={savingPosture}
        loadingCurrent={loadingCurrent}
        selectedPosture={selectedPosture}
        supported={supported}
        onSelectPosture={setSelectedPosture}
        onLoadCurrent={() => {
          void loadCurrent();
        }}
        onSavePosture={() => {
          void savePosture();
        }}
      />
      <DeploymentPostureReadbackSection
        supported={supported}
        current={current}
      />
      <DeploymentPostureAuditSection
        postureNotice={postureNotice}
        auditReference={auditReference}
      />
      {postureError !== null ? (
        <p className="text-sm text-[--color-error]">{postureError}</p>
      ) : null}
    </section>
  );
}
