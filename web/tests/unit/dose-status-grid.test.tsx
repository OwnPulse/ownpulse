// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { DoseStatusGrid } from "../../src/components/protocols/DoseStatusGrid";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();
beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function makeItem(day: number, overrides: Partial<Record<string, unknown>> = {}) {
  return {
    day_number: day,
    date: `2026-03-${String(day + 1).padStart(2, "0")}`,
    protocol_line_id: "line-1",
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    time_of_day: "AM",
    status: "pending",
    dose_id: null,
    intervention_id: null,
    skip_reason: null,
    logged_at: null,
    ...overrides,
  };
}

function renderGrid(runId = "run-1", durationDays = 3) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <DoseStatusGrid runId={runId} durationDays={durationDays} />
    </QueryClientProvider>,
  );
}

describe("DoseStatusGrid", () => {
  beforeAll(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
  });

  it("renders loading state", () => {
    server.use(http.get("/api/v1/protocols/runs/:runId/doses", () => new Promise(() => {})));
    renderGrid();
    expect(screen.getByText("Loading schedule...")).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get(
        "/api/v1/protocols/runs/:runId/doses",
        () => new HttpResponse("Server error", { status: 500 }),
      ),
    );
    renderGrid();
    await waitFor(() => {
      expect(screen.getByText("Error loading schedule.")).toBeDefined();
    });
  });

  it("renders a cell per scheduled day and an off cell where unscheduled", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([
          makeItem(0, { status: "completed" }),
          makeItem(2, { status: "pending" }),
        ]),
      ),
    );
    renderGrid("run-1", 3);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, completed — undo")).toBeDefined();
    });
    expect(screen.getByLabelText("Day 3, pending — log or skip")).toBeDefined();
    // Day 2 (index 1) has no data row — rendered as a non-interactive off cell.
    expect(screen.queryByLabelText(/Day 2,/)).toBeNull();
  });

  it("missed and pending cells are actionable via a Log/Skip popover", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "missed" })]),
      ),
    );
    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, missed — log or skip")).toBeDefined();
    });

    await user.click(screen.getByLabelText("Day 1, missed — log or skip"));

    expect(screen.getByRole("dialog", { name: "Log day 1" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Switch to log form" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Switch to skip form" })).toBeDefined();
  });

  it("logs a missed dose with optional time and notes", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "missed" })]),
      ),
    );

    let capturedBody: unknown;
    server.use(
      http.post("/api/v1/protocols/runs/:runId/doses/log", async ({ request }) => {
        capturedBody = await request.json();
        return HttpResponse.json({
          id: "dose-1",
          protocol_line_id: "line-1",
          day_number: 0,
          status: "completed",
          intervention_id: "iv-1",
          logged_at: "2026-03-01T08:00:00Z",
          run_id: "run-1",
          skip_reason: null,
        });
      }),
    );

    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, missed — log or skip")).toBeDefined();
    });
    await user.click(screen.getByLabelText("Day 1, missed — log or skip"));

    const notesField = screen.getByLabelText(/notes/i);
    await user.type(notesField, "felt fine");

    await user.click(screen.getByRole("button", { name: "Log" }));

    await waitFor(() => {
      expect(capturedBody).toMatchObject({
        protocol_line_id: "line-1",
        day_number: 0,
        notes: "felt fine",
      });
    });
  });

  it("skips a missed dose with an optional reason", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "missed" })]),
      ),
    );

    let capturedBody: unknown;
    server.use(
      // 204 No Content — `skip_dose_on_run` doesn't return a dose row.
      http.post("/api/v1/protocols/runs/:runId/doses/skip", async ({ request }) => {
        capturedBody = await request.json();
        return new HttpResponse(null, { status: 204 });
      }),
    );

    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, missed — log or skip")).toBeDefined();
    });
    await user.click(screen.getByLabelText("Day 1, missed — log or skip"));
    await user.click(screen.getByRole("button", { name: "Switch to skip form" }));

    const reasonField = screen.getByLabelText(/reason/i);
    await user.type(reasonField, "traveling");
    await user.click(screen.getByRole("button", { name: "Skip" }));

    await waitFor(() => {
      expect(capturedBody).toEqual({
        protocol_line_id: "line-1",
        day_number: 0,
        skip_reason: "traveling",
      });
    });
  });

  it("undo requires a second, confirming click before it DELETEs", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "completed", dose_id: "dose-1" })]),
      ),
    );

    let deleteCalls = 0;
    let deletedDoseId: string | undefined;
    server.use(
      http.delete("/api/v1/protocols/runs/:runId/doses/:doseId", ({ params }) => {
        deleteCalls++;
        deletedDoseId = params.doseId as string;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, completed — undo")).toBeDefined();
    });

    // First click only arms the confirmation — no DELETE yet.
    await user.click(screen.getByLabelText("Day 1, completed — undo"));
    expect(deleteCalls).toBe(0);
    const confirmButton = screen.getByLabelText("Day 1, completed — confirm undo");
    expect(confirmButton).toBeDefined();

    await user.click(confirmButton);

    await waitFor(() => {
      expect(deletedDoseId).toBe("dose-1");
    });
    expect(deleteCalls).toBe(1);
  });

  it("shows the mutation error inside the popover and keeps it open", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "missed" })]),
      ),
      http.post(
        "/api/v1/protocols/runs/:runId/doses/log",
        () => new HttpResponse("day already closed", { status: 400 }),
      ),
    );

    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, missed — log or skip")).toBeDefined();
    });
    await user.click(screen.getByLabelText("Day 1, missed — log or skip"));
    await user.click(screen.getByRole("button", { name: "Log", exact: true }));

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain("day already closed");
    });
    // The popover is still open — the failed write wasn't silently swallowed.
    expect(screen.getByRole("dialog", { name: "Log day 1" })).toBeDefined();
  });

  it("closes the popover on Escape", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", () =>
        HttpResponse.json([makeItem(0, { status: "missed" })]),
      ),
    );

    const user = userEvent.setup();
    renderGrid("run-1", 1);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, missed — log or skip")).toBeDefined();
    });
    await user.click(screen.getByLabelText("Day 1, missed — log or skip"));
    expect(screen.getByRole("dialog", { name: "Log day 1" })).toBeDefined();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Log day 1" })).toBeNull();
  });

  it("does not render actionable cells for days beyond the server's default range", async () => {
    // The server default (no explicit from_day/to_day) is 0..=min(today,
    // duration-1) — a run-doses response that only covers today (day 0 of
    // a 3-day grid) must not make days 1-2 actionable client-side; they
    // should render as inert "off" cells instead.
    server.use(
      http.get("/api/v1/protocols/runs/:runId/doses", ({ request }) => {
        const url = new URL(request.url);
        // Confirms the grid doesn't force an explicit to_day beyond today.
        expect(url.searchParams.get("to_day")).toBeNull();
        return HttpResponse.json([makeItem(0, { status: "completed" })]);
      }),
    );

    renderGrid("run-1", 3);

    await waitFor(() => {
      expect(screen.getByLabelText("Day 1, completed — undo")).toBeDefined();
    });
    expect(screen.queryByLabelText(/Day 2,/)).toBeNull();
    expect(screen.queryByLabelText(/Day 3,/)).toBeNull();
  });
});
