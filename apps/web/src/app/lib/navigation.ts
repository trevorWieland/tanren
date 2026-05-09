export const ACCOUNT_HOME_ROUTE = "/account";
export const CONFIGURATION_ACCOUNT_ROUTE = "/configuration/account";

export interface PostAuthRouteContext {
  accountId?: string;
  accountIdentifier?: string;
  displayName?: string;
  joinedOrganization?: string;
  source?: "sign_in" | "sign_up" | "invitation";
}

export function buildAccountHomeRoute(
  context: PostAuthRouteContext = {},
): string {
  const params = new URLSearchParams();

  if (typeof context.accountId === "string" && context.accountId !== "") {
    params.set("account_id", context.accountId);
  }
  if (
    typeof context.accountIdentifier === "string" &&
    context.accountIdentifier !== ""
  ) {
    params.set("account", context.accountIdentifier);
  }
  if (typeof context.displayName === "string" && context.displayName !== "") {
    params.set("name", context.displayName);
  }
  if (
    typeof context.joinedOrganization === "string" &&
    context.joinedOrganization !== ""
  ) {
    params.set("joined_org", context.joinedOrganization);
  }
  if (typeof context.source === "string") {
    params.set("from", context.source);
  }

  const query = params.toString();
  if (query === "") {
    return ACCOUNT_HOME_ROUTE;
  }
  return `${ACCOUNT_HOME_ROUTE}?${query}`;
}
