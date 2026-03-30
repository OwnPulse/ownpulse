// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

function fakeJwt(sub: string, role: string): string {
  const payload = btoa(JSON.stringify({ sub, role, exp: 9999999999 }));
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

async function mockAuthRefresh(page: import("@playwright/test").Page, jwt: string) {
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );
}

test.describe("Registration flow", () => {
  test("registers with valid invite code and redirects to dashboard", async ({ page }) => {
    const jwt = fakeJwt("new-user-id", "user");

    // Mock the register endpoint to succeed
    await page.route("**/api/v1/auth/register", async (route) => {
      const body = JSON.parse(route.request().postData() || "{}");
      expect(body.email).toBe("newuser@example.com");
      expect(body.password).toBe("securepassword123");
      expect(body.invite_code).toBe("TEST-CODE");

      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
      });
    });

    // Mock the refresh endpoint for after-login navigation
    await mockAuthRefresh(page, jwt);

    // Mock any API calls the dashboard might make after redirect
    await page.route("**/api/v1/**", (route) => {
      const url = route.request().url();
      if (url.includes("/auth/register") || url.includes("/auth/refresh")) {
        // Already handled above
        return route.fallback();
      }
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: "[]",
      });
    });

    await page.goto("/register?invite=TEST-CODE");

    // Verify the invite code is pre-filled
    const inviteInput = page.getByLabel(/invite code/i);
    await expect(inviteInput).toHaveValue("TEST-CODE");

    // Fill in the registration form
    await page.getByLabel(/email/i).fill("newuser@example.com");
    await page.getByLabel(/^password$/i).fill("securepassword123");
    await page.getByLabel(/confirm password/i).fill("securepassword123");

    // Submit the form
    await page.getByRole("button", { name: /create account/i }).click();

    // Should redirect to dashboard (root path)
    await page.waitForURL("/");
  });

  test("shows error when registration fails with invalid invite", async ({ page }) => {
    // Mock the register endpoint to fail
    await page.route("**/api/v1/auth/register", (route) =>
      route.fulfill({
        status: 422,
        contentType: "text/plain",
        body: "Invalid or expired invite code",
      }),
    );

    await page.goto("/register?invite=INVALID-CODE");

    // Fill in the form
    await page.getByLabel(/email/i).fill("newuser@example.com");
    await page.getByLabel(/^password$/i).fill("securepassword123");
    await page.getByLabel(/confirm password/i).fill("securepassword123");

    // Submit
    await page.getByRole("button", { name: /create account/i }).click();

    // Should show error message
    await expect(
      page.getByText(/registration failed.*invite code may be invalid or expired/i),
    ).toBeVisible();
  });

  test("shows message when no invite code is provided", async ({ page }) => {
    await page.goto("/register");

    await expect(page.getByText(/you need an invite code to sign up/i)).toBeVisible();
    await expect(page.getByText(/already have an account\? sign in/i)).toBeVisible();
  });

  test("validates password length before submitting", async ({ page }) => {
    await page.goto("/register?invite=TEST-CODE");

    await page.getByLabel(/email/i).fill("newuser@example.com");
    await page.getByLabel(/^password$/i).fill("short");
    await page.getByLabel(/confirm password/i).fill("short");

    await page.getByRole("button", { name: /create account/i }).click();

    await expect(page.getByText(/password must be at least 10 characters/i)).toBeVisible();
  });

  test("validates password confirmation match", async ({ page }) => {
    await page.goto("/register?invite=TEST-CODE");

    await page.getByLabel(/email/i).fill("newuser@example.com");
    await page.getByLabel(/^password$/i).fill("securepassword123");
    await page.getByLabel(/confirm password/i).fill("differentpassword");

    await page.getByRole("button", { name: /create account/i }).click();

    await expect(page.getByText(/passwords do not match/i)).toBeVisible();
  });
});
