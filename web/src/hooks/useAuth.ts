// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useEffect, useState } from "react";
import { refreshTokenOnce } from "../api/refresh";
import { useAuthStore } from "../store/auth";

export function useAuth(): { loading: boolean } {
  const [loading, setLoading] = useState(true);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);

  useEffect(() => {
    async function init() {
      try {
        if (!isAuthenticated) {
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
  }, [isAuthenticated]);

  return { loading };
}
