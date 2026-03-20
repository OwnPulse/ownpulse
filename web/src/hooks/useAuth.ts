// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useEffect, useState } from "react";
import { useAuthStore } from "../store/auth";
import { refreshToken } from "../api/auth";

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
          window.history.replaceState(
            {},
            document.title,
            window.location.pathname,
          );
        } else if (!isAuthenticated) {
          await refreshToken();
        }
      } finally {
        setLoading(false);
      }
    }

    init();
    // Run only on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { loading };
}
