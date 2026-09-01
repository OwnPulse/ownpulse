// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Exercises the coalesced 401 refresh-and-retry flow end to end: a data call
// that 401s mid-session should transparently refresh and retry rather than
// bouncing the user to the login screen, and a refresh that itself fails
// should still send them there.

function fakeJwt(): string {
  const payload = btoa(
    JSON.stringify({
      sub: "00000000-0000-0000-0000-000000000001",
      role: "user",
      exp: 9999999999,
    }),
  );
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

const DASHBOARD_SUMMARY = {
  latest_checkin: { date: "2026-06-01", energy: 7, mood: 6, focus: 8, recovery: 5, libido: 6 }, // date-ok
  checkin_count_7d: 5,
  health_record_count_7d: 12,
  intervention_count_7d: 3,
  observation_count_7d: 8,
  latest_lab_date: "2026-05-20", // date-ok
  pending_friend_shares: 0,
};

test("expired access token: dashboard call 401s, refreshes transparently, and renders data", async ({
  page,
}) => {
  let summaryCalls = 0;

  // `?token=` on first load bypasses the boot-time refresh (see useAuth) so
  // the session starts authenticated without touching /auth/refresh — the
  // only refresh in this test is the one client.ts triggers on the 401 below.
  await page.route("**/api/v1/dashboard/summary", (route) => {
    summaryCalls += 1;
    if (summaryCalls === 1) {
      return route.fulfill({ status: 401, contentType: "text/plain", body: "Unauthorized" });
    }
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(DASHBOARD_SUMMARY),
    });
  });

  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: fakeJwt(), token_type: "bearer", expires_in: 3600 }),
    }),
  );
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
  await page.route("**/api/v1/**", (route) => {
    const url = route.request().url();
    if (url.includes("/auth/") || url.includes("/dashboard/summary") || url.includes("/events")) {
      return route.fallback();
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
  });

  await page.goto(`/?token=${fakeJwt()}`);

  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
  // The 401'd request's data still shows up once the retry succeeds — the
  // user never saw an error state or a login redirect.
  await expect(page.getByText("Health Records (7 days)")).toBeVisible();
  await expect(page).toHaveURL(/\/$|\/dashboard/);
  expect(summaryCalls).toBeGreaterThanOrEqual(2);
});

test("expired access token: refresh itself fails, user is redirected to login", async ({
  page,
}) => {
  await page.route("**/api/v1/dashboard/summary", (route) =>
    route.fulfill({ status: 401, contentType: "text/plain", body: "Unauthorized" }),
  );
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({ status: 401, contentType: "text/plain", body: "Unauthorized" }),
  );
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
  await page.route("**/api/v1/**", (route) => {
    const url = route.request().url();
    if (url.includes("/auth/") || url.includes("/dashboard/summary") || url.includes("/events")) {
      return route.fallback();
    }
    return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
  });

  await page.goto(`/?token=${fakeJwt()}`);

  await expect(page).toHaveURL(/\/login/);
});
