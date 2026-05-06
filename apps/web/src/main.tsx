import { StrictMode } from "react";
import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";

import { InvitationPage } from "./routes/InvitationPage";

import "./app/globals.css";

function App(): ReactNode {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/invitations/:token" element={<InvitationPage />} />
      </Routes>
    </BrowserRouter>
  );
}

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
