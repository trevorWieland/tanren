import { useState } from "react";
import type { FormEvent, ReactNode } from "react";

interface CredentialSecretFormProps {
  busy: boolean;
  className?: string;
  placeholder: string;
  submitLabel: string;
  onSubmit: (secret: string) => Promise<void>;
}

export function CredentialSecretForm({
  busy,
  className,
  placeholder,
  submitLabel,
  onSubmit,
}: CredentialSecretFormProps): ReactNode {
  const [secret, setSecret] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const secretValue = secret.trim();
    if (secretValue === "") {
      return;
    }
    // Clear immediately so raw secrets are not retained in rendered state.
    setSecret("");
    await onSubmit(secretValue);
  }

  return (
    <form className={className} onSubmit={(event) => void submit(event)}>
      <input
        type="password"
        value={secret}
        onChange={(event) => setSecret(event.target.value)}
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
