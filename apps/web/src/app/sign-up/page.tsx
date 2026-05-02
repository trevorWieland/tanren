"use client";

import { useRouter } from "next/navigation";
import { useState, type FormEvent, type JSX } from "react";

import {
  AccountRequestError,
  describeFailure,
  persistSession,
  signUp,
} from "../lib/account-client";

const formStyle = {
  display: "flex",
  flexDirection: "column" as const,
  gap: "1rem",
  width: "100%",
  maxWidth: "24rem",
};

const inputStyle = {
  padding: "0.6rem 0.75rem",
  borderRadius: "0.4rem",
  border: "1px solid #2a2f36",
  background: "#13171c",
  color: "#e6e8eb",
  fontSize: "1rem",
};

const buttonStyle = {
  padding: "0.7rem 1rem",
  borderRadius: "0.4rem",
  border: "1px solid #2a2f36",
  background: "#1f6feb",
  color: "#fff",
  fontSize: "1rem",
  cursor: "pointer",
};

export default function SignUpPage(): JSX.Element {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function onSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setErrorMessage(null);

    const trimmedEmail = email.trim().toLowerCase();
    const trimmedDisplayName = displayName.trim();
    if (trimmedEmail === "" || password === "" || trimmedDisplayName === "") {
      setErrorMessage("Email, password, and display name are required.");
      return;
    }

    setSubmitting(true);
    try {
      const result = await signUp({
        email: trimmedEmail,
        password,
        display_name: trimmedDisplayName,
      });
      persistSession(result.account.id, result.session.token);
      router.push("/");
    } catch (cause: unknown) {
      if (cause instanceof AccountRequestError) {
        setErrorMessage(describeFailure(cause.failure));
      } else if (cause instanceof Error) {
        setErrorMessage(cause.message);
      } else {
        setErrorMessage("Sign-up failed.");
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main
      style={{
        minHeight: "100vh",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "1.5rem",
        padding: "2rem",
      }}
    >
      <h1 style={{ fontSize: "1.75rem", fontWeight: 600 }}>
        Create a Tanren account
      </h1>
      <form onSubmit={onSubmit} style={formStyle} noValidate>
        <label
          style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}
        >
          <span>Email</span>
          <input
            type="email"
            name="email"
            autoComplete="email"
            value={email}
            onChange={(event) => {
              setEmail(event.target.value);
            }}
            style={inputStyle}
          />
        </label>
        <label
          style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}
        >
          <span>Password</span>
          <input
            type="password"
            name="password"
            autoComplete="new-password"
            value={password}
            onChange={(event) => {
              setPassword(event.target.value);
            }}
            style={inputStyle}
          />
        </label>
        <label
          style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}
        >
          <span>Display name</span>
          <input
            type="text"
            name="display_name"
            autoComplete="name"
            value={displayName}
            onChange={(event) => {
              setDisplayName(event.target.value);
            }}
            style={inputStyle}
          />
        </label>
        <button type="submit" disabled={submitting} style={buttonStyle}>
          {submitting ? "Creating account…" : "Create account"}
        </button>
        {errorMessage !== null ? (
          <p role="alert" style={{ color: "#ff6b6b", margin: 0 }}>
            {errorMessage}
          </p>
        ) : null}
      </form>
    </main>
  );
}
