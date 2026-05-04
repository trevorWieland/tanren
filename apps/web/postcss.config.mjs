// Tailwind v4 uses the `@tailwindcss/postcss` plugin. The CSS-first
// `@theme` block in `apps/web/src/app/globals.css` carries the design
// tokens; no `tailwind.config.ts` is required.
const config = {
  plugins: {
    "@tailwindcss/postcss": {},
  },
};

export default config;
