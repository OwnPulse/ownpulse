// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "../store/auth";
import { useAuth } from "../hooks/useAuth";

export default function ProtectedRoute() {
  const { loading } = useAuth();
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);

  if (loading) {
    return <div>Loading...</div>;
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return <Outlet />;
}
