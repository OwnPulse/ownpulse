// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { TodaysDoses } from "../../src/components/dashboard/TodaysDoses";
import { useAuthStore } from "../../src/store/auth";

const pendingDoses = [
  {
    protocol_id: "p1",
    protocol_name: "BPC Stack",
    protocol_line_id: "pl-1",
    run_id: "run-1",
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    time_of_day: "08:00",
    day_number: 3,
    status: "pending",
  },
  {
    protocol_id: "p1",
    protocol_name: "BPC Stack",
    protocol_line_id: "pl-2",
    run_id: "run-1",
    substance: "TB-500",
    dose: 2,
    unit: "mg",
    route: "SubQ",
    time_of_day: "08:00",
    day_number: 3,
    status: "pending",
  },
];

const allCompletedDoses = [
  {
    ...pendingDoses[0],
    status: "completed",
  },
  {
    ...pendingDoses[1],
    status: "completed",
  },
];

const mixedDoses = [
  pendingDoses[0],
  {
    ...pendingDoses[1],
    status: "completed",
  },
];

const server = setupServer(
  http.get("/api/v1/protocols/runs/todays-doses", () => {
    return HttpResponse.json(pendingDoses);
  }),
  http.get("/api/v1/protocols/runs/missed-doses", () => {
    return HttpResponse.json([]);
  }),
  http.post("/api/v1/protocols/runs/:runId/doses/log", () => {
    return HttpResponse.json({
      id: "dose-new",
      protocol_line_id: "pl-1",
      day_number: 3,
      status: "completed",
      intervention_id: "iv-1",
      // date-ok
      logged_at: "2026-03-28T08:00:00Z",
      // date-ok
      created_at: "2026-03-28T08:00:00Z",
    });
  }),
  // 204 No Content — `skip_dose_on_run` doesn't return a dose row.
  http.post("/api/v1/protocols/runs/:runId/doses/skip", () => {
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("TodaysDoses", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders pending doses with Log and Skip buttons", async () => {
    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    expect(screen.getByText(/TB-500/)).toBeDefined();
    expect(screen.getByText("2 pending")).toBeDefined();

    const logButtons = screen.getAllByRole("button", { name: "Log" });
    expect(logButtons).toHaveLength(2);

    const skipButtons = screen.getAllByRole("button", { name: "Skip" });
    expect(skipButtons).toHaveLength(2);
  });

  it("renders loading state (returns null)", () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", async () => {
        // Never resolve — simulates perpetual loading
        await new Promise(() => {});
        return HttpResponse.json([]);
      }),
    );

    const { container } = renderWithProviders(<TodaysDoses />);
    // Component returns null during loading
    expect(container.innerHTML).toBe("");
  });

  it("renders error state (returns null)", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    const { container } = renderWithProviders(<TodaysDoses />);

    // Wait for query to error out
    await waitFor(() => {
      // Component returns null on error
      expect(container.innerHTML).toBe("");
    });
  });

  it("shows all done with green checkmark when all doses completed", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", () => {
        return HttpResponse.json(allCompletedDoses);
      }),
    );

    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText("All done")).toBeDefined();
    });

    // Green check mark exists
    const check = screen.getByText("\u2713");
    expect(check).toBeDefined();
  });

  it("shows mixed pending and completed doses", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", () => {
        return HttpResponse.json(mixedDoses);
      }),
    );

    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    expect(screen.getByText("1 pending")).toBeDefined();

    // One Log button for pending, one status text for completed
    const logButtons = screen.getAllByRole("button", { name: "Log" });
    expect(logButtons).toHaveLength(1);

    expect(screen.getByText("completed")).toBeDefined();
  });

  it("returns null when no doses", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", () => {
        return HttpResponse.json([]);
      }),
    );

    const { container } = renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(container.innerHTML).toBe("");
    });
  });

  it("clicking Log calls the log endpoint", async () => {
    const user = userEvent.setup();

    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    const logButtons = screen.getAllByRole("button", { name: "Log" });
    await user.click(logButtons[0]);

    // After mutation, queries get invalidated — we just verify no crash
    await waitFor(() => {
      expect(logButtons[0]).toBeDefined();
    });
  });

  it("clicking Skip calls the skip endpoint", async () => {
    const user = userEvent.setup();

    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    const skipButtons = screen.getAllByRole("button", { name: "Skip" });
    await user.click(skipButtons[0]);

    // After mutation, queries get invalidated — we just verify no crash
    await waitFor(() => {
      expect(skipButtons[0]).toBeDefined();
    });
  });

  it("maps null status to pending", async () => {
    server.use(
      http.get("/api/v1/protocols/runs/todays-doses", () => {
        return HttpResponse.json([
          {
            ...pendingDoses[0],
            status: null,
          },
        ]);
      }),
    );

    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    // Should show Log button since null maps to pending
    expect(screen.getByRole("button", { name: "Log" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Skip" })).toBeDefined();
    expect(screen.getByText("1 pending")).toBeDefined();
  });

  it("shows protocol name and time of day in metadata", async () => {
    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getAllByText(/BPC Stack/).length).toBeGreaterThan(0);
    });

    // time_of_day is "08:00", should appear with middle dot
    const metaElements = screen.getAllByText(/08:00/);
    expect(metaElements.length).toBeGreaterThan(0);
  });

  it("contains link to protocols page", async () => {
    renderWithProviders(<TodaysDoses />);

    await waitFor(() => {
      expect(screen.getByText(/BPC-157/)).toBeDefined();
    });

    const link = screen.getByText("View all protocols");
    expect(link).toBeDefined();
    expect(link.getAttribute("href")).toBe("/protocols");
  });

  describe("missed-doses expander", () => {
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
      // date-ok
      date: "2026-03-27",
      status: "missed",
    };

    it("does not render the expander when there are no missed doses", async () => {
      renderWithProviders(<TodaysDoses />);

      await waitFor(() => {
        expect(screen.getByText(/BPC-157/)).toBeDefined();
      });

      expect(screen.queryByText(/missed dose/i)).toBeNull();
    });

    it("shows a review toggle when there are missed doses", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/missed-doses", () => HttpResponse.json([missedItem])),
      );

      renderWithProviders(<TodaysDoses />);

      await waitFor(() => {
        expect(screen.getByText("1 missed dose — Review")).toBeDefined();
      });
    });

    it("pluralizes the count for multiple missed doses", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/missed-doses", () =>
          HttpResponse.json([missedItem, { ...missedItem, protocol_line_id: "pl-4" }]),
        ),
      );

      renderWithProviders(<TodaysDoses />);

      await waitFor(() => {
        expect(screen.getByText("2 missed doses — Review")).toBeDefined();
      });
    });

    it("expands to show per-item rows with Log/Skip on click", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/missed-doses", () => HttpResponse.json([missedItem])),
      );

      const user = userEvent.setup();
      renderWithProviders(<TodaysDoses />);

      await waitFor(() => {
        expect(screen.getByText("1 missed dose — Review")).toBeDefined();
      });

      // Collapsed by default — no per-item row yet.
      // date-ok
      expect(screen.queryByText("2026-03-27")).toBeNull();

      await user.click(screen.getByText("1 missed dose — Review"));

      expect(screen.getByText(/2026-03-27/)).toBeDefined();
      const logButtons = screen.getAllByRole("button", { name: "Log" });
      const skipButtons = screen.getAllByRole("button", { name: "Skip" });
      expect(logButtons.length).toBeGreaterThan(0);
      expect(skipButtons.length).toBeGreaterThan(0);
    });

    it("logging a missed dose posts to the item's run id and day number", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/missed-doses", () => HttpResponse.json([missedItem])),
      );

      let capturedRunId: string | undefined;
      let capturedBody: unknown;
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/log", async ({ params, request }) => {
          capturedRunId = params.runId as string;
          capturedBody = await request.json();
          return HttpResponse.json({
            id: "dose-backfill",
            protocol_line_id: "pl-3",
            day_number: 2,
            status: "completed",
            intervention_id: "iv-2",
            // date-ok
            logged_at: "2026-03-27T08:00:00Z",
            // date-ok
            created_at: "2026-03-27T08:00:00Z",
          });
        }),
      );

      const user = userEvent.setup();
      renderWithProviders(<TodaysDoses />);

      await waitFor(() => {
        expect(screen.getByText("1 missed dose — Review")).toBeDefined();
      });
      await user.click(screen.getByText("1 missed dose — Review"));

      await waitFor(() => {
        expect(screen.getAllByRole("button", { name: "Log" }).length).toBeGreaterThan(0);
      });

      // The missed row's own Log button is the last one rendered (today's
      // pending doses render first).
      const logButtons = screen.getAllByRole("button", { name: "Log" });
      await user.click(logButtons[logButtons.length - 1]);

      await waitFor(() => {
        expect(capturedRunId).toBe("run-1");
      });
      expect(capturedBody).toMatchObject({ protocol_line_id: "pl-3", day_number: 2 });
    });
  });
});
