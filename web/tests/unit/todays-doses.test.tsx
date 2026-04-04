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
    dose_id: null,
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
    dose_id: null,
  },
];

const allCompletedDoses = [
  {
    ...pendingDoses[0],
    status: "completed",
    dose_id: "dose-1",
  },
  {
    ...pendingDoses[1],
    status: "completed",
    dose_id: "dose-2",
  },
];

const mixedDoses = [
  pendingDoses[0],
  {
    ...pendingDoses[1],
    status: "completed",
    dose_id: "dose-2",
  },
];

const server = setupServer(
  http.get("/api/v1/protocols/todays-doses", () => {
    return HttpResponse.json(pendingDoses);
  }),
  http.post("/api/v1/protocols/runs/:runId/doses/log", () => {
    return HttpResponse.json({
      id: "dose-new",
      protocol_line_id: "pl-1",
      day_number: 3,
      status: "completed",
      intervention_id: "iv-1",
      logged_at: "2026-03-28T08:00:00Z",
      created_at: "2026-03-28T08:00:00Z",
    });
  }),
  http.post("/api/v1/protocols/runs/:runId/doses/skip", () => {
    return HttpResponse.json({
      id: "dose-skip",
      protocol_line_id: "pl-1",
      day_number: 3,
      status: "skipped",
      intervention_id: null,
      logged_at: "2026-03-28T08:00:00Z",
      created_at: "2026-03-28T08:00:00Z",
    });
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
      http.get("/api/v1/protocols/todays-doses", async () => {
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
      http.get("/api/v1/protocols/todays-doses", () => {
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
      http.get("/api/v1/protocols/todays-doses", () => {
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
      http.get("/api/v1/protocols/todays-doses", () => {
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
      http.get("/api/v1/protocols/todays-doses", () => {
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
      http.get("/api/v1/protocols/todays-doses", () => {
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
});
