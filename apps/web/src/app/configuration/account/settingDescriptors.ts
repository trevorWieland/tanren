import type {
  ThemePreference,
  UpsertUserSettingInput,
  UserCredentialKind,
  UserSettingKey,
} from "@/app/lib/api-contracts";
import {
  THEME_PREFERENCE_VALUES,
  USER_CREDENTIAL_KIND_VALUES,
  USER_SETTING_KEY_VALUES,
} from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

export interface UserSettingDescriptor {
  key: UserSettingKey;
  label: () => string;
  placeholder: () => string;
  toValue: (raw: string) => UpsertUserSettingInput["value"] | null;
}

const SETTING_DESCRIPTOR_BY_KEY = {
  theme: {
    key: "theme",
    label: m.config_setting_key_theme,
    placeholder: m.config_settings_placeholder_theme_values,
    toValue: (raw: string): UpsertUserSettingInput["value"] | null =>
      isThemePreference(raw) ? { kind: "theme", value: raw } : null,
  },
  editor: {
    key: "editor",
    label: m.config_setting_key_editor,
    placeholder: m.config_settings_placeholder_editor_command,
    toValue: (raw: string): UpsertUserSettingInput["value"] => ({
      kind: "editor",
      value: raw,
    }),
  },
} as const satisfies Record<UserSettingKey, UserSettingDescriptor>;

export const USER_SETTING_DESCRIPTORS: readonly UserSettingDescriptor[] =
  USER_SETTING_KEY_VALUES.map((key) => SETTING_DESCRIPTOR_BY_KEY[key]);

export function userSettingDescriptorForKey(
  key: UserSettingKey,
): UserSettingDescriptor {
  return SETTING_DESCRIPTOR_BY_KEY[key];
}

export interface CredentialKindDescriptor {
  kind: UserCredentialKind;
  label: () => string;
}

const CREDENTIAL_KIND_LABELS = {
  provider_api_token: m.config_credential_kind_provider_api_token,
  harness_api_token: m.config_credential_kind_harness_api_token,
} as const satisfies Record<UserCredentialKind, () => string>;

export const USER_CREDENTIAL_KIND_DESCRIPTORS: readonly CredentialKindDescriptor[] =
  USER_CREDENTIAL_KIND_VALUES.map((kind) => ({
    kind,
    label: CREDENTIAL_KIND_LABELS[kind],
  }));

export function toUserCredentialKind(value: string): UserCredentialKind | null {
  for (const kind of USER_CREDENTIAL_KIND_VALUES) {
    if (kind === value) {
      return kind;
    }
  }
  return null;
}

function isThemePreference(value: string): value is ThemePreference {
  for (const candidate of THEME_PREFERENCE_VALUES) {
    if (candidate === value) {
      return true;
    }
  }
  return false;
}
