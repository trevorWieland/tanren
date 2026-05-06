import { useNavigate } from "react-router";
import type { ReactNode } from "react";

import { SignInForm } from "@/components/account/SignInForm";
import * as m from "@/i18n/paraglide/messages";

export default function SignInPage(): ReactNode {
  const navigate = useNavigate();
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.signIn_title()}</h1>
      <SignInForm
        onSuccess={() => {
          navigate("/");
        }}
      />
    </main>
  );
}
