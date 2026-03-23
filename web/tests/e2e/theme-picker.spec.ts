// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

async function mockApis(page: import("@playwright/test").Page) {
  const fakeJwt = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }))}.fake`;
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: fakeJwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );
  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
  await page.route("**/api/v1/auth/methods", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
}

test.describe("Theme picker", () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
  });

  test("selects dark theme and persists across navigation", async ({ page }) => {
    await page.goto("/settings");

    // Default is system
    const darkRadio = page.getByRole("radio", { name: "Dark" });
    const systemRadio = page.getByRole("radio", { name: "System" });
    await expect(systemRadio).toBeChecked();

    // Select dark
    await darkRadio.click();
    await expect(darkRadio).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    // Navigate away and back
    await page.goto("/");
    await page.goto("/settings");
    await expect(page.getByRole("radio", { name: "Dark" })).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  test("switching to system clears data-theme", async ({ page }) => {
    // Pre-set dark in localStorage
    await page.goto("/settings");
    await page.evaluate(() => {
      localStorage.setItem("theme", "dark");
      document.documentElement.dataset.theme = "dark";
    });
    await page.reload();

    await expect(page.getByRole("radio", { name: "Dark" })).toBeChecked();

    // Switch to system
    await page.getByRole("radio", { name: "System" }).click();
    await expect(page.getByRole("radio", { name: "System" })).toBeChecked();
    await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  });
});
