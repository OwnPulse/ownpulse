// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Auth note: these E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before
// they reach the proxy, so no real backend or auth session is needed.

async function mockShellApis(page: import("@playwright/test").Page) {
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
}

const protocol = {
  id: "proto-1",
  user_id: "user-1",
  name: "BPC-157 Stack",
  description: "Healing protocol",
  status: "active",
  start_date: "2026-03-01",
  duration_days: 5,
  share_token: null,
  created_at: "2026-03-01T00:00:00Z",
  updated_at: "2026-03-01T00:00:00Z",
  lines: [
    {
      id: "line-1",
      protocol_id: "proto-1",
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      schedule_pattern: [true, true, true, true, true],
      sort_order: 0,
      doses: [],
    },
  ],
};

const activeRun = {
  id: "run-1",
  protocol_id: "proto-1",
  protocol_name: "BPC-157 Stack",
  user_id: "user-1",
  start_date: "2026-03-01",
  duration_days: 5,
  status: "active",
  notify: false,
  notify_times: [],
  repeat_reminders: false,
  repeat_interval_minutes: 30,
  progress_pct: 40,
  doses_today: 1,
  doses_completed_today: 0,
  adherence_pct: null,
  doses_missed: null,
  created_at: "2026-03-01T10:00:00Z",
};

function runDoseItem(day: number, status: string) {
  return {
    day_number: day,
    date: `2026-03-0${day + 1}`,
    protocol_line_id: "line-1",
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    time_of_day: "AM",
    status,
    dose_id: status === "completed" || status === "skipped" ? `dose-${day}` : null,
    intervention_id: null,
    skip_reason: null,
    logged_at: null,
  };
}

test.describe("Protocol dose backfill via the schedule grid", () => {
  test.beforeEach(async ({ page }) => {
    await mockShellApis(page);
    await page.route("**/api/v1/protocols/proto-1", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(protocol),
      }),
    );
    await page.route("**/api/v1/protocols/proto-1/runs", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([activeRun]),
      }),
    );
    await page.route("**/api/v1/protocols/runs/run-1/adherence", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          run_id: "run-1",
          scheduled_so_far: 2,
          completed: 1,
          skipped: 0,
          missed: 1,
          adherence_pct: 50,
          lines: [],
        }),
      }),
    );
  });

  test("backfills a missed day by logging it from the grid", async ({ page }) => {
    let day1Status = "missed";
    let loggedBody: unknown;

    await page.route("**/api/v1/protocols/runs/run-1/doses*", (route) => {
      const url = new URL(route.request().url());
      if (url.pathname.endsWith("/doses")) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([
            runDoseItem(0, "completed"),
            runDoseItem(1, day1Status),
            runDoseItem(2, "pending"),
            runDoseItem(3, "pending"),
            runDoseItem(4, "pending"),
          ]),
        });
      }
      return route.continue();
    });

    await page.route("**/api/v1/protocols/runs/run-1/doses/log", async (route) => {
      loggedBody = JSON.parse(route.request().postData() ?? "{}");
      day1Status = "completed";
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "dose-1",
          protocol_line_id: "line-1",
          day_number: 1,
          status: "completed",
          intervention_id: "iv-1",
          logged_at: "2026-03-02T09:00:00Z",
          run_id: "run-1",
          skip_reason: null,
        }),
      });
    });

    await page.goto("/protocols/proto-1");

    await expect(page.getByRole("heading", { name: "BPC-157 Stack" })).toBeVisible();
    const missedCell = page.getByRole("button", { name: "Day 2, missed — log or skip" });
    await expect(missedCell).toBeVisible();

    await missedCell.click();
    await expect(page.getByRole("dialog", { name: "Log day 2" })).toBeVisible();

    await page.getByRole("button", { name: "Log", exact: true }).click();

    await expect(page.getByRole("button", { name: "Day 2, completed — undo" })).toBeVisible();
    expect(loggedBody).toMatchObject({ protocol_line_id: "line-1", day_number: 1 });
  });

  test("undo requires a confirming second click before it deletes the dose", async ({ page }) => {
    let deleteCalled = false;

    await page.route("**/api/v1/protocols/runs/run-1/doses*", (route) => {
      const url = new URL(route.request().url());
      if (url.pathname.endsWith("/doses")) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([
            runDoseItem(0, "completed"),
            runDoseItem(1, "pending"),
            runDoseItem(2, "pending"),
            runDoseItem(3, "pending"),
            runDoseItem(4, "pending"),
          ]),
        });
      }
      return route.continue();
    });

    await page.route("**/api/v1/protocols/runs/run-1/doses/dose-0", (route) => {
      deleteCalled = true;
      return route.fulfill({ status: 204 });
    });

    await page.goto("/protocols/proto-1");

    const completedCell = page.getByRole("button", { name: "Day 1, completed — undo" });
    await expect(completedCell).toBeVisible();

    await completedCell.click();
    await expect(
      page.getByRole("button", { name: "Day 1, completed — confirm undo" }),
    ).toBeVisible();
    expect(deleteCalled).toBe(false);

    await page.getByRole("button", { name: "Day 1, completed — confirm undo" }).click();
    await expect(async () => expect(deleteCalled).toBe(true)).toPass();
  });

  test("shows a visible error and keeps the popover open after a failed log attempt", async ({
    page,
  }) => {
    await page.route("**/api/v1/protocols/runs/run-1/doses*", (route) => {
      const url = new URL(route.request().url());
      if (url.pathname.endsWith("/doses")) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify([
            runDoseItem(0, "completed"),
            runDoseItem(1, "missed"),
            runDoseItem(2, "pending"),
            runDoseItem(3, "pending"),
            runDoseItem(4, "pending"),
          ]),
        });
      }
      return route.continue();
    });

    await page.route("**/api/v1/protocols/runs/run-1/doses/log", (route) =>
      route.fulfill({ status: 400, body: "day 1 (2026-03-02) hasn't happened yet" }),
    );

    await page.goto("/protocols/proto-1");

    const missedCell = page.getByRole("button", { name: "Day 2, missed — log or skip" });
    await expect(missedCell).toBeVisible();
    await missedCell.click();

    await page.getByRole("button", { name: "Log", exact: true }).click();

    // The mutation failed — the cell must still show "missed" (no false
    // success), the popover stays open, and the failure is visible (not
    // silently swallowed) so the user knows to retry or investigate.
    await expect(page.getByRole("dialog", { name: "Log day 2" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Day 2, missed — log or skip" })).toBeVisible();
    await expect(page.getByRole("alert")).toContainText("hasn't happened yet");
  });
});
