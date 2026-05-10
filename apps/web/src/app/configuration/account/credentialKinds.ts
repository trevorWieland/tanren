import type { UserCredentialKind } from "@/app/lib/api-contracts";
import { USER_CREDENTIAL_KIND_VALUES } from "@/app/lib/api-contracts";
import * as m from "@/i18n/paraglide/messages";

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
