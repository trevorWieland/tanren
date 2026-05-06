"use client";

import type { ReactNode } from "react";

import { OrganizationCreateForm } from "@/components/organization/OrganizationCreateForm";
import * as m from "@/i18n/paraglide/messages";

export default function NewOrganizationPage(): ReactNode {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.orgCreate_title()}</h1>
      <OrganizationCreateForm
        onSuccess={() => {
          window.location.assign("/");
        }}
      />
    </main>
  );
}
