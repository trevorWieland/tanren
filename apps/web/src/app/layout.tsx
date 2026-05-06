import type { ReactNode } from "react";

import { QueryProvider } from "@/lib/query-provider";

import "./globals.css";

export const metadata = {
  title: "Tanren",
  description: "Tanren control plane for agentic software delivery.",
};

interface RootLayoutProps {
  children: ReactNode;
}

export default function RootLayout({ children }: RootLayoutProps): ReactNode {
  return (
    <html lang="en">
      <body>
        <QueryProvider>{children}</QueryProvider>
      </body>
    </html>
  );
}
