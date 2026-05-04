import type { NextConfig } from "next";

const config: NextConfig = {
  reactStrictMode: true,
  // Turbopack is enabled per-script via `next dev --turbopack` and
  // `next build --turbopack` rather than via this config block, so the
  // setting tracks the CLI invocation rather than the config file.
  //
  // Next 16's dev runtime blocks HMR-websocket requests originating
  // from any host other than the one the user typed into the browser
  // bar. Playwright drives the dev server from `127.0.0.1`, which Next
  // treats as cross-origin against the dev server's default `localhost`
  // view — without this allowlist, the WebSocket handshake fails and
  // the React client bundle never finishes hydrating, which in turn
  // lets the form submit as a native HTML GET (observed during PR 11
  // playwright-bdd bring-up).
  allowedDevOrigins: ["127.0.0.1", "localhost"],
};

export default config;
