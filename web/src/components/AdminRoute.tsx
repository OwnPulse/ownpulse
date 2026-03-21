// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "../store/auth";

export default function AdminRoute() {
  const role = useAuthStore((s) => s.role);
  if (role !== "admin") {
    return <Navigate to="/" replace />;
  }
  return <Outlet />;
}
