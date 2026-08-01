// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useEffect, useState } from "react";
import { refreshTokenOnce } from "../api/refresh";
import { useAuthStore } from "../store/auth";

export function useAuth(): { loading: boolean } {
  const [loading, setLoading] = useState(true);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const login = useAuthStore((s) => s.login);

  useEffect(() => {
    async function init() {
      try {
        const params = new URLSearchParams(window.location.search);
        const token = params.get("token");

        if (token) {
          login(token);
          window.history.replaceState({}, document.title, window.location.pathname);
        } else if (!isAuthenticated) {
          // Single-flight: shares the in-flight refresh with client.ts's
          // 401 retry path so a request that 401s during boot can't rotate
          // the refresh cookie out from under this call (or vice versa).
          await refreshTokenOnce();
        }
      } finally {
        setLoading(false);
      }
    }

    init();
  }, [isAuthenticated, login]);

  return { loading };
}
