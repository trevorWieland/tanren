import Link from "next/link";
import type { ReactNode } from "react";

import {
  CONFIGURATION_ACCOUNT_ROUTE,
  type PostAuthRouteContext,
} from "@/app/lib/navigation";

interface AccountHomePageProps {
  searchParams:
    | Promise<Record<string, string | string[] | undefined>>
    | Record<string, string | string[] | undefined>;
}

function readSource(
  value: string | string[] | undefined,
): PostAuthRouteContext["source"] {
  const source = readParam(value);
  if (source === "sign_in" || source === "sign_up" || source === "invitation") {
    return source;
  }
  return undefined;
}

function readParam(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) {
    return value[0];
  }
  return value;
}

function pickSummaryItems(context: PostAuthRouteContext): string[] {
  const items: string[] = [];
  if (typeof context.displayName === "string" && context.displayName !== "") {
    items.push(`Signed in as ${context.displayName}`);
  }
  if (
    typeof context.accountIdentifier === "string" &&
    context.accountIdentifier !== ""
  ) {
    items.push(`Account ${context.accountIdentifier}`);
  }
  if (
    typeof context.joinedOrganization === "string" &&
    context.joinedOrganization !== ""
  ) {
    items.push(`Joined organization ${context.joinedOrganization}`);
  }
  return items;
}

export default async function AccountHomePage(
  props: AccountHomePageProps,
): Promise<ReactNode> {
  const params = await props.searchParams;
  const context: PostAuthRouteContext = {};

  const accountId = readParam(params["account_id"]);
  if (typeof accountId === "string" && accountId !== "") {
    context.accountId = accountId;
  }
  const accountIdentifier = readParam(params["account"]);
  if (typeof accountIdentifier === "string" && accountIdentifier !== "") {
    context.accountIdentifier = accountIdentifier;
  }
  const displayName = readParam(params["name"]);
  if (typeof displayName === "string" && displayName !== "") {
    context.displayName = displayName;
  }
  const joinedOrganization = readParam(params["joined_org"]);
  if (typeof joinedOrganization === "string" && joinedOrganization !== "") {
    context.joinedOrganization = joinedOrganization;
  }
  const source = readSource(params["from"]);
  if (typeof source === "string") {
    context.source = source;
  }

  const summaryItems = pickSummaryItems(context);

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 px-6 py-10">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
        <h1 className="text-2xl font-semibold">Account home</h1>
        <p className="mt-2 text-sm text-[--color-fg-muted]">
          Your session is active. Continue to configuration when you are ready.
        </p>
      </header>

      {summaryItems.length > 0 ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
          <h2 className="text-lg font-medium">Session context</h2>
          <ul className="mt-3 list-disc pl-5 text-sm text-[--color-fg-muted]">
            {summaryItems.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </section>
      ) : null}

      <section className="grid gap-3 sm:grid-cols-2">
        <Link
          href={CONFIGURATION_ACCOUNT_ROUTE}
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          Open account configuration
        </Link>
        <Link
          href="/"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          Return to landing page
        </Link>
      </section>
    </main>
  );
}
