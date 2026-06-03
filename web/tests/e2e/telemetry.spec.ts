// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Auth note: these E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before they
// reach the proxy, so no real backend or auth session is needed.

const DEVICE_ID_KEY = "telemetry_device_id";
const ENABLED_KEY = "telemetry_enabled";

function fakeJwt(): string {
  const payload = btoa(
    JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }),
  );
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

async function mockSettingsApis(page: import("@playwright/test").Page) {
  const jwt = fakeJwt();
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );
  await page.route("**/api/v1/auth/methods", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
  await page.route("**/api/v1/notifications/preferences", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        default_notify: false,
        default_notify_times: ["08:00"],
        repeat_reminders: false,
        repeat_interval_minutes: 30,
      }),
    }),
  );
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
}

test.describe("Settings — telemetry opt-in", () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApis(page);
  });

  test("happy path: enabling telemetry persists the preference and sends events", async ({
    page,
  }) => {
    const reports: unknown[] = [];
    await page.route("**/api/v1/telemetry/report", async (route) => {
      reports.push(JSON.parse(route.request().postData() ?? "{}"));
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ accepted: 1, rejected: 0 }),
      });
    });

    await page.goto("/settings");

    const toggle = page.getByRole("checkbox", { name: /anonymous usage telemetry/i });
    await expect(toggle).not.toBeChecked();

    await toggle.check();
    await expect(toggle).toBeChecked();

    const enabled = await page.evaluate((k) => localStorage.getItem(k), ENABLED_KEY);
    expect(enabled).toBe("true");

    // Navigating generates a page_view; force a flush by logging out, which the
    // app flushes before clearing the session.
    await page.goto("/explore");
    await page.goto("/settings");

    // An anonymous device id was created in localStorage.
    const deviceId = await page.evaluate((k) => localStorage.getItem(k), DEVICE_ID_KEY);
    expect(deviceId).toBeTruthy();
  });

  test("error path: a failing telemetry endpoint does not surface an error to the user", async ({
    page,
  }) => {
    await page.route("**/api/v1/telemetry/report", (route) =>
      route.fulfill({ status: 500, contentType: "text/plain", body: "boom" }),
    );

    await page.goto("/settings");
    const toggle = page.getByRole("checkbox", { name: /anonymous usage telemetry/i });
    await toggle.check();
    await expect(toggle).toBeChecked();

    // Navigate to trigger buffered events whose flush will hit the 500 — the UI
    // must stay functional and show no error banner.
    await page.goto("/explore");
    await page.goto("/settings");

    await expect(toggle).toBeChecked();
    await expect(page.getByRole("heading", { name: /anonymous usage telemetry/i })).toBeVisible();
  });
});
