// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import Sources from "../../src/pages/Sources";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <Sources />
    </QueryClientProvider>,
  );
}

describe("Sources page", () => {
  it("renders a loading state", () => {
    server.use(http.get("/api/v1/integrations", () => new Promise(() => {})));
    renderPage();
    expect(screen.getByText("Loading integrations...")).toBeDefined();
  });

  it("renders an error state", async () => {
    server.use(http.get("/api/v1/integrations", () => new HttpResponse("Error", { status: 500 })));
    renderPage();
    await waitFor(() => {
      expect(screen.getByText("Error loading integrations.")).toBeDefined();
    });
  });

  it("shows a Connect link for Google Calendar when it isn't connected", async () => {
    server.use(http.get("/api/v1/integrations", () => HttpResponse.json([])));
    renderPage();

    await waitFor(() => {
      expect(screen.getByText("google_calendar")).toBeDefined();
    });
    expect(screen.getAllByText("Disconnected").length).toBeGreaterThanOrEqual(1);
    const connectLink = screen.getByRole("link", { name: "Connect" });
    expect(connectLink.getAttribute("href")).toBe("/api/v1/auth/google-calendar/login");
  });

  it("shows Connected status, last sync time, and a Disconnect button for a connected source", async () => {
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json([
          {
            source: "google_calendar",
            connected: true,
            last_synced_at: "2026-08-01T12:00:00Z",
          },
        ]),
      ),
    );
    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Connected")).toBeDefined();
    });
    expect(screen.getByText(/Last sync: 2026-08-01T12:00:00Z/)).toBeDefined();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeDefined();
    // Already connected, so no separate Connect link should render for it.
    expect(screen.queryByRole("link", { name: "Connect" })).toBeNull();
  });

  it("surfaces last_sync_error when present", async () => {
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json([
          {
            source: "google_calendar",
            connected: true,
            last_sync_error: "Google token expired",
          },
        ]),
      ),
    );
    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Google token expired")).toBeDefined();
    });
  });

  it("disconnects a source and refreshes the list", async () => {
    let connected = true;
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json(connected ? [{ source: "google_calendar", connected: true }] : []),
      ),
      http.delete("/api/v1/integrations/google_calendar", () => {
        connected = false;
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Disconnect" })).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: "Disconnect" }));

    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Connect" })).toBeDefined();
    });
  });

  it("triggers a manual sync and refreshes on success", async () => {
    let syncCount = 0;
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json([{ source: "google_calendar", connected: true }]),
      ),
      http.post("/api/v1/integrations/google-calendar/sync", () => {
        syncCount += 1;
        return HttpResponse.json({ source: "google_calendar", records_inserted: 3 });
      }),
    );
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Sync now" })).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: "Sync now" }));

    await waitFor(() => {
      expect(syncCount).toBe(1);
    });
  });

  it("shows a retry-after message when sync is rate limited (429)", async () => {
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json([{ source: "google_calendar", connected: true }]),
      ),
      http.post(
        "/api/v1/integrations/google-calendar/sync",
        () =>
          new HttpResponse(JSON.stringify({ error: "rate limited" }), {
            status: 429,
            headers: { "retry-after": "30" },
          }),
      ),
    );
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Sync now" })).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: "Sync now" }));

    await waitFor(() => {
      expect(screen.getByText("Rate limited — try again in 30s.")).toBeDefined();
    });
  });

  it("shows a generic error message when sync fails with a 500", async () => {
    server.use(
      http.get("/api/v1/integrations", () =>
        HttpResponse.json([{ source: "google_calendar", connected: true }]),
      ),
      http.post(
        "/api/v1/integrations/google-calendar/sync",
        () => new HttpResponse("Error", { status: 500 }),
      ),
    );
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Sync now" })).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: "Sync now" }));

    await waitFor(() => {
      expect(screen.getByText("Sync failed.")).toBeDefined();
    });
  });
});
