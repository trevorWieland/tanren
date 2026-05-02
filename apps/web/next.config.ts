import type { NextConfig } from "next";

const config: NextConfig = {
  reactStrictMode: true,
  // Turbopack is enabled per-script via `next dev --turbopack` and
  // `next build --turbopack` rather than via this config block, so the
  // setting tracks the CLI invocation rather than the config file.
};

export default config;
