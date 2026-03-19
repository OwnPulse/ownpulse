// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { create } from "zustand";

interface AuthState {
  /** JWT stored in memory only — never localStorage. */
  token: string | null;
  /** Whether the user is authenticated. */
  isAuthenticated: boolean;
  /** Set the JWT after login. Refresh token is in an httpOnly cookie (set by backend). */
  login: (token: string) => void;
  /** Clear the JWT and mark as unauthenticated. */
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  token: null,
  isAuthenticated: false,
  login: (token: string) => set({ token, isAuthenticated: true }),
  logout: () => set({ token: null, isAuthenticated: false }),
}));
