// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Theme is applied by useTheme hook on mount, reading from localStorage.
// We test on the login page (no auth required) since theme is global.

test.describe("Theme persistence", () => {
  test("dark theme applies on page load", async ({ page }) => {
    // Pre-seed localStorage before navigating
    await page.addInitScript(() => {
      localStorage.setItem("theme", "dark");
    });
    await page.goto("/login");
    await page.waitForLoadState("domcontentloaded");

    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  test("light theme applies on page load", async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem("theme", "light");
    });
    await page.goto("/login");
    await page.waitForLoadState("domcontentloaded");

    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  });

  test("system theme has no data-theme attribute", async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem("theme");
    });
    await page.goto("/login");
    await page.waitForLoadState("domcontentloaded");

    await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  });

  test("theme persists across navigation", async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem("theme", "dark");
    });
    await page.goto("/login");
    await page.waitForLoadState("domcontentloaded");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    // Navigate away and back
    await page.goto("/login");
    await page.waitForLoadState("domcontentloaded");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });
});
