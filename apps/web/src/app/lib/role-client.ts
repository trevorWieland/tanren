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
  buildPermissionCheckRequest,
  type ApplyRoleFormSubmission,
  type CreateRoleFormSubmission,
  type DeleteRoleFormSubmission,
  type EditRoleFormSubmission,
  type PermissionCheckFormSubmission,
  permissionScopeFromRoleScope,
  type RoleRequestContextInput,
  roleScopeFromPermissionScope,
} from "./role-client/request-builders";

export { ROLE_REQUEST_TIMEOUT_MS } from "./role-client/transport";

export const ROLE_READ_MODEL_DEFAULT_ROLE_PAGE_SIZE = 50;
export const ROLE_READ_MODEL_DEFAULT_GRANT_PAGE_SIZE = 50;
