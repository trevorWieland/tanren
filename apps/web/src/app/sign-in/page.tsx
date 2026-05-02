"use client";

import { useRouter } from "next/navigation";
import { useState, type FormEvent, type JSX } from "react";

import {
  AccountRequestError,
  describeFailure,
  persistSession,
  signIn,
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

export default function SignInPage(): JSX.Element {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function onSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setErrorMessage(null);

    const trimmedEmail = email.trim().toLowerCase();
    if (trimmedEmail === "" || password === "") {
      setErrorMessage("Email and password are required.");
      return;
    }

    setSubmitting(true);
    try {
      const result = await signIn({ email: trimmedEmail, password });
      persistSession(result.account.id, result.session.token);
      router.push("/");
    } catch (cause: unknown) {
      if (cause instanceof AccountRequestError) {
        setErrorMessage(describeFailure(cause.failure));
      } else if (cause instanceof Error) {
        setErrorMessage(cause.message);
      } else {
        setErrorMessage("Sign-in failed.");
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
        Sign in to Tanren
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
            autoComplete="current-password"
            value={password}
            onChange={(event) => {
              setPassword(event.target.value);
            }}
            style={inputStyle}
          />
        </label>
        <button type="submit" disabled={submitting} style={buttonStyle}>
          {submitting ? "Signing in…" : "Sign in"}
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
