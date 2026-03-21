// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { create } from "zustand";

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

export const useAuthStore = create<AuthState>((set) => ({
  token: null,
  isAuthenticated: false,
  role: null,
  login: (token: string) => {
    const claims = decodeJwtPayload(token);
    const role = typeof claims.role === "string" ? claims.role : "user";
    set({ token, isAuthenticated: true, role });
  },
  logout: () => set({ token: null, isAuthenticated: false, role: null }),
}));
