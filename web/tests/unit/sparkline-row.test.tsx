// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { SparklineRow } from "../../src/components/dashboard/SparklineRow";
import { useAuthStore } from "../../src/store/auth";

// Mock unovis (SVG/D3 doesn't render in jsdom)
vi.mock("@unovis/react", () => ({
  VisXYContainer: ({ children }: { children: ReactNode }) => (
    <div data-testid="xy-container">{children}</div>
  ),
  VisLine: () => <div data-testid="vis-line" />,
}));

const sparklineResponse = {
  series: [
    {
      source: "checkins",
      field: "energy",
      unit: "score",
      points: [
        { t: "2026-03-21T00:00:00Z", v: 5, n: 1 },
        { t: "2026-03-22T00:00:00Z", v: 6, n: 1 },
        { t: "2026-03-23T00:00:00Z", v: 7, n: 1 },
        { t: "2026-03-24T00:00:00Z", v: 7, n: 1 },
        { t: "2026-03-25T00:00:00Z", v: 8, n: 1 },
        { t: "2026-03-26T00:00:00Z", v: 8, n: 1 },
        { t: "2026-03-27T00:00:00Z", v: 9, n: 1 },
      ],
    },
    {
      source: "checkins",
      field: "mood",
      unit: "score",
      points: [
        { t: "2026-03-21T00:00:00Z", v: 8, n: 1 },
        { t: "2026-03-22T00:00:00Z", v: 7, n: 1 },
        { t: "2026-03-23T00:00:00Z", v: 6, n: 1 },
        { t: "2026-03-24T00:00:00Z", v: 5, n: 1 },
        { t: "2026-03-25T00:00:00Z", v: 5, n: 1 },
        { t: "2026-03-26T00:00:00Z", v: 4, n: 1 },
        { t: "2026-03-27T00:00:00Z", v: 4, n: 1 },
      ],
    },
    {
      source: "checkins",
      field: "focus",
      unit: "score",
      points: [{ t: "2026-03-27T00:00:00Z", v: 6, n: 1 }],
    },
    {
      source: "checkins",
      field: "recovery",
      unit: "score",
      points: [],
    },
    {
      source: "checkins",
      field: "libido",
      unit: "score",
      points: [
        { t: "2026-03-21T00:00:00Z", v: 5, n: 1 },
        { t: "2026-03-27T00:00:00Z", v: 5, n: 1 },
      ],
    },
  ],
};

const server = setupServer(
  http.post("/api/v1/explore/series", () => {
    return HttpResponse.json(sparklineResponse);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>{children}</MemoryRouter>
      </QueryClientProvider>
    );
  };
}

describe("SparklineRow", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders loading state with 5 dimension placeholders", () => {
    server.use(
      http.post("/api/v1/explore/series", () => {
        return new Promise(() => {});
      }),
    );
    render(<SparklineRow />, { wrapper: createWrapper() });
    expect(screen.getByTestId("sparkline-row-loading")).toBeDefined();
    expect(screen.getByText("energy")).toBeDefined();
    expect(screen.getByText("mood")).toBeDefined();
    expect(screen.getByText("focus")).toBeDefined();
    expect(screen.getByText("recovery")).toBeDefined();
    expect(screen.getByText("libido")).toBeDefined();
  });

  it("renders 5 sparklines with data", async () => {
    render(<SparklineRow />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByTestId("sparkline-row")).toBeDefined();
    });
    expect(screen.getByTestId("sparkline-energy")).toBeDefined();
    expect(screen.getByTestId("sparkline-mood")).toBeDefined();
    expect(screen.getByTestId("sparkline-focus")).toBeDefined();
    expect(screen.getByTestId("sparkline-recovery")).toBeDefined();
    expect(screen.getByTestId("sparkline-libido")).toBeDefined();
  });

  it("shows current value from last data point", async () => {
    render(<SparklineRow />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByTestId("sparkline-row")).toBeDefined();
    });
    // Energy last point is 9
    const energyItem = screen.getByTestId("sparkline-energy");
    expect(energyItem.textContent).toContain("9");
    // Mood last point is 4
    const moodItem = screen.getByTestId("sparkline-mood");
    expect(moodItem.textContent).toContain("4");
  });

  it("shows dash for dimensions with no data", async () => {
    render(<SparklineRow />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByTestId("sparkline-row")).toBeDefined();
    });
    // Recovery has no points
    const recoveryItem = screen.getByTestId("sparkline-recovery");
    expect(recoveryItem.textContent).toContain("\u2014");
  });

  it("renders nothing on error", async () => {
    server.use(
      http.post("/api/v1/explore/series", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );
    const { container } = render(<SparklineRow />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.queryByTestId("sparkline-row-loading")).toBeNull();
    });
    // On error, SparklineRow returns null
    expect(screen.queryByTestId("sparkline-row")).toBeNull();
    expect(container.innerHTML).toBe("");
  });

  it("renders vis-line components for dimensions with data", async () => {
    render(<SparklineRow />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByTestId("sparkline-row")).toBeDefined();
    });
    // Should have VisLine for energy, mood, focus, libido (4 with data), not recovery (0 points)
    const lines = screen.getAllByTestId("vis-line");
    expect(lines.length).toBe(4);
  });
});
