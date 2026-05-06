import { Routes, Route, Outlet } from "react-router";
import type { ReactNode } from "react";

import Home from "./app/page";
import SignInPage from "./app/sign-in/page";
import SignUpPage from "./app/sign-up/page";
import InvitationPage from "./app/invitations/[token]/page";

function Layout(): ReactNode {
  return <Outlet />;
}

export function App(): ReactNode {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route path="/" element={<Home />} />
        <Route path="/sign-in" element={<SignInPage />} />
        <Route path="/sign-up" element={<SignUpPage />} />
        <Route path="/invitations/:token" element={<InvitationPage />} />
      </Route>
    </Routes>
  );
}
