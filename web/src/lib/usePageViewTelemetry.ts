// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// React Router listener that reports a first-party `page_view` telemetry event
// on navigation. Only the pathname is sent (query string and hash are dropped,
// and id-shaped path segments are collapsed to `:id` by trackPageView) — never
// search params, which can carry tokens or PII.
//
// Collection is scoped structurally to authenticated sessions: navigations on
// public/unauthenticated routes (login, register, invite, shared-protocol) are
// never tracked, regardless of opt-in state. This bounds page-view collection
// to logged-in usage rather than relying on the flush-time JWT check alone.
// Emission is additionally gated by the opt-in check inside trackPageView.

import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import { useAuthStore } from "../store/auth";
import { trackPageView } from "./telemetry";

/** Track the current route as a page view whenever the pathname changes, but
 * only while the user is authenticated. */
export function usePageViewTelemetry(): void {
  const { pathname } = useLocation();
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  useEffect(() => {
    if (!isAuthenticated) return;
    trackPageView(pathname);
  }, [pathname, isAuthenticated]);
}
