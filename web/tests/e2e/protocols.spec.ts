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

test("create protocol page loads", async ({ page }) => {
  await mockProtocolApis(page);
  await page.goto("/protocols/new");

  // Verify the builder page renders with key elements
  await expect(page.getByRole("button", { name: /create protocol/i })).toBeVisible();
});
