import type { JSX, ReactNode } from "react";

export const metadata = {
  title: "Tanren",
  description: "Tanren control plane for agentic software delivery.",
};

interface RootLayoutProps {
  children: ReactNode;
}

export default function RootLayout({ children }: RootLayoutProps): JSX.Element {
  return (
    <html lang="en">
      <body
        style={{
          fontFamily:
            "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif",
          margin: 0,
          padding: 0,
          background: "#0b0d10",
          color: "#e6e8eb",
        }}
      >
        {children}
      </body>
    </html>
  );
}
