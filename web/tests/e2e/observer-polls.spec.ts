// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { expect, test } from "@playwright/test";

const mockPolls = [
  {
    id: "poll-1",
    name: "Daily mood check",
    custom_prompt: "How did I seem today?",
    dimensions: ["energy", "mood", "focus"],
    members: [
      {
        id: "member-1",
        observer_email: "s***@example.com",
        accepted_at: "2026-03-01T00:00:00Z",
        created_at: "2026-02-28T00:00:00Z",
      },
    ],
    created_at: "2026-02-28T00:00:00Z",
    deleted_at: null,
  },
];

const mockObserverPolls = [
  {
    id: "poll-2",
    owner_display: "J***",
    name: "Partner wellness",
    custom_prompt: "How is your partner doing?",
    dimensions: ["energy", "mood"],
  },
];

function userJwt(): string {
  const payload = btoa(JSON.stringify({ sub: "u1", role: "user", exp: 9999999999 }));
  return `eyJhbGciOiJIUzI1NiJ9.${payload}.fake`;
}

async function mockApis(page: import("@playwright/test").Page) {
  const jwt = userJwt();

  await page.route("**/api/v1/auth/refresh", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
    }),
  );

  await page.route("**/api/v1/observer-polls", (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(mockPolls),
      });
    }
    if (route.request().method() === "POST") {
      return route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify({
          id: "poll-new",
          name: "New poll",
          custom_prompt: null,
          dimensions: ["energy", "mood"],
          members: [],
          created_at: new Date().toISOString(),
          deleted_at: null,
        }),
      });
    }
    return route.fallback();
  });

  await page.route("**/api/v1/observer-polls/my-polls", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(mockObserverPolls),
    }),
  );

  await page.route("**/api/v1/observer-polls/poll-1/invite", (route) =>
    route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({
        invite_token: "test-token",
        invite_expires_at: "2026-04-04T00:00:00Z",
        invite_url: "http://localhost:5173/observe/accept?token=test-token",
      }),
    }),
  );

  await page.route("**/api/v1/observer-polls/poll-1/responses", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ responses: [] }),
    }),
  );

  await page.route("**/api/v1/observer-polls/poll-2/respond", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        id: "resp-new",
        date: "2026-03-28",
        scores: { energy: 5, mood: 5 },
        created_at: "2026-03-28T10:00:00Z",
      }),
    }),
  );

  await page.route("**/api/v1/observer-polls/accept", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ status: "accepted" }),
    }),
  );

  // Catch other API calls
  await page.route("**/api/v1/source-preferences", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
  );
}

// TODO: Fix E2E tests to work without a running backend (use built app + route mocking)
test.describe.skip("Observer Polls page", () => {
  test("renders My Polls tab with poll list", async ({ page }) => {
    await mockApis(page);
    await page.goto("/observer-polls");

    await expect(page.getByText("Daily mood check")).toBeVisible();
    await expect(page.getByText("energy")).toBeVisible();
    await expect(page.getByText("mood")).toBeVisible();
    await expect(page.getByText(/1 member/)).toBeVisible();
  });

  test("create poll flow", async ({ page }) => {
    await mockApis(page);

    // After creation, re-fetch returns the new poll
    let created = false;
    await page.route("**/api/v1/observer-polls", (route) => {
      if (route.request().method() === "GET") {
        const polls = created
          ? [
              ...mockPolls,
              {
                id: "poll-new",
                name: "New poll",
                custom_prompt: null,
                dimensions: ["energy", "mood"],
                members: [],
                created_at: new Date().toISOString(),
                deleted_at: null,
              },
            ]
          : mockPolls;
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(polls),
        });
      }
      if (route.request().method() === "POST") {
        created = true;
        return route.fulfill({
          status: 201,
          contentType: "application/json",
          body: JSON.stringify({
            id: "poll-new",
            name: "New poll",
            custom_prompt: null,
            dimensions: ["energy", "mood"],
            members: [],
            created_at: new Date().toISOString(),
            deleted_at: null,
          }),
        });
      }
      return route.fallback();
    });

    await page.goto("/observer-polls");
    await expect(page.getByText("Daily mood check")).toBeVisible();

    await page.getByRole("button", { name: /create poll/i }).click();
    await page.getByLabel(/name/i).fill("New poll");
    await page.getByRole("button", { name: /^create$/i }).click();

    await expect(page.getByText("New poll")).toBeVisible();
  });

  test("generate invite link flow", async ({ page }) => {
    await mockApis(page);
    await page.goto("/observer-polls");

    await expect(page.getByText("Daily mood check")).toBeVisible();
    await page.getByText("Daily mood check").click();

    await expect(page.getByRole("button", { name: /generate invite link/i })).toBeVisible();
    await page.getByRole("button", { name: /generate invite link/i }).click();

    await expect(
      page.getByDisplayValue("http://localhost:5173/observe/accept?token=test-token"),
    ).toBeVisible();
  });

  test("Polls I Observe tab shows observer polls", async ({ page }) => {
    await mockApis(page);
    await page.goto("/observer-polls");

    await page.getByRole("button", { name: /polls i observe/i }).click();

    await expect(page.getByText("Partner wellness")).toBeVisible();
    await expect(page.getByText("J***")).toBeVisible();
  });

  test("observer accept page shows success", async ({ page }) => {
    await mockApis(page);
    await page.goto("/observe/accept?token=valid-token");

    await expect(page.getByText("Observer Invite Accepted")).toBeVisible();
    await expect(page.getByText(/added as an observer/)).toBeVisible();
  });

  test("observer accept page shows error for invalid token", async ({ page }) => {
    const jwt = userJwt();

    await page.route("**/api/v1/auth/refresh", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ access_token: jwt, token_type: "bearer", expires_in: 3600 }),
      }),
    );

    await page.route("**/api/v1/observer-polls/accept", (route) =>
      route.fulfill({
        status: 400,
        contentType: "text/plain",
        body: "Invalid or expired token",
      }),
    );

    await page.route("**/api/v1/source-preferences", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );

    await page.goto("/observe/accept?token=bad-token");

    await expect(page.getByText("Error")).toBeVisible();
    await expect(page.getByText("Invalid or expired token")).toBeVisible();
  });
});
