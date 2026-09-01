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
  DoseStatusGrid: ({ runId, interactive }: { runId: string; interactive: boolean }) => (
    <div data-testid="dose-grid">
      Grid for {runId} ({interactive ? "interactive" : "read-only"})
    </div>
  ),
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
  start_date: "2026-03-01",
  duration_days: 28,
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
      schedule_pattern: Array(28).fill(true),
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
  start_date: "2026-03-28",
  duration_days: 28,
  status: "active",
  notify: false,
  notify_times: [],
  repeat_reminders: false,
  repeat_interval_minutes: 30,
  progress_pct: 10,
  doses_today: 1,
  doses_completed_today: 0,
  adherence_pct: null,
  doses_missed: null,
  created_at: "2026-03-28T10:00:00Z",
};

const runs = [
  activeRun,
  {
    id: "run-2",
    protocol_id: "proto-1",
    protocol_name: "BPC-157 Stack",
    user_id: "user-1",
    start_date: "2026-02-01",
    duration_days: 28,
    status: "completed",
    notify: false,
    notify_times: [],
    repeat_reminders: false,
    repeat_interval_minutes: 30,
    progress_pct: 100,
    doses_today: 0,
    doses_completed_today: 0,
    adherence_pct: 90,
    doses_missed: 1,
    created_at: "2026-02-01T10:00:00Z",
  },
];

const adherenceResponse = {
  run_id: "run-1",
  scheduled_so_far: 10,
  completed: 8,
  skipped: 1,
  missed: 1,
  adherence_pct: 88.9,
  lines: [],
};

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
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    });

    // Runs section
    expect(screen.getByText("Runs")).toBeDefined();
    expect(screen.getByText("Start New Run")).toBeDefined();

    // Run cards
    expect(screen.getByText("Started 2026-03-28")).toBeDefined();
    expect(screen.getByText("Started 2026-02-01")).toBeDefined();

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
    const mixedRuns = [runs[0], { ...runs[1], status: "paused", start_date: "2026-02-15" }];

    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json(mixedRuns)),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
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
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
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

  it("passes the active run id (not the protocol id) to the dose grid", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[0]])),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByTestId("dose-grid")).toBeDefined();
    });

    expect(screen.getByText("Grid for run-1 (interactive)")).toBeDefined();
  });

  it("shows adherence summary from the adherence endpoint", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[0]])),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText(/89% adherence · 8 done · 1 skipped · 1 missed/)).toBeDefined();
    });
  });

  it('shows "No closed days yet" when adherence_pct is null', async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[0]])),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json({ ...adherenceResponse, adherence_pct: null }),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("No closed days yet")).toBeDefined();
    });
  });

  it("shows a hint instead of the schedule grid only when there are no runs at all", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Schedule")).toBeDefined();
    });

    expect(screen.getByText("Start a run to see your schedule.")).toBeDefined();
    expect(screen.queryByTestId("dose-grid")).toBeNull();
  });

  it("falls back to the most recent run and renders a read-only grid when there is no active run (completed only)", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[1]])), // completed only
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByTestId("dose-grid")).toBeDefined();
    });

    expect(screen.getByText("Grid for run-2 (read-only)")).toBeDefined();
    expect(screen.getByText(/this run is completed — showing read-only history/i)).toBeDefined();
  });

  it("falls back to the most recent run and renders a read-only grid when the only run is paused", async () => {
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () =>
        HttpResponse.json([{ ...runs[0], status: "paused" }]),
      ),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Grid for run-1 (read-only)")).toBeDefined();
    });
    expect(screen.getByText(/this run is paused — showing read-only history/i)).toBeDefined();
  });

  it("prefers the active run over a more recent non-active run", async () => {
    // runs[] is ordered created_at DESC like the real API — an older
    // active run must still win over a newer completed one.
    server.use(
      http.get("/api/v1/protocols/:id", () => HttpResponse.json(protocol)),
      http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json([runs[1], runs[0]])),
      http.get("/api/v1/protocols/runs/:runId/adherence", () =>
        HttpResponse.json(adherenceResponse),
      ),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Grid for run-1 (interactive)")).toBeDefined();
    });
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
        HttpResponse.json({ token: "share-abc", expires_at: "2026-04-30T00:00:00Z" }),
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
});
