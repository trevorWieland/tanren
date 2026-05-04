"use client";

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";

import {
  OrganizationRequestError,
  listOrganizations,
  type OrganizationView,
} from "./actions";
import * as m from "@/i18n/paraglide/messages";

export default function OrganizationsPage(): ReactNode {
  const router = useRouter();
  const [orgs, setOrgs] = useState<OrganizationView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    listOrganizations()
      .then((result) => {
        if (!cancelled) {
          setOrgs(result.organizations);
          setLoading(false);
        }
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        if (
          cause instanceof OrganizationRequestError &&
          cause.failure.code === "unauthenticated"
        ) {
          router.push("/sign-in");
          return;
        }
        setError(
          cause instanceof OrganizationRequestError
            ? cause.message
            : cause instanceof Error
              ? cause.message
              : String(cause),
        );
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [router]);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.orgs_title()}</h1>
      {loading ? null : error !== null ? (
        <p className="m-0 text-[--color-error]">{error}</p>
      ) : orgs.length === 0 ? (
        <p className="m-0 text-[--color-fg-muted]">{m.orgs_empty()}</p>
      ) : (
        <ul className="w-full max-w-md list-none p-0">
          {orgs.map((org) => (
            <li
              key={org.id}
              className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3"
            >
              {org.name}
            </li>
          ))}
        </ul>
      )}
      <Link
        href="/organizations/new"
        className="rounded-md bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover]"
      >
        {m.orgs_createNew()}
      </Link>
    </main>
  );
}
