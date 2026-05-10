export const ACCOUNT_HOME_ROUTE = "/account";
export const CONFIGURATION_ACCOUNT_ROUTE = "/configuration/account";

export type PostAuthSource = "sign_in" | "sign_up" | "invitation";

export interface PostAuthRouteContext {
  from?: PostAuthSource;
}

export type AccountHomeSearchParams = Record<
  string,
  string | string[] | undefined
>;

function readParam(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) {
    return value[0];
  }
  return value;
}

export function parsePostAuthSource(
  value: string | string[] | undefined,
): PostAuthSource | undefined {
  const parsed = readParam(value);
  if (parsed === "sign_in" || parsed === "sign_up" || parsed === "invitation") {
    return parsed;
  }
  return undefined;
}

export function parseAccountHomeRouteContext(
  params: AccountHomeSearchParams,
): PostAuthRouteContext {
  const from = parsePostAuthSource(params["from"]);
  return from === undefined ? {} : { from };
}

export function buildAccountHomeRoute(
  context: PostAuthRouteContext = {},
): string {
  const params = new URLSearchParams();

  if (typeof context.from === "string") {
    params.set("from", context.from);
  }

  const query = params.toString();
  if (query === "") {
    return ACCOUNT_HOME_ROUTE;
  }
  return `${ACCOUNT_HOME_ROUTE}?${query}`;
}
