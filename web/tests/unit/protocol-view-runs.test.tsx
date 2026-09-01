// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import ProtocolView from "../../src/pages/ProtocolView";
import { useAuthStore } from "../../src/store/auth";

vi.mock("../../src/components/protocols/DoseStatusGrid", () => ({
  DoseStatusGrid: () => <div data-testid="dose-grid">Grid</div>,
}));

vi.mock("../../src/components/protocols/StartRunModal", () => ({
  StartRunModal: ({ protocolName, onClose }: { protocolName: string; onClose: () => void }) => (
    <div data-testid="start-run-modal">
      <span>{protocolName}</span>
      <button type="button" onClick={onClose}>
        Close
      </button>
    </div>
  ),
}));

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const protocol = {
  id: "proto-1",
  user_id: "user-1",
  name: "BPC-157 Stack",
  description: "Healing protocol",
  status: "active",
  start_date: "2026-03-01", // date-ok
  duration_days: 28,
  share_token: null,
  created_at: "2026-03-01T00:00:00Z", // date-ok
  updated_at: "2026-03-01T00:00:00Z", // date-ok
  lines: [
    {
      id: "line-1",
      protocol_id: "proto-1",
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      schedule_pattern: Array(28).fill(true),
      sort_order: 0,
      doses: [],
    },
  ],
};

const runs = [
  {
    id: "run-1",
    protocol_id: "proto-1",
    user_id: "user-1",
    start_date: "2026-03-28", // date-ok
    status: "active",
    notify: false,
    notify_times: [],
    repeat_reminders: false,
    repeat_interval_minutes: 30,
    created_at: "2026-03-28T10:00:00Z", // date-ok
  },
  {
    id: "run-2",
    protocol_id: "proto-1",
    user_id: "user-1",
    start_date: "2026-02-01", // date-ok
    status: "completed",
    notify: false,
    notify_times: [],
    repeat_reminders: false,
    repeat_interval_minutes: 30,
    created_at: "2026-02-01T10:00:00Z", // date-ok
  },
];

