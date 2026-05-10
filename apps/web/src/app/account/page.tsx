import Link from "next/link";
import type { JSX } from "react";

import {
  AccountRequestError,
  fetchAuthenticatedAccountHome,
} from "@/app/lib/account-client";
import {
  CONFIGURATION_ACCOUNT_ROUTE,
  parseAccountHomeRouteContext,
  type AccountHomeSearchParams,
  type PostAuthSource,
} from "@/app/lib/navigation";
import * as m from "@/i18n/paraglide/messages";

interface AccountHomePageProps {
  searchParams: Promise<AccountHomeSearchParams> | AccountHomeSearchParams;
}

interface SummaryFieldDescriptor {
  id:
    | "source"
    | "display_name"
    | "identifier"
    | "organization"
    | "session"
    | "read_at";
  label: string;
  value: string;
}

const dateFormatter = new Intl.DateTimeFormat("en-US", {
  dateStyle: "medium",
  timeStyle: "short",
  timeZone: "UTC",
});

function describeSource(source: PostAuthSource): string {
  switch (source) {
    case "sign_in":
      return m.accountHome_source_signIn();
    case "sign_up":
      return m.accountHome_source_signUp();
    case "invitation":
      return m.accountHome_source_invitation();
    default:
      source satisfies never;
      return "";
  }
}

function formatTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return dateFormatter.format(parsed);
}

export default async function AccountHomePage(
  props: AccountHomePageProps,
): Promise<JSX.Element> {
  const params = await props.searchParams;
  const context = parseAccountHomeRouteContext(params);

  let accountHome: Awaited<
    ReturnType<typeof fetchAuthenticatedAccountHome>
  > | null = null;
  let authRequired = false;
  try {
    accountHome = await fetchAuthenticatedAccountHome();
  } catch (cause: unknown) {
    if (
      cause instanceof AccountRequestError &&
      cause.failure.code === "auth_required"
    ) {
      authRequired = true;
    } else {
      throw cause;
    }
  }

  const summaryFields: SummaryFieldDescriptor[] = [];
  if (context.from !== undefined) {
    summaryFields.push({
      id: "source",
      label: m.accountHome_summary_source(),
      value: describeSource(context.from),
    });
  }
  if (accountHome !== null) {
    summaryFields.push(
      {
        id: "display_name",
        label: m.accountHome_summary_displayName(),
        value: accountHome.account.display_name,
      },
      {
        id: "identifier",
        label: m.accountHome_summary_identifier(),
        value: accountHome.account.identifier,
      },
      {
        id: "session",
        label: m.accountHome_summary_sessionExpires(),
        value: formatTimestamp(accountHome.freshness.session_expires_at),
      },
      {
        id: "read_at",
        label: m.accountHome_summary_snapshot(),
        value: formatTimestamp(accountHome.freshness.read_at),
      },
    );
    if (
      typeof accountHome.account.org === "string" &&
      accountHome.account.org !== ""
    ) {
      summaryFields.push({
        id: "organization",
        label: m.accountHome_summary_organization(),
        value: accountHome.account.org,
      });
    }
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 px-6 py-10">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
        <h1 className="text-2xl font-semibold">{m.accountHome_title()}</h1>
        <p className="mt-2 text-sm text-[--color-fg-muted]">
          {authRequired
            ? m.accountHome_subtitleSignedOut()
            : m.accountHome_subtitleSignedIn()}
        </p>
      </header>

      {summaryFields.length > 0 ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
          <h2 className="text-lg font-medium">
            {m.accountHome_summary_title()}
          </h2>
          <dl className="mt-3 grid gap-3">
            {summaryFields.map((field) => (
              <div
                key={field.id}
                className="grid gap-1 sm:grid-cols-[14rem_1fr]"
              >
                <dt className="text-sm font-medium">{field.label}</dt>
                <dd className="text-sm text-[--color-fg-muted]">
                  {field.value}
                </dd>
              </div>
            ))}
          </dl>
        </section>
      ) : null}

      <section className="grid gap-3 sm:grid-cols-2">
        {authRequired ? (
          <Link
            href="/sign-in"
            className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
          >
            {m.accountHome_link_signIn()}
          </Link>
        ) : (
          <Link
            href={CONFIGURATION_ACCOUNT_ROUTE}
            className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
          >
            {m.accountHome_link_configuration()}
          </Link>
        )}
        <Link
          href="/"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          {m.accountHome_link_landing()}
        </Link>
      </section>
    </main>
  );
}
