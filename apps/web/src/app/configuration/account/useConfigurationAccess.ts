"use client";

import { useEffect, useMemo, useState } from "react";

import { discoverConfigurationAccess } from "@/app/lib/account-client";
import type {
  ConfigurationCapabilities,
  ListUserCredentialsResult,
  ListUserSettingsResult,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

import {
  formatFailure,
  formatRequestError,
} from "@/app/configuration/account/configurationRequestErrors";

export type SettingCapabilityAction =
  ConfigurationCapabilities["settings"]["allowed_actions"][number];
export type CredentialCapabilityAction =
  ConfigurationCapabilities["user_items"]["allowed_actions"][number];

const EMPTY_SETTING_ACTIONS: readonly SettingCapabilityAction[] = [];
const EMPTY_CREDENTIAL_ACTIONS: readonly CredentialCapabilityAction[] = [];

export interface ConfigurationAccessModel {
  capabilities: ConfigurationCapabilities | null;
  capabilitiesAccessError: string | null;
  capabilitiesLoading: boolean;
  canCreateCredentials: boolean;
  canDeleteCredentials: boolean;
  canReadCredentials: boolean;
  canUpdateCredentials: boolean;
  credentialActions: readonly CredentialCapabilityAction[];
  discoveredCredentialsReadModel: ListUserCredentialsResult | null;
  discoveredSettingsReadModel: ListUserSettingsResult | null;
  settingsAccessError: string | null;
  settingActions: readonly SettingCapabilityAction[];
  userCredentialsAccessError: string | null;
}

export function useConfigurationAccess(): ConfigurationAccessModel {
  const [capabilities, setCapabilities] =
    useState<ConfigurationCapabilities | null>(null);
  const [discoveredSettingsReadModel, setDiscoveredSettingsReadModel] =
    useState<ListUserSettingsResult | null>(null);
  const [discoveredCredentialsReadModel, setDiscoveredCredentialsReadModel] =
    useState<ListUserCredentialsResult | null>(null);
  const [capabilitiesAccessError, setCapabilitiesAccessError] = useState<
    string | null
  >(null);
  const [settingsAccessError, setSettingsAccessError] = useState<string | null>(
    null,
  );
  const [userCredentialsAccessError, setUserCredentialsAccessError] = useState<
    string | null
  >(null);

  useEffect(() => {
    let cancelled = false;

    void discoverConfigurationAccess()
      .then((result) => {
        if (cancelled) {
          return;
        }
        setCapabilities(result.capabilities);
        setCapabilitiesAccessError(
          result.capabilities_failure === null
            ? null
            : `${m.config_access_check_failed()}: ${formatFailure(result.capabilities_failure)}`,
        );
        setDiscoveredSettingsReadModel(result.settings_read_model);
        setDiscoveredCredentialsReadModel(result.credentials_read_model);
        setSettingsAccessError(
          result.settings_failure === null
            ? null
            : `${m.config_settings_access_limited()}: ${formatFailure(result.settings_failure)}`,
        );
        setUserCredentialsAccessError(
          result.credentials_failure === null
            ? null
            : `${m.config_credentials_access_limited()}: ${formatFailure(result.credentials_failure)}`,
        );
      })
      .catch((reason: unknown) => {
        if (cancelled) {
          return;
        }
        const details = formatRequestError(reason);
        setCapabilitiesAccessError(details);
        setSettingsAccessError(`${m.config_access_check_failed()}: ${details}`);
        setUserCredentialsAccessError(
          `${m.config_access_check_failed()}: ${details}`,
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const settingActions = useMemo<readonly SettingCapabilityAction[]>(
    () => capabilities?.settings.allowed_actions ?? EMPTY_SETTING_ACTIONS,
    [capabilities],
  );
  const credentialActions = useMemo<readonly CredentialCapabilityAction[]>(
    () => capabilities?.user_items.allowed_actions ?? EMPTY_CREDENTIAL_ACTIONS,
    [capabilities],
  );
  const credentialActionSet = useMemo(
    () => new Set<CredentialCapabilityAction>(credentialActions),
    [credentialActions],
  );

  return {
    capabilities,
    capabilitiesAccessError,
    capabilitiesLoading: capabilities === null,
    canCreateCredentials: credentialActionSet.has("create"),
    canDeleteCredentials: credentialActionSet.has("delete"),
    canReadCredentials: credentialActionSet.has("read"),
    canUpdateCredentials: credentialActionSet.has("update"),
    credentialActions,
    discoveredCredentialsReadModel,
    discoveredSettingsReadModel,
    settingsAccessError,
    settingActions,
    userCredentialsAccessError,
  };
}
