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

export type UserSettingView = components["schemas"]["UserSettingView"];
export type UserCredentialView = components["schemas"]["UserCredentialView"];

export type UpsertUserSettingInput =
  OperationJsonRequest<"upsert_authenticated_user_setting_route">;
export type UpsertUserSettingResult = OperationJsonResponse<
  "upsert_authenticated_user_setting_route",
  200
>;

export type ListUserSettingsResult = OperationJsonResponse<
  "list_authenticated_user_settings_route",
  200
>;

export type RemoveUserSettingResult = OperationJsonResponse<
  "remove_authenticated_user_setting_route",
  200
>;

export type CreateUserCredentialInput =
  OperationJsonRequest<"add_authenticated_user_credential_route">;
export type CreateUserCredentialResult = OperationJsonResponse<
  "add_authenticated_user_credential_route",
  201
>;

export type UpdateUserCredentialInput =
  OperationJsonRequest<"update_authenticated_user_credential_route">;
export type UpdateUserCredentialResult = OperationJsonResponse<
  "update_authenticated_user_credential_route",
  200
>;

export type ListUserCredentialsResult = OperationJsonResponse<
  "list_authenticated_user_credentials_route",
  200
>;

export type RemoveUserCredentialResult = OperationJsonResponse<
  "remove_authenticated_user_credential_route",
  200
>;

export type AccountFailure = components["schemas"]["AccountFailureBody"];

export type GetConfigurationCapabilitiesResult = OperationJsonResponse<
  "get_authenticated_user_configuration_capabilities_route",
  200
>;

export type ConfigurationCapabilities =
  GetConfigurationCapabilitiesResult["capabilities"];

export interface ConfigurationDiscoveryResult {
  capabilities: ConfigurationCapabilities;
  settings: UserSettingView[];
  credentials: UserCredentialView[];
  capabilities_failure: AccountFailure | null;
  settings_failure: AccountFailure | null;
  credentials_failure: AccountFailure | null;
}
