import type { components, operations } from "@/app/lib/generated-openapi";

type JsonContent<T> = T extends {
  content: { "application/json": infer Body };
}
  ? Body
  : never;

type OperationJsonResponse<
  Op extends keyof operations,
  Status extends keyof operations[Op]["responses"],
> = JsonContent<operations[Op]["responses"][Status]>;

type OperationJsonRequest<Op extends keyof operations> =
  operations[Op] extends {
    requestBody: { content: { "application/json": infer Body } };
  }
    ? Body
    : never;

export type UserSettingKey = components["schemas"]["UserSettingKey"];
export type UserCredentialKind = components["schemas"]["UserCredentialKind"];
export type UserSettingValue = components["schemas"]["UserSettingValue"];
declare const SECRET_INPUT_BRAND: unique symbol;
declare const USER_CREDENTIAL_ITEM_ID_BRAND: unique symbol;
export type SecretInput = {
  readonly value: string;
  readonly [SECRET_INPUT_BRAND]: "SecretInput";
};

export function secretInput(value: string): SecretInput {
  return { value } as SecretInput;
}

export type UserSettingView = components["schemas"]["UserSettingView"];
export type UserCredentialView = components["schemas"]["UserCredentialView"];
export type UserCredentialItemId = string & {
  readonly [USER_CREDENTIAL_ITEM_ID_BRAND]: "UserCredentialItemId";
};

export function userCredentialItemId(
  value: UserCredentialView["id"],
): UserCredentialItemId {
  return value as UserCredentialItemId;
}

export interface CredentialListPageInput {
  cursor?: string;
  page_size?: number;
}

type SharedUpsertUserSettingInput =
  OperationJsonRequest<"upsert_user_setting_route">;
type SharedUpsertUserSettingResult = OperationJsonResponse<
  "upsert_user_setting_route",
  200
>;
export type UpsertUserSettingInput = SharedUpsertUserSettingInput;
export type UpsertUserSettingResult = SharedUpsertUserSettingResult;

type SharedListUserSettingsResult = OperationJsonResponse<
  "list_user_settings_route",
  200
>;
interface ListMetadata {
  next_cursor?: string | null;
  freshness?: string | null;
  as_of?: string | null;
  generated_at?: string | null;
}

export type ListUserSettingsResult = SharedListUserSettingsResult &
  ListMetadata;

type SharedRemoveUserSettingResult = OperationJsonResponse<
  "remove_user_setting_route",
  200
>;
export type RemoveUserSettingResult = SharedRemoveUserSettingResult;

type SharedCreateUserCredentialResult = OperationJsonResponse<
  "add_user_credential_route",
  201
>;
type AuthenticatedCreateUserCredentialInput =
  OperationJsonRequest<"add_authenticated_user_credential_route">;
export type CreateUserCredentialInput = AuthenticatedCreateUserCredentialInput;
export type CreateUserCredentialResult = SharedCreateUserCredentialResult;

type SharedUpdateUserCredentialInput =
  OperationJsonRequest<"update_user_credential_route">;
type SharedUpdateUserCredentialResult = OperationJsonResponse<
  "update_user_credential_route",
  200
>;
export type UpdateUserCredentialInput = SharedUpdateUserCredentialInput;
export type UpdateUserCredentialResult = SharedUpdateUserCredentialResult;

type SharedListUserCredentialsResult = OperationJsonResponse<
  "list_user_credentials_route",
  200
>;
export type ListUserCredentialsResult = SharedListUserCredentialsResult &
  ListMetadata;

type SharedRemoveUserCredentialResult = OperationJsonResponse<
  "remove_user_credential_route",
  200
>;
export type RemoveUserCredentialResult = SharedRemoveUserCredentialResult;

export type AccountFailure = components["schemas"]["AccountFailureBody"];

export type GetConfigurationCapabilitiesResult = OperationJsonResponse<
  "get_authenticated_user_configuration_capabilities_route",
  200
>;

export type ConfigurationCapabilities =
  GetConfigurationCapabilitiesResult["capabilities"];

export interface ConfigurationDiscoveryResult {
  capabilities: ConfigurationCapabilities;
  settings_read_model: ListUserSettingsResult | null;
  credentials_read_model: ListUserCredentialsResult | null;
  capabilities_failure: AccountFailure | null;
  settings_failure: AccountFailure | null;
  credentials_failure: AccountFailure | null;
}
