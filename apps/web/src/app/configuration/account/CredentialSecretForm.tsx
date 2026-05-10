import { useRef } from "react";
import type { FormEvent, ReactNode } from "react";

import { secretInput } from "@/app/lib/api-contracts";
import type { SecretInput } from "@/app/lib/api-contracts";

interface CredentialSecretFormProps {
  busy: boolean;
  className?: string;
  placeholder: string;
  submitLabel: string;
  onSubmit: (secret: SecretInput) => Promise<void>;
}

export function CredentialSecretForm({
  busy,
  className,
  placeholder,
  submitLabel,
  onSubmit,
}: CredentialSecretFormProps): ReactNode {
  const secretFieldRef = useRef<HTMLInputElement>(null);

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const input = secretFieldRef.current;
    if (input === null) {
      return;
    }
    const capturedSecret = input.value;
    // Clear immediately after capture so the DOM never retains prior secrets.
    input.value = "";
    if (capturedSecret === "") {
      return;
    }
    await onSubmit(secretInput(capturedSecret));
  }

  return (
    <form className={className} onSubmit={(event) => void submit(event)}>
      <input
        ref={secretFieldRef}
        type="password"
        disabled={busy}
        className="min-w-48 flex-1 rounded-md border border-[--color-border] bg-[--color-bg-canvas] px-3 py-1.5 text-sm"
        placeholder={placeholder}
        autoComplete="off"
      />
      <button
        type="submit"
        disabled={busy}
        className="rounded-md bg-[--color-accent] px-3 py-1.5 text-sm text-[--color-accent-fg] disabled:opacity-60"
      >
        {submitLabel}
      </button>
    </form>
  );
}
