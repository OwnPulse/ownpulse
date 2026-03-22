// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { test, expect } from "@playwright/test";

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

    // Mock other endpoints the settings page needs
    await page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );

    await page.goto("/settings");

    await expect(page.getByText("google")).toBeVisible();
    await expect(page.getByText("apple")).toBeVisible();
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

    await page.route("**/api/v1/auth/link/google", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
        ]),
      }),
    );

    await page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );

    // Accept the confirmation dialog
    page.on("dialog", (dialog) => dialog.accept());

    await page.goto("/settings");

    await expect(page.getByRole("button", { name: /unlink google/i })).toBeVisible();
    await page.getByRole("button", { name: /unlink google/i }).click();

    // After unlink, google should be gone and only apple remains
    await expect(page.getByText("apple")).toBeVisible();
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

    await page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );

    await page.goto("/settings");

    await expect(page.getByText("google")).toBeVisible();
    await expect(page.getByRole("button", { name: /unlink/i })).toHaveCount(0);
  });
});
