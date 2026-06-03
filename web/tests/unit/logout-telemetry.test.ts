// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { logout } from "../../src/api/auth";
import { isTelemetryEnabled, setTelemetryEnabled, trackPageView } from "../../src/lib/telemetry";
import { useAuthStore } from "../../src/store/auth";

const DEVICE_ID_KEY = "telemetry_device_id";

let telemetryReportCount = 0;
let logoutCalled = false;

const server = setupServer(
  http.post("/api/v1/telemetry/report", async () => {
    telemetryReportCount += 1;
    return HttpResponse.json({ accepted: 1, rejected: 0 });
  }),
  http.post("/api/v1/auth/logout", () => {
    logoutCalled = true;
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

beforeEach(() => {
  localStorage.clear();
  telemetryReportCount = 0;
  logoutCalled = false;
  useAuthStore.getState().login("test-jwt-token");
});

afterEach(() => {
  server.resetHandlers();
  useAuthStore.getState().logout();
  setTelemetryEnabled(false);
  localStorage.clear();
});

describe("logout flow + telemetry", () => {
  it("flushes buffered telemetry and resets the device id on logout", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard"); // creates a device id + a buffered event
    const deviceIdBefore = localStorage.getItem(DEVICE_ID_KEY);
    expect(deviceIdBefore).toBeTruthy();

    await logout();

    // Buffered events were flushed while the token was still valid.
    expect(telemetryReportCount).toBe(1);
    // Backend logout was still called.
    expect(logoutCalled).toBe(true);
    // Device id is cleared so the next session starts fresh.
    expect(localStorage.getItem(DEVICE_ID_KEY)).toBeNull();
    // Auth state is cleared.
    expect(useAuthStore.getState().token).toBeNull();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });

  it("resets the device id even when the backend logout request fails", async () => {
    server.use(http.post("/api/v1/auth/logout", () => new HttpResponse("error", { status: 500 })));
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    expect(localStorage.getItem(DEVICE_ID_KEY)).toBeTruthy();

    await logout();

    expect(localStorage.getItem(DEVICE_ID_KEY)).toBeNull();
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("does not send telemetry on logout when telemetry was never enabled", async () => {
    expect(isTelemetryEnabled()).toBe(false);
    await logout();
    expect(telemetryReportCount).toBe(0);
    expect(logoutCalled).toBe(true);
  });
});

describe("login rotates the device id (anti-correlation across sessions)", () => {
  it("issues a fresh device id on login even when a stale one survived (tab close, no logout)", async () => {
    setTelemetryEnabled(true);

    // Simulate a prior session that ended by closing the tab: the device id is
    // still in localStorage and no logout ran. trackPageView creates the id
    // synchronously, so no flush is needed to observe it.
    trackPageView("/dashboard");
    const staleId = localStorage.getItem(DEVICE_ID_KEY);
    expect(staleId).toBeTruthy();

    // A new login on the same browser must rotate the id so the new session
    // cannot be correlated with the stale one.
    useAuthStore.getState().login("test-jwt-token");
    trackPageView("/dashboard");
    const freshId = localStorage.getItem(DEVICE_ID_KEY);

    expect(freshId).toBeTruthy();
    expect(freshId).not.toBe(staleId);
  });
});
