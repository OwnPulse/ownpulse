// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

function userJwt(): string {
  const payload = btoa(JSON.stringify({ sub: "u1", role: "user", exp: 9999999999 }));
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

const mockProtocols = [
  {
    id: "p1",
    name: "Test Protocol",
    status: "active",
    start_date: "2026-03-01",
    duration_days: 14,
    created_at: "2026-03-01T00:00:00Z",
    lines: [],
  },
];

async function mockProtocolApis(page: import("@playwright/test").Page) {
  const jwt = userJwt();

  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );

  await page.route("**/api/v1/protocols", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(mockProtocols),
      });
    }
    return route.fallback();
  });

  await page.route("**/api/v1/protocols/templates", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );

  await page.route("**/api/v1/protocols/today", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );

  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );

  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
}

test("protocols page loads", async ({ page }) => {
  await mockProtocolApis(page);
  await page.goto("/protocols");

  await expect(page.getByRole("link", { name: /new protocol/i })).toBeVisible();
});

test("create protocol flow", async ({ page }) => {
  await mockProtocolApis(page);

  // Mock the create endpoint
  await page.route("**/api/v1/protocols", (route) => {
    if (route.request().method() === "POST") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "p-new",
          name: "New Protocol",
          status: "active",
          start_date: "2026-04-01",
          duration_days: 7,
          created_at: "2026-03-27T00:00:00Z",
          updated_at: "2026-03-27T00:00:00Z",
          lines: [],
        }),
      });
    }
    return route.fallback();
  });

  // Mock the protocol view endpoint
  await page.route("**/api/v1/protocols/p-new", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: "p-new",
        name: "New Protocol",
        description: null,
        status: "active",
        start_date: "2026-04-01",
        duration_days: 7,
        share_token: null,
        created_at: "2026-03-27T00:00:00Z",
        updated_at: "2026-03-27T00:00:00Z",
        lines: [],
      }),
    }),
  );

  await page.goto("/protocols/new");

  // Fill the form
  await page.getByLabel(/protocol name/i).fill("New Protocol");
  await page.getByLabel(/duration/i).fill("7");

  // Submit
  await page.getByRole("button", { name: /create protocol/i }).click();

  // Should navigate to the protocol view
  await page.waitForURL("**/protocols/p-new");
});
