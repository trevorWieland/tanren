import {
  ROLE_READ_MODEL_PAGE_DEFAULT,
  ROLE_READ_MODEL_PAGE_MAX,
} from "./generated/role-contract";

export {
  type RoleCapabilitySnapshot,
  fetchRoleCapabilities,
  hasRoleActionCapability,
  requireRoleActionSnapshot,
  requireRoleCapabilitySnapshot,
} from "./role-client/capabilities";

export { formatRoleError, RoleRequestError } from "./role-client/errors";

export {
  type PrincipalKind,
  type ScopeKind,
  readAccountPrincipalRef,
  readPermissionBundle,
  readPermissionNameField,
  readPermissionScope,
  readPrincipalKindField,
  readRequiredField,
  readRoleIdField,
  readRoleNameField,
  readRolePrincipalRejectionRef,
  readRoleScope,
} from "./role-client/form-parsing";

export {
  applyRole,
  checkPermission,
  checkPermissionRolePrincipalRejection,
  createRole,
  deleteRole,
  editRole,
  type PermissionCheckRolePrincipalRejectionRequest,
  readRoleModel,
} from "./role-client/operations";

export {
  buildApplyRoleRequest,
  buildCreateRoleRequest,
  buildDeleteRoleRequest,
  buildEditRoleRequest,
  buildPermissionCheckRolePrincipalRejectionRequest,
  buildPermissionCheckRequest,
  buildRoleReadModelRequest,
  type ApplyRoleFormSubmission,
  type ApplyRoleAccountPrincipalRequest,
  type CreateRoleFormSubmission,
  type DeleteRoleFormSubmission,
  type EditRoleFormSubmission,
  type PermissionCheckAccountPrincipalRequest,
  type PermissionCheckFormSubmission,
  type PermissionCheckRolePrincipalRejectionFormSubmission,
  type RoleReadModelAccountPrincipalRequest,
  type RoleReadModelRequestInput,
  permissionScopeFromRoleScope,
  type RoleRequestContextInput,
  roleScopeFromPermissionScope,
} from "./role-client/request-builders";

export { ROLE_REQUEST_TIMEOUT_MS } from "./role-client/transport";

export const ROLE_READ_MODEL_DEFAULT_ROLE_PAGE_SIZE =
  ROLE_READ_MODEL_PAGE_DEFAULT;
export const ROLE_READ_MODEL_DEFAULT_GRANT_PAGE_SIZE =
  ROLE_READ_MODEL_PAGE_DEFAULT;
export const ROLE_READ_MODEL_MAX_PAGE_SIZE = ROLE_READ_MODEL_PAGE_MAX;
