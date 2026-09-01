// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Auth note: these E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before
// they reach the proxy, so no real backend or auth session is needed.

async function mockDashboardApis(page: import("@playwright/test").Page) {
  const fakeJwt = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }))}.fake`;
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: fakeJwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
  await page.route("**/api/v1/dashboard/summary", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        latest_checkin: null,
        checkin_count_7d: 0,
        health_record_count_7d: 0,
        intervention_count_7d: 0,
        observation_count_7d: 0,
        latest_lab_date: null,
        pending_friend_shares: 0,
      }),
    }),
  );
  await page.route("**/api/v1/protocols/runs/todays-doses", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
}

const missedItem = {
  protocol_id: "p1",
  protocol_name: "BPC Stack",
  run_id: "run-1",
  protocol_line_id: "pl-3",
  substance: "BPC-157",
  dose: 250,
  unit: "mcg",
  route: "SubQ",
  time_of_day: "08:00",
  day_number: 2,
  date: "2026-03-27",
  status: "missed",
};

test.describe("Dashboard — missed doses review", () => {
  test.beforeEach(async ({ page }) => {
    await mockDashboardApis(page);
  });

  test("reviews and logs a missed dose from the dashboard", async ({ page }) => {
    let missed = [missedItem];
    let loggedBody: unknown;

    await page.route("**/api/v1/protocols/runs/missed-doses", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(missed) }),
    );
    await page.route("**/api/v1/protocols/runs/run-1/doses/log", async (route) => {
      loggedBody = JSON.parse(route.request().postData() ?? "{}");
      missed = [];
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "dose-backfill",
          protocol_line_id: "pl-3",
          day_number: 2,
          status: "completed",
          intervention_id: "iv-2",
          logged_at: "2026-03-27T08:00:00Z",
          run_id: "run-1",
          skip_reason: null,
        }),
      });
    });

    await page.goto("/");

    const reviewToggle = page.getByRole("button", {
      name: "1 missed dose from earlier days — Review",
    });
    await expect(reviewToggle).toBeVisible();
    await reviewToggle.click();

    await expect(page.getByText("2026-03-27")).toBeVisible();
    await page.getByRole("button", { name: "Log", exact: true }).click();

    await expect(reviewToggle).not.toBeVisible();
    expect(loggedBody).toMatchObject({ protocol_line_id: "pl-3", day_number: 2 });
  });

  test("keeps the missed item visible when logging fails (error path)", async ({ page }) => {
    await page.route("**/api/v1/protocols/runs/missed-doses", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([missedItem]),
      }),
    );
    await page.route("**/api/v1/protocols/runs/run-1/doses/log", (route) =>
      route.fulfill({ status: 500, body: "Server error" }),
    );

    await page.goto("/");

    const reviewToggle = page.getByRole("button", {
      name: "1 missed dose from earlier days — Review",
    });
    await expect(reviewToggle).toBeVisible();
    await reviewToggle.click();

    await expect(page.getByText("2026-03-27")).toBeVisible();
    await page.getByRole("button", { name: "Log", exact: true }).click();

    // The write failed — the item must still be there, not optimistically
    // removed.
    await expect(page.getByText("2026-03-27")).toBeVisible();
    await expect(reviewToggle).toBeVisible();
  });
});
