// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";
import { expect, test } from "@playwright/test";

// Accessibility gate: run axe-core against the key public and authenticated
// pages and fail on WCAG 2.1 A/AA `critical` and `serious` violations.
// Moderate/minor are reported (attached to the test) but not gated yet.
//
// These tests reuse the same pattern as the other E2E specs: Playwright route
// intercepts answer the API calls before they reach the Vite dev proxy, so no
// real backend or auth session is required. A fake JWT satisfies the auth
// store so protected pages render past their guard.

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"];
const GATED_IMPACTS = new Set(["critical", "serious"]);

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

/**
 * Run an axe scan and assert there are zero critical/serious violations.
 * Moderate and minor findings are attached to the test report for triage.
 */
async function expectNoSeriousA11yViolations(
  page: Page,
  testInfo: import("@playwright/test").TestInfo,
) {
  const results = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();

  await testInfo.attach("axe-results.json", {
    body: JSON.stringify(results.violations, null, 2),
    contentType: "application/json",
  });

  const gated = results.violations.filter((v) => GATED_IMPACTS.has(v.impact ?? ""));

  const summary = gated
    .map((v) => {
      const nodes = v.nodes.map((n) => n.target.join(" ")).join(", ");
      return `[${v.impact}] ${v.id}: ${v.help}\n  nodes: ${nodes}\n  ${v.helpUrl}`;
    })
    .join("\n\n");

  expect(gated, `Critical/serious a11y violations found:\n\n${summary}`).toEqual([]);
}

/** Authenticated-session boilerplate shared by every protected-page scan. */
async function mockAuthedSession(page: Page) {
  const jwt = fakeJwt();
  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );
  // SSE events stream — return an empty event-stream so the client doesn't hang.
  await page.route("**/api/v1/events*", (route) =>
    route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }),
  );
}

