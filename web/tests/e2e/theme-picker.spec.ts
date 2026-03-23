// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Theme picker E2E: tests the useTheme hook behavior at the browser level.
// We test on the login page (no auth required) since theme is applied globally.
// The Settings radio UI is covered by unit/integration tests.

test.describe("Theme persistence", () => {
  test("dark theme persists across page loads", async ({ page }) => {
    await page.goto("/login");

    // Set dark theme via localStorage (same as the Settings picker does)
    await page.evaluate(() => {
      localStorage.setItem("theme", "dark");
    });
    await page.reload();

    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  test("light theme persists across page loads", async ({ page }) => {
    await page.goto("/login");

    await page.evaluate(() => {
      localStorage.setItem("theme", "light");
    });
    await page.reload();

    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  });

  test("system theme has no data-theme attribute", async ({ page }) => {
    await page.goto("/login");

    // Ensure no theme is stored (system default)
    await page.evaluate(() => {
      localStorage.removeItem("theme");
    });
    await page.reload();

    await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  });

  test("switching from dark to system removes data-theme", async ({ page }) => {
    await page.goto("/login");

    // Start with dark
    await page.evaluate(() => {
      localStorage.setItem("theme", "dark");
      document.documentElement.dataset.theme = "dark";
    });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    // Switch to system
    await page.evaluate(() => {
      localStorage.removeItem("theme");
      delete document.documentElement.dataset.theme;
    });
    await expect(page.locator("html")).not.toHaveAttribute("data-theme");

    // Persists after reload
    await page.reload();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  });
});