function renderWithProviders() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/protocols/proto-1"]}>
        <Routes>
          <Route path="/protocols/:id" element={<ProtocolView />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("ProtocolView with runs", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
  });

  it("renders loading state", () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => new Promise(() => {})),
      http.get("/api/v1/protocols/:id/runs", () => new Promise(() => {})),
    );

    renderWithProviders();
    expect(screen.getByText("Loading...")).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => new HttpResponse("Error", { status: 500 })),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
    );

    renderWithProviders();
    await waitFor(() => {
      expect(screen.getByText("Error loading protocol.")).toBeDefined();
    });
  });

  it("renders protocol with runs list", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json(runs)),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    });

    // Runs section
    expect(screen.getByText("Runs")).toBeDefined();
    expect(screen.getByText("Start New Run")).toBeDefined();

    // Run cards
    expect(screen.getByText("Started 2026-03-28")).toBeDefined(); // date-ok
    expect(screen.getByText("Started 2026-02-01")).toBeDefined(); // date-ok

    // Active run has Pause + Complete buttons
    expect(screen.getByText("Pause")).toBeDefined();
    expect(screen.getByText("Complete")).toBeDefined();
  });

  it("renders empty runs message when no runs", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("No runs yet. Start your first run.")).toBeDefined();
    });
  });

  it("opens Start New Run modal on click", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Start New Run")).toBeDefined();
    });

    await user.click(screen.getByText("Start New Run"));
    expect(screen.getByTestId("start-run-modal")).toBeDefined();
  });

  it("shows Pause/Complete for active run and Resume for paused run", async () => {
    const mixedRuns = [runs[0], { ...runs[1], status: "paused", start_date: "2026-02-15" }]; // date-ok

    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json(mixedRuns)),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Pause")).toBeDefined();
    });

    expect(screen.getByText("Complete")).toBeDefined();
    expect(screen.getByText("Resume")).toBeDefined();
  });

  it("calls updateRun when Pause is clicked", async () => {
    let patchCalled = false;
    let capturedBody: unknown;

    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[0]])),
      http.patch("/api/v1/protocols/runs/:runId", async ({ params, request }) => {
        patchCalled = true;
        expect(params.runId).toBe("run-1");
        capturedBody = await request.json();
        return HttpResponse.json({ ...runs[0], status: "paused" });
      }),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Pause")).toBeDefined();
    });

    await user.click(screen.getByText("Pause"));

    await waitFor(() => {
      expect(patchCalled).toBe(true);
    });
    expect(capturedBody).toEqual({ status: "paused" });
  });

  it("calls logRunDose with the active run id (not the protocol id) when Log is clicked", async () => {
    let capturedRunId: string | undefined;
    let capturedUrl: string | undefined;
    let capturedBody: unknown;

    // Local date, matching how ProtocolView now parses run.start_date — using
    // the UTC date here would make this test flaky for contributors west of
    // UTC near midnight.
    const now = new Date();
    const todayStr = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    const activeRun = {
      id: "run-1",
      protocol_id: "proto-1",
      user_id: "user-1",
      start_date: todayStr,
      status: "active" as const,
      notify: false,
      notify_times: [],
      repeat_reminders: false,
      repeat_interval_minutes: 30,
      created_at: `${todayStr}T10:00:00Z`,
    };

    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([activeRun])),
      http.post("/api/v1/protocols/runs/:runId/doses/log", async ({ params, request }) => {
        capturedRunId = params.runId as string;
        capturedUrl = request.url;
        capturedBody = await request.json();
        return HttpResponse.json({
          id: "dose-1",
          protocol_line_id: "line-1",
          day_number: 0,
          status: "completed",
          intervention_id: "iv-1",
          logged_at: `${todayStr}T12:00:00Z`,
          created_at: `${todayStr}T12:00:00Z`,
        });
      }),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Today’s Doses")).toBeDefined();
    });

    const logButton = screen.getByRole("button", { name: "Log" });
    expect(logButton).not.toBeDisabled();
    await user.click(logButton);

    await waitFor(() => {
      expect(capturedRunId).toBe("run-1");
    });

    // Must NOT have posted using the protocol id ("proto-1") as the run id.
    expect(capturedRunId).not.toBe(protocol.id);
    expect(capturedUrl).toContain("/api/v1/protocols/runs/run-1/doses/log");
    // The request body itself must be the exact expected shape — a handler
    // that returns 200 for any body (e.g. `{totally_wrong: 1}`) would pass a
    // test that only checks the URL.
    expect(capturedBody).toEqual({ protocol_line_id: "line-1", day_number: 0 });
  });

  it("shows a hint instead of the dose list when there is no active run", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[1]])), // completed only
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Today’s Doses")).toBeDefined();
    });

    expect(screen.getByText("Start a run to log doses")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Log" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Skip" })).toBeNull();
  });

  it("shows the same hint when the only run is paused", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () =>
        HttpResponse.json([{ ...runs[0], status: "paused" }]),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Today’s Doses")).toBeDefined();
    });

    expect(screen.getByText("Start a run to log doses")).toBeDefined();
  });

  it("renders description section", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Healing protocol")).toBeDefined();
    });
  });

  it("builds the share link from the response's `token` field", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
      http.post("/api/v1/protocols/:id/share", () =>
        HttpResponse.json({ token: "share-abc", expires_at: "2026-04-30T00:00:00Z" }), // date-ok
      ),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Share")).toBeDefined();
    });

    await user.click(screen.getByText("Share"));

    await waitFor(() => {
      const input = screen.getByDisplayValue(/\/protocols\/shared\/share-abc$/);
      expect(input).toBeDefined();
    });
  });

  describe("today's-dose day math at a UTC-offset-sensitive instant", () => {
    const originalTz = process.env.TZ;

    beforeEach(() => {
      // UTC-10, no DST — a run.start_date parsed as UTC midnight instead of
      // local midnight rolls its "day 0" forward by 10h relative to this
      // timezone's actual local day.
      process.env.TZ = "Pacific/Honolulu";
    });

    afterEach(() => {
      vi.useRealTimers();
      if (originalTz === undefined) {
        delete process.env.TZ;
      } else {
        process.env.TZ = originalTz;
      }
    });

    it("does not show today's doses before the run's start date has arrived locally", async () => {
      // 2026-03-29T05:00:00Z is 2026-03-28T19:00 in Honolulu — local
      // calendar is still the day *before* the run's start_date.
      vi.setSystemTime(new Date("2026-03-29T05:00:00Z")); // date-ok

      server.use(
        http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
        http.get("/api/v1/protocols/:id/runs", () =>
          HttpResponse.json([{ ...runs[0], start_date: "2026-03-29" }]), // date-ok
        ),
      );

      renderWithProviders();

      await waitFor(() => {
        expect(screen.getByText("Today’s Doses")).toBeDefined();
      });

      // Parsing start_date as UTC (the bug) would put "now" 5h after that
      // UTC instant, i.e. inside day 0 of the run — showing today's dose a
      // full local day early. Parsing it as local correctly treats the run
      // as not yet started for this user's actual calendar day.
      expect(screen.getByText("No doses scheduled for today.")).toBeDefined();
      expect(screen.queryByRole("button", { name: "Log" })).toBeNull();
    });

    it("shows today's dose once the run's start date has arrived locally", async () => {
      // 2026-03-29T05:00:00Z is 2026-03-28T19:00 in Honolulu — the local
      // calendar day matching the run's start_date.
      vi.setSystemTime(new Date("2026-03-29T05:00:00Z")); // date-ok

      server.use(
        http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
        http.get("/api/v1/protocols/:id/runs", () =>
          HttpResponse.json([{ ...runs[0], start_date: "2026-03-28" }]), // date-ok
        ),
      );

      renderWithProviders();

      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Log" })).toBeDefined();
      });
    });
  });
});
