"use client";

import { use, useState, type FormEvent, type JSX } from "react";

import {
  AccountRequestError,
  acceptInvitation,
  describeFailure,
  persistSession,
  type AcceptInvitationResult,
} from "../../lib/account-client";

interface InvitationPageProps {
  params: Promise<{ token: string }>;
}

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

export default function InvitationAcceptPage(
  props: InvitationPageProps,
): JSX.Element {
  const { token } = use(props.params);
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [accepted, setAccepted] = useState<AcceptInvitationResult | null>(null);

  async function onSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setErrorMessage(null);

    const trimmedDisplayName = displayName.trim();
    if (password === "" || trimmedDisplayName === "") {
      setErrorMessage("Password and display name are required.");
      return;
    }

    setSubmitting(true);
    try {
      const result = await acceptInvitation(token, {
        password,
        display_name: trimmedDisplayName,
      });
      persistSession(result.account.id, result.session.token);
      setAccepted(result);
    } catch (cause: unknown) {
      if (cause instanceof AccountRequestError) {
        setErrorMessage(describeFailure(cause.failure));
      } else if (cause instanceof Error) {
        setErrorMessage(cause.message);
      } else {
        setErrorMessage("Failed to accept invitation.");
      }
    } finally {
      setSubmitting(false);
    }
  }

  if (accepted !== null) {
    return (
      <main
        style={{
          minHeight: "100vh",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          gap: "1rem",
          padding: "2rem",
        }}
      >
        <h1 style={{ fontSize: "1.75rem", fontWeight: 600 }}>
          Invitation accepted
        </h1>
        <p role="status" style={{ margin: 0 }}>
          You have joined organization {accepted.joined_org}.
        </p>
        <a
          href="/"
          style={{
            color: "#7aa6ff",
            textDecoration: "underline",
          }}
        >
          Continue
        </a>
      </main>
    );
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
        Accept invitation
      </h1>
      <p style={{ opacity: 0.7, margin: 0 }}>
        Create an account to join the organization that invited you.
      </p>
      <form onSubmit={onSubmit} style={formStyle} noValidate>
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
        <button type="submit" disabled={submitting} style={buttonStyle}>
          {submitting ? "Accepting…" : "Accept invitation"}
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
