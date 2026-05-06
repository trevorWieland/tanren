/**
 * Shared TypeScript contract types mirroring the Rust wire shapes in
 * `tanren-contract::organization`. Kept in lock-step with the Rust enum
 * and struct definitions so every interface (api, mcp, cli, tui, web)
 * projects the same JSON shape.
 */

export type OrgPermission =
  | "invite_members"
  | "manage_access"
  | "configure"
  | "set_policy"
  | "delete";

export interface OrganizationView {
  id: string;
  name: string;
  created_at: string;
}

export interface OrganizationMembershipView {
  id: string;
  account_id: string;
  org_id: string;
  permissions: OrgPermission[];
  created_at: string;
}

export interface CreateOrganizationResponse {
  organization: OrganizationView;
  membership: OrganizationMembershipView;
}

export interface ListOrganizationsResponse {
  organizations: OrganizationView[];
  next_cursor: string | null;
}
