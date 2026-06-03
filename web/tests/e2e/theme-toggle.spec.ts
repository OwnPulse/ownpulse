// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Auth note: these E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before
// they reach the proxy, so no real backend or auth session is needed.

async function mockSettingsApis(page: import("@playwright/test").Page) {
  const fakeJwt = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }))}.fake`;
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: fakeJwt, token_type: "bearer", expires_in: 3600 }),
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

  // Mock SSE events endpoint to avoid connection errors.
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
}

test.describe("Settings — theme toggle", () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApis(page);
  });

  // The radio <input>s are visually hidden (opacity:0, 0×0) — the clickable
  // target is the wrapping <label>. Click the label by its text; assert on the
  // radio's checked property (works on hidden inputs), the data-theme attribute,
  // and localStorage, so the test is deterministic and backend-independent.
  test("selecting Dark applies the dark theme and persists it", async ({ page }) => {
    await page.goto("/settings");

    const appearance = page.getByRole("group", { name: "Theme" });
    await expect(appearance).toBeVisible();

    await page.getByText("Dark", { exact: true }).click();

    await expect(page.getByRole("radio", { name: "Dark" })).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    const stored = await page.evaluate(() => localStorage.getItem("theme"));
    expect(stored).toBe("dark");

    // Reload — the dark preference must survive (persistence).
    await page.reload();
    await expect(page.getByRole("radio", { name: "Dark" })).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  test("selecting Light applies the light theme", async ({ page }) => {
    await page.goto("/settings");

    await page.getByText("Light", { exact: true }).click();

    await expect(page.getByRole("radio", { name: "Light" })).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  });

  test("switching back to System clears the stored override", async ({ page }) => {
    await page.goto("/settings");

    await page.getByText("Dark", { exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    await page.getByText("System", { exact: true }).click();

    await expect(page.getByRole("radio", { name: "System" })).toBeChecked();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/);

    const stored = await page.evaluate(() => localStorage.getItem("theme"));
    expect(stored).toBeNull();
  });
});
