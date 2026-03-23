// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

const mockUsers = [
  {
    id: "u1",
    email: "admin@example.com",
    username: "admin",
    auth_provider: "password",
    role: "admin",
    status: "active",
    data_region: "us",
    created_at: "2025-01-01T00:00:00Z",
  },
  {
    id: "u2",
    email: "user@example.com",
    auth_provider: "google",
    role: "user",
    status: "active",
    data_region: "us",
    created_at: "2025-06-01T00:00:00Z",
  },
];

const mockInvites = [
  {
    id: "inv1",
    code: "INVITE-ABC",
    label: "For friends",
    max_uses: 10,
    use_count: 3,
    expires_at: null,
    revoked_at: null,
    created_at: "2025-01-01T00:00:00Z",
  },
];

function adminJwt(): string {
  const payload = btoa(JSON.stringify({ sub: "u1", role: "admin", exp: 9999999999 }));
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

async function mockAdminApis(page: import("@playwright/test").Page) {
  const jwt = adminJwt();

  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );

  await page.route("**/api/v1/admin/users", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(mockUsers),
      });
    }
    return route.fallback();
  });

  await page.route("**/api/v1/admin/invites", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(mockInvites),
      });
    }
    return route.fallback();
  });

  // Catch other API calls that Layout or other components might make
  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
}

test.describe("Admin page", () => {
  test("renders users table and invites section", async ({ page }) => {
    await mockAdminApis(page);
    await page.goto("/admin");

    // Verify users table
    await expect(page.getByText("admin@example.com")).toBeVisible();
    await expect(page.getByText("user@example.com")).toBeVisible();

    // Verify invites section
    await expect(page.getByText("INVITE-ABC")).toBeVisible();
    await expect(page.getByText("For friends")).toBeVisible();
  });

  test("create invite flow", async ({ page }) => {
    await mockAdminApis(page);

    // Mock the create invite endpoint
    await page.route("**/api/v1/admin/invites", (route) => {
      if (route.request().method() === "POST") {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            id: "inv-new",
            code: "INVITE-NEW",
            label: "New invite",
            max_uses: 5,
            use_count: 0,
            expires_at: null,
            revoked_at: null,
            created_at: "2026-03-22T00:00:00Z",
          }),
        });
      }
      // GET returns original list plus new invite after creation
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          ...mockInvites,
          {
            id: "inv-new",
            code: "INVITE-NEW",
            label: "New invite",
            max_uses: 5,
            use_count: 0,
            expires_at: null,
            revoked_at: null,
            created_at: "2026-03-22T00:00:00Z",
          },
        ]),
      });
    });

    await page.goto("/admin");

    // Wait for page to load
    await expect(page.getByText("INVITE-ABC")).toBeVisible();

    // Click Create Invite to show form
    await page.getByRole("button", { name: /create invite/i }).click();

    // Fill in the form
    await page.getByLabel(/label/i).fill("New invite");
    await page.getByLabel(/max uses/i).fill("5");

    // Submit
    await page.getByRole("button", { name: /^create$/i }).click();

    // After creation, the new invite should appear (the GET refetch returns it)
    await expect(page.getByText("INVITE-NEW")).toBeVisible();
  });

  test("revoke invite flow", async ({ page }) => {
    await mockAdminApis(page);

    const revokedInvite = { ...mockInvites[0], revoked_at: "2026-03-22T00:00:00Z" };

    // Mock the revoke endpoint
    await page.route("**/api/v1/admin/invites/inv1", (route) => {
      if (route.request().method() === "DELETE") {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(revokedInvite),
        });
      }
      return route.fallback();
    });

    // After revoke, the GET re-fetch returns the revoked invite
    let revoked = false;
    await page.route("**/api/v1/admin/invites", (route) => {
      if (route.request().method() === "GET") {
        if (revoked) {
          return route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify([revokedInvite]),
          });
        }
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(mockInvites),
        });
      }
      return route.fallback();
    });

    await page.goto("/admin");

    // Wait for invite to appear
    await expect(page.getByText("INVITE-ABC")).toBeVisible();
    await expect(page.getByRole("button", { name: /revoke/i })).toBeVisible();

    // Click Revoke
    revoked = true;
    await page.getByRole("button", { name: /revoke/i }).click();

    // After revocation, the Revoke button should disappear (invite is no longer active)
    await expect(page.getByRole("button", { name: /revoke/i })).not.toBeVisible();
  });

  test("displays user action buttons for non-self users only", async ({ page }) => {
    await mockAdminApis(page);
    await page.goto("/admin");

    // Wait for users to load
    await expect(page.getByText("user@example.com")).toBeVisible();

    // u2 should have Disable and Delete buttons
    await expect(page.getByRole("button", { name: /disable/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /delete/i })).toBeVisible();

    // Only one Delete button (for u2, not u1 which is self)
    const deleteButtons = page.getByRole("button", { name: /delete/i });
    await expect(deleteButtons).toHaveCount(1);
  });
});
