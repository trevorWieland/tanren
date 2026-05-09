import Link from "next/link";
import type { ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";

export default function LandingPage(): ReactNode {
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-3xl flex-col justify-center gap-6 px-6 py-10">
      <header className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-6">
        <h1 className="text-2xl font-semibold">{m.app_title()}</h1>
        <p className="mt-2 text-sm text-[--color-fg-muted]">
          {m.landing_subtitle()}
        </p>
      </header>
      <nav className="grid gap-3 sm:grid-cols-3">
        <Link
          href="/sign-in"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          {m.landing_link_sign_in()}
        </Link>
        <Link
          href="/sign-up"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          {m.landing_link_create_account()}
        </Link>
        <Link
          href="/configuration/account"
          className="rounded-md border border-[--color-border] bg-[--color-bg-surface] px-4 py-3 text-sm"
        >
          {m.landing_link_configuration()}
        </Link>
      </nav>
    </main>
  );
}
