import { StrictMode } from "react";
import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { InvitationPage } from "./routes/InvitationPage";

import "./app/globals.css";

const queryClient = new QueryClient();

function App(): ReactNode {
  return (
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Routes>
            <Route path="/invitations/:token" element={<InvitationPage />} />
          </Routes>
        </BrowserRouter>
      </QueryClientProvider>
    </StrictMode>
  );
}

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(<App />);
}
