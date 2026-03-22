// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { test, expect } from "@playwright/test";

// Auth note: These E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before
// they reach the proxy, so no real backend or auth session is needed.
// If route guards are added in the future, a beforeEach that sets an auth
// cookie/token via page.context().addCookies() or storageState will be needed.

function mockSettingsApis(page: import("@playwright/test").Page) {
  return Promise.all([
    page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    ),
  ]);
}

test.describe("Apple Sign-In button", () => {
  test("renders on login page with correct href", async ({ page }) => {
    await page.goto("/login");
    const appleLink = page.getByRole("link", { name: /sign in with apple/i });
    await expect(appleLink).toBeVisible();
    await expect(appleLink).toHaveAttribute("href", "/api/v1/auth/apple/login");
  });
});

test.describe("Linked Accounts", () => {
  test("displays auth methods from the API", async ({ page }) => {
    await page.route("**/api/v1/auth/methods", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
          { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
        ]),
      }),
    );

    await mockSettingsApis(page);

    await page.goto("/settings");

    await expect(page.getByText("Google")).toBeVisible();
    await expect(page.getByText("Apple")).toBeVisible();
    await expect(page.getByText("user@example.com")).toBeVisible();
  });

  test("unlink flow removes method from the list", async ({ page }) => {
    await page.route("**/api/v1/auth/methods", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
          { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
        ]),
      }),
    );

    await page.route("**/api/v1/auth/link/google", (route) => {
      if (route.request().method() === "DELETE") {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([
            { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
          ]),
        });
      }
      return route.fulfill({ status: 405, body: "Method Not Allowed" });
    });

    await mockSettingsApis(page);

    // Accept the confirmation dialog
    page.on("dialog", (dialog) => dialog.accept());

    await page.goto("/settings");

    await expect(page.getByRole("button", { name: /unlink google/i })).toBeVisible();
    await page.getByRole("button", { name: /unlink google/i }).click();

    // After unlink, google should be gone and only apple remains
    await expect(page.getByText("Apple")).toBeVisible();
    await expect(page.getByRole("button", { name: /unlink google/i })).not.toBeVisible();
  });

  test("shows error message when unlink fails", async ({ page }) => {
    await page.route("**/api/v1/auth/methods", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
          { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
        ]),
      }),
    );

    await page.route("**/api/v1/auth/link/google", (route) =>
      route.fulfill({ status: 500, contentType: "text/plain", body: "Internal Server Error" }),
    );

    await mockSettingsApis(page);

    page.on("dialog", (dialog) => dialog.accept());

    await page.goto("/settings");

    await expect(page.getByRole("button", { name: /unlink google/i })).toBeVisible();
    await page.getByRole("button", { name: /unlink google/i }).click();

    await expect(page.getByText("Internal Server Error")).toBeVisible();
  });

  test("hides unlink button when only one method exists", async ({ page }) => {
    await page.route("**/api/v1/auth/methods", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
        ]),
      }),
    );

    await mockSettingsApis(page);

    await page.goto("/settings");

    await expect(page.getByText("Google")).toBeVisible();
    await expect(page.getByRole("button", { name: /unlink/i })).toHaveCount(0);
  });
});
