"use client";

import { useEffect, useState, type JSX } from "react";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

export default function Home(): JSX.Element {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_URL}/health`)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return (await response.json()) as HealthReport;
      })
      .then((data) => {
        if (!cancelled) {
          setReport(data);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
      <h1 style={{ fontSize: "2rem", fontWeight: 600 }}>Tanren</h1>
      <p style={{ opacity: 0.7 }}>
        F-0001 placeholder — minimum buildable Tanren.
      </p>
      <section
        style={{
          background: "#13171c",
          border: "1px solid #2a2f36",
          borderRadius: "0.5rem",
          padding: "1rem 1.5rem",
          minWidth: "20rem",
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        }}
      >
        {report !== null ? (
          <pre style={{ margin: 0 }}>{JSON.stringify(report, null, 2)}</pre>
        ) : error !== null ? (
          <span style={{ color: "#ff6b6b" }}>API unreachable: {error}</span>
        ) : (
          <span style={{ opacity: 0.5 }}>Loading {API_URL}/health…</span>
        )}
      </section>
    </main>
  );
}
