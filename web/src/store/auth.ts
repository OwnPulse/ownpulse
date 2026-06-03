// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { create } from "zustand";
import { resetDeviceId } from "../lib/telemetry";
import { telemetry } from "../lib/telemetryMiddleware";

/** Decode the payload of a JWT without verification (backend already verified). */
function decodeJwtPayload(token: string): Record<string, unknown> {
  const parts = token.split(".");
  if (parts.length !== 3) return {};
  try {
    const payload = atob(parts[1].replace(/-/g, "+").replace(/_/g, "/"));
    return JSON.parse(payload);
  } catch {
    return {};
  }
}

interface AuthState {
  /** JWT stored in memory only — never localStorage. */
  token: string | null;
  /** Whether the user is authenticated. */
  isAuthenticated: boolean;
  /** User role extracted from JWT claims. */
  role: string | null;
  /** Set the JWT after login. Refresh token is in an httpOnly cookie (set by backend). */
  login: (token: string) => void;
  /** Clear the JWT and mark as unauthenticated. */
  logout: () => void;
}

export const useAuthStore = create<AuthState>()(
  telemetry((set) => ({
    token: null,
    isAuthenticated: false,
    role: null,
    login: (token: string) => {
      const claims = decodeJwtPayload(token);
      const role = typeof claims.role === "string" ? claims.role : "user";
      // Rotate the anonymous telemetry device id at the start of every session
      // (password login, register, OAuth callback, or silent restore on load).
      // Combined with the reset on logout, this guarantees the "sessions can't
      // be correlated" property holds however the previous session ended —
      // including closing the tab without an explicit logout.
      resetDeviceId();
      // The third arg is a coarse action label consumed by the telemetry
      // middleware — no token or claim content is ever included.
      set({ token, isAuthenticated: true, role }, false, "auth/login");
    },
    logout: () => set({ token: null, isAuthenticated: false, role: null }, false, "auth/logout"),
  })),
);
