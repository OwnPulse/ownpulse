// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// React Router listener that reports a first-party `page_view` telemetry event
// on every navigation. Only the pathname is sent (query string and hash are
// dropped, and id-shaped path segments are collapsed to `:id` by trackPageView)
// — never search params, which can carry tokens or PII. Emission is gated by
// the opt-in check inside trackPageView.

import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import { trackPageView } from "./telemetry";

/** Track the current route as a page view whenever the pathname changes. */
export function usePageViewTelemetry(): void {
  const { pathname } = useLocation();
  useEffect(() => {
    trackPageView(pathname);
  }, [pathname]);
}
