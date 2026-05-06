import { createRoot } from "react-dom/client";
import type { ReactNode } from "react";

import { AcceptInvitationForm } from "@/components/account/AcceptInvitationForm";
import { SignInForm } from "@/components/account/SignInForm";
import { SignUpForm } from "@/components/account/SignUpForm";
import { QueryProvider } from "@/app/lib/query-provider";
import { NewProjectRoute } from "@/routes/projects/NewProjectRoute";
import * as m from "@/i18n/paraglide/messages";

import "@/app/globals.css";

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("Root element not found");
createRoot(rootEl).render(
  <QueryProvider>
    <App />
  </QueryProvider>,
);

function App(): ReactNode {
  const path = window.location.pathname;

  if (path === "/projects/new") {
    return <NewProjectRoute />;
  }

  if (path === "/sign-up") {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
        <h1 className="text-2xl font-semibold">{m.signUp_title()}</h1>
        <SignUpForm
          onSuccess={() => {
            window.location.href = "/";
          }}
        />
      </main>
    );
  }

  if (path === "/sign-in") {
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
        <h1 className="text-2xl font-semibold">{m.signIn_title()}</h1>
        <SignInForm
          onSuccess={() => {
            window.location.href = "/";
          }}
        />
      </main>
    );
  }

  const invitationMatch = path.match(/^\/invitations\/([^/]+)$/);
  if (invitationMatch) {
    const token = invitationMatch[1] ?? "";
    return (
      <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
        <h1 className="text-2xl font-semibold">{m.acceptInvitation_title()}</h1>
        <p className="m-0 text-[--color-fg-muted]">
          {m.acceptInvitation_subtitle()}
        </p>
        <AcceptInvitationForm
          token={token}
          onSuccess={() => {
            window.location.href = "/";
          }}
        />
      </main>
    );
  }

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
      <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>
    </main>
  );
}