test.describe("Accessibility (axe-core)", () => {
  test("login page has no critical/serious violations", async ({ page }, testInfo) => {
    await page.goto("/login");
    await expect(page.getByRole("link", { name: /sign in with google/i })).toBeVisible();
    await expectNoSeriousA11yViolations(page, testInfo);
  });

  test("register page (no invite) has no critical/serious violations", async ({
    page,
  }, testInfo) => {
    await page.goto("/register");
    await expect(page.getByText(/you need an invite code to sign up/i)).toBeVisible();
    await expectNoSeriousA11yViolations(page, testInfo);
  });

  test("register page (valid invite form) has no critical/serious violations", async ({
    page,
  }, testInfo) => {
    await page.route("**/api/v1/invites/TEST-CODE/check", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          valid: true,
          code: "TEST-CODE",
          created_by_name: "Admin",
          expires_at: "2027-01-01T00:00:00Z",
        }),
      }),
    );
    await page.goto("/register?invite=TEST-CODE");
    await page.getByLabel(/^password$/i).waitFor();
    await expectNoSeriousA11yViolations(page, testInfo);
  });

  test("dashboard has no critical/serious violations", async ({ page }, testInfo) => {
    await mockAuthedSession(page);

    // Dashboard summary — concrete shape so the page renders past its guard.
    await page.route("**/api/v1/dashboard/summary", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          latest_checkin: {
            date: "2026-06-01",
            energy: 7,
            mood: 6,
            focus: 8,
            recovery: 5,
            libido: 6,
          },
          checkin_count_7d: 5,
          health_record_count_7d: 12,
          intervention_count_7d: 3,
          observation_count_7d: 8,
          latest_lab_date: "2026-05-20",
          pending_friend_shares: 1,
        }),
      }),
    );

    // Today's doses — one of each status so the (previously low-contrast)
    // completed/skipped status colors are present in the scanned DOM.
    await page.route("**/api/v1/protocols/runs/todays-doses", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          {
            protocol_id: "p1",
            protocol_name: "Morning Stack",
            protocol_line_id: "l1",
            run_id: "r1",
            substance: "Creatine",
            dose: 5,
            unit: "g",
            route: "oral",
            time_of_day: "morning",
            day_number: 3,
            status: "pending",
            dose_id: null,
          },
          {
            protocol_id: "p1",
            protocol_name: "Morning Stack",
            protocol_line_id: "l2",
            run_id: "r1",
            substance: "Vitamin D",
            dose: 2000,
            unit: "IU",
            route: "oral",
            time_of_day: "morning",
            day_number: 3,
            status: "completed",
            dose_id: "d2",
          },
          {
            protocol_id: "p1",
            protocol_name: "Morning Stack",
            protocol_line_id: "l3",
            run_id: "r1",
            substance: "Magnesium",
            dose: 200,
            unit: "mg",
            route: "oral",
            time_of_day: "evening",
            day_number: 3,
            status: "skipped",
            dose_id: "d3",
          },
        ]),
      }),
    );

    // Insights — one of every type so each (previously low-contrast) tag
    // background + white text pair is present in the scanned DOM.
    await page.route("**/api/v1/insights", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          {
            id: "i1",
            insight_type: "trend",
            headline: "Energy trending up",
            detail: "Up 12% over the last week.",
            metadata: {},
            created_at: "2026-06-01T00:00:00Z",
          },
          {
            id: "i2",
            insight_type: "anomaly",
            headline: "Unusual recovery dip",
            detail: "Recovery dropped below your baseline.",
            metadata: {},
            created_at: "2026-06-01T00:00:00Z",
          },
          {
            id: "i3",
            insight_type: "missing_data",
            headline: "No sleep data this week",
            detail: null,
            metadata: {},
            created_at: "2026-06-01T00:00:00Z",
          },
          {
            id: "i4",
            insight_type: "streak",
            headline: "5-day check-in streak",
            detail: "Keep it going.",
            metadata: {},
            created_at: "2026-06-01T00:00:00Z",
          },
          {
            id: "i5",
            insight_type: "correlation",
            headline: "Caffeine vs sleep",
            detail: "Negative correlation detected.",
            metadata: {},
            created_at: "2026-06-01T00:00:00Z",
          },
        ]),
      }),
    );

    // Sparkline / remaining dashboard fetches — empty payloads are fine.
    await page.route("**/api/v1/**", (route) => {
      const url = route.request().url();
      if (
        url.includes("/auth/") ||
        url.includes("/dashboard/summary") ||
        url.includes("/todays-doses") ||
        url.endsWith("/insights") ||
        url.includes("/events")
      ) {
        return route.fallback();
      }
      return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
    });

    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
    // Wait for the fixed components so their colored elements are in the DOM.
    await expect(page.getByRole("heading", { name: /today.s doses/i })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Insights" })).toBeVisible();
    await expectNoSeriousA11yViolations(page, testInfo);
  });

  test("explore page has no critical/serious violations", async ({ page }, testInfo) => {
    await mockAuthedSession(page);
    await page.route("**/api/v1/explore/metrics", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          sources: [
            {
              source: "checkins",
              label: "Check-ins",
              metrics: [{ field: "energy", label: "Energy", unit: "score" }],
            },
          ],
        }),
      }),
    );
    await page.route("**/api/v1/explore/charts", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );
    await page.route("**/api/v1/**", (route) => {
      const url = route.request().url();
      if (
        url.includes("/auth/") ||
        url.includes("/explore/metrics") ||
        url.includes("/explore/charts") ||
        url.includes("/events")
      ) {
        return route.fallback();
      }
      return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
    });

    await page.goto("/explore");
    await expect(page.getByRole("heading", { name: "Explore" })).toBeVisible();
    await expectNoSeriousA11yViolations(page, testInfo);
  });

  test("settings page has no critical/serious violations", async ({ page }, testInfo) => {
    await mockAuthedSession(page);
    await page.route("**/api/v1/auth/methods", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify([
          {
            id: "1",
            provider: "google",
            email: "user@example.com",
            created_at: "2026-01-01T00:00:00Z",
          },
        ]),
      }),
    );
    // Notification preferences returns an object (not a list).
    await page.route("**/api/v1/notifications/preferences", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ default_notify: false, default_notify_times: [] }),
      }),
    );
    await page.route("**/api/v1/**", (route) => {
      const url = route.request().url();
      if (url.includes("/auth/") || url.includes("/notifications/preferences")) {
        return route.fallback();
      }
      return route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
    });

    await page.goto("/settings");
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await expectNoSeriousA11yViolations(page, testInfo);
  });
});
