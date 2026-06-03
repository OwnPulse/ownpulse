// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { __bufferLength, setTelemetryEnabled } from "../../src/lib/telemetry";
import { usePageViewTelemetry } from "../../src/lib/usePageViewTelemetry";
import { useAuthStore } from "../../src/store/auth";

function Probe() {
  usePageViewTelemetry();
  return null;
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  setTelemetryEnabled(false);
  useAuthStore.getState().logout();
  localStorage.clear();
});

describe("usePageViewTelemetry auth scoping", () => {
  it("does not track page views while unauthenticated", () => {
    useAuthStore.getState().logout();
    setTelemetryEnabled(true);
    renderAt("/login");
    expect(__bufferLength()).toBe(0);
  });

  it("does not track public routes even with a token-like path", () => {
    useAuthStore.getState().logout();
    setTelemetryEnabled(true);
    renderAt("/invite/SOME-CODE");
    expect(__bufferLength()).toBe(0);
  });

  it("tracks a page view once authenticated", () => {
    // Authenticate first, then enable telemetry, so the login action itself
    // isn't buffered — this isolates the page-view event under test.
    useAuthStore.getState().login("test-jwt-token");
    setTelemetryEnabled(true);
    renderAt("/dashboard");
    expect(__bufferLength()).toBe(1);
  });
});
