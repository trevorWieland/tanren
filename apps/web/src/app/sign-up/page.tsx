import { useNavigate } from "react-router";
import type { ReactNode } from "react";

import { SignUpForm } from "@/components/account/SignUpForm";
import * as m from "@/i18n/paraglide/messages";

export default function SignUpPage(): ReactNode {
  const navigate = useNavigate();
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-2xl font-semibold">{m.signUp_title()}</h1>
      <SignUpForm
        onSuccess={() => {
          navigate("/");
        }}
      />
    </main>
  );
}
