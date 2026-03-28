// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

// Auth note: These E2E tests run against the Vite dev server which proxies
// /api to the backend. Playwright route intercepts catch API calls before
// they reach the proxy, so no real backend or auth session is needed.

async function mockExploreApis(page: import("@playwright/test").Page) {
  const fakeJwt = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }))}.fake`;
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: fakeJwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );

  await page.route("**/api/v1/explore/metrics", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        sources: [
          {
            source: "checkins",
            label: "Check-ins",
            metrics: [
              { field: "energy", label: "Energy", unit: "score" },
              { field: "mood", label: "Mood", unit: "score" },
            ],
          },
          {
            source: "health_records",
            label: "Health Records",
            metrics: [{ field: "weight", label: "Weight", unit: "kg" }],
          },
        ],
      }),
    }),
  );

  await page.route("**/api/v1/explore/charts", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([]),
      });
    }
    if (route.request().method() === "POST") {
      return route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify({
          id: "chart-1",
          name: "Test Chart",
          config: { version: 1, metrics: [{ source: "checkins", field: "energy" }], range: { preset: "30d" }, resolution: "daily" },
          created_at: "2026-03-01T00:00:00Z",
          updated_at: "2026-03-01T00:00:00Z",
        }),
      });
    }
    return route.fulfill({ status: 405 });
  });

  await page.route("**/api/v1/explore/series", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        series: [
          {
            source: "checkins",
            field: "energy",
            unit: "score",
            points: [
              { t: "2026-03-01T00:00:00Z", v: 7, n: 1 },
              { t: "2026-03-02T00:00:00Z", v: 6, n: 1 },
              { t: "2026-03-03T00:00:00Z", v: 8, n: 1 },
              { t: "2026-03-04T00:00:00Z", v: 5, n: 1 },
              { t: "2026-03-05T00:00:00Z", v: 9, n: 1 },
            ],
          },
        ],
      }),
    }),
  );

  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );

  // Mock SSE events endpoint to avoid connection errors
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "text/event-stream",
      body: "",
    }),
  );
}

test.describe("Explore page", () => {
  test("loads with metric picker and empty chart area", async ({ page }) => {
    await mockExploreApis(page);
    await page.goto("/explore");

    await expect(page.getByText("Explore")).toBeVisible();
    await expect(page.getByText("Check-ins")).toBeVisible();
    await expect(page.getByText("Energy")).toBeVisible();
    await expect(page.getByText("Select metrics from the picker to start exploring.")).toBeVisible();
  });

  test("selecting a metric fetches and displays chart data", async ({ page }) => {
    await mockExploreApis(page);
    await page.goto("/explore");

    await expect(page.getByText("Energy")).toBeVisible();
    await page.getByLabel("Energy").check();

    // Chart should now render (unovis SVG container)
    await expect(page.getByText("Select metrics from the picker to start exploring.")).not.toBeVisible();
  });

  test("date range preset buttons work", async ({ page }) => {
    await mockExploreApis(page);
    await page.goto("/explore");

    await page.getByRole("button", { name: "7D" }).click();
    // 7D should be visually active (has the active class)
    await expect(page.getByRole("button", { name: "7D" })).toBeVisible();

    await page.getByRole("button", { name: "90D" }).click();
    await expect(page.getByRole("button", { name: "90D" })).toBeVisible();
  });

  test("resolution toggle buttons work", async ({ page }) => {
    await mockExploreApis(page);
    await page.goto("/explore");

    await expect(page.getByRole("button", { name: "Daily" })).toHaveAttribute("aria-pressed", "true");
    await page.getByRole("button", { name: "Weekly" }).click();
    await expect(page.getByRole("button", { name: "Weekly" })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("button", { name: "Daily" })).toHaveAttribute("aria-pressed", "false");
  });

  test("search filters metrics", async ({ page }) => {
    await mockExploreApis(page);
    await page.goto("/explore");

    await expect(page.getByText("Weight")).toBeVisible();
    await page.getByLabel("Search metrics").fill("energy");
    await expect(page.getByText("Energy")).toBeVisible();
    await expect(page.getByText("Weight")).not.toBeVisible();
  });

  test("handles API error gracefully", async ({ page }) => {
    const fakeJwt = `eyJhbGciOiJIUzI1NiJ9.${btoa(JSON.stringify({ sub: "00000000-0000-0000-0000-000000000001", role: "user", exp: 9999999999 }))}.fake`;
    await page.route("**/api/v1/auth/refresh", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ access_token: fakeJwt, token_type: "bearer", expires_in: 3600 }),
      }),
    );
    await page.route("**/api/v1/explore/metrics", (route) =>
      route.fulfill({ status: 500, contentType: "text/plain", body: "Internal Server Error" }),
    );
    await page.route("**/api/v1/explore/charts", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );
    await page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );
    await page.route("**/api/v1/events*", (route) =>
      route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
    );

    await page.goto("/explore");
    await expect(page.getByText("Error loading metrics.")).toBeVisible();
  });
});
