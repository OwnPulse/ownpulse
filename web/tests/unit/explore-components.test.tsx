// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { SavedChart } from "../../src/api/explore";
import { ChartLegend } from "../../src/components/explore/ChartLegend";
import { DateRangeBar } from "../../src/components/explore/DateRangeBar";
import { ExploreChart } from "../../src/components/explore/ExploreChart";
import { MetricPicker } from "../../src/components/explore/MetricPicker";
import { ResolutionToggle } from "../../src/components/explore/ResolutionToggle";
import { SavedChartCard } from "../../src/components/explore/SavedChartCard";
import { useExploreStore } from "../../src/stores/exploreStore";

// Mock unovis (SVG/D3 doesn't render in jsdom)
vi.mock("@unovis/react", () => ({
  VisXYContainer: ({ children }: { children: ReactNode }) => (
    <div data-testid="xy-container">{children}</div>
  ),
  VisLine: () => <div data-testid="vis-line" />,
  VisAxis: ({ type, label }: { type?: string; label?: string }) => (
    <div data-testid={`axis-${type ?? label}`} />
  ),
  VisCrosshair: () => <div data-testid="crosshair" />,
  VisTooltip: () => <div data-testid="tooltip" />,
}));

const metricsResponse = {
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
};

const server = setupServer(
  http.get("/api/v1/explore/metrics", () => {
    return HttpResponse.json(metricsResponse);
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

describe("MetricPicker", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("renders loading state", () => {
    server.use(
      http.get("/api/v1/explore/metrics", () => {
        // Never respond to keep loading
        return new Promise(() => {});
      }),
    );
    render(<MetricPicker />, { wrapper: createWrapper() });
    expect(screen.getByText("Loading metrics...")).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get("/api/v1/explore/metrics", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );
    render(<MetricPicker />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByText("Error loading metrics.")).toBeDefined();
    });
  });

  it("renders source groups and metrics", async () => {
    render(<MetricPicker />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByText("Check-ins")).toBeDefined();
      expect(screen.getByText("Health Records")).toBeDefined();
      expect(screen.getByText("Energy")).toBeDefined();
      expect(screen.getByText("Mood")).toBeDefined();
      expect(screen.getByText("Weight")).toBeDefined();
    });
  });

  it("toggles metric selection on checkbox click", async () => {
    const user = userEvent.setup();
    render(<MetricPicker />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy")).toBeDefined();
    });

    const energyCheckbox = screen.getByRole("checkbox", { name: /energy/i });
    await user.click(energyCheckbox);

    expect(useExploreStore.getState().selectedMetrics).toEqual([
      { source: "checkins", field: "energy" },
    ]);

    await user.click(energyCheckbox);
    expect(useExploreStore.getState().selectedMetrics).toEqual([]);
  });

  it("filters metrics by search", async () => {
    const user = userEvent.setup();
    render(<MetricPicker />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy")).toBeDefined();
    });

    const searchInput = screen.getByLabelText("Search metrics");
    await user.type(searchInput, "weight");

    expect(screen.getByText("Weight")).toBeDefined();
    expect(screen.queryByText("Energy")).toBeNull();
    expect(screen.queryByText("Mood")).toBeNull();
  });
});

describe("DateRangeBar", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("renders preset buttons", () => {
    render(<DateRangeBar />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: "7D" })).toBeDefined();
    expect(screen.getByRole("button", { name: "30D" })).toBeDefined();
    expect(screen.getByRole("button", { name: "90D" })).toBeDefined();
    expect(screen.getByRole("button", { name: "1Y" })).toBeDefined();
    expect(screen.getByRole("button", { name: "All" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Custom" })).toBeDefined();
  });

  it("clicking preset updates store", async () => {
    const user = userEvent.setup();
    render(<DateRangeBar />, { wrapper: createWrapper() });

    await user.click(screen.getByRole("button", { name: "7D" }));
    expect(useExploreStore.getState().dateRange).toEqual({ type: "preset", preset: "7d" });
  });

  it("shows custom date inputs when Custom clicked", async () => {
    const user = userEvent.setup();
    render(<DateRangeBar />, { wrapper: createWrapper() });

    await user.click(screen.getByRole("button", { name: "Custom" }));
    expect(screen.getByLabelText("Start date")).toBeDefined();
    expect(screen.getByLabelText("End date")).toBeDefined();
  });
});

describe("ResolutionToggle", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("renders three resolution buttons", () => {
    render(<ResolutionToggle />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: "Daily" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Weekly" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Monthly" })).toBeDefined();
  });

  it("clicking button updates resolution in store", async () => {
    const user = userEvent.setup();
    render(<ResolutionToggle />, { wrapper: createWrapper() });

    await user.click(screen.getByRole("button", { name: "Weekly" }));
    expect(useExploreStore.getState().resolution).toBe("weekly");
  });

  it("marks active button with aria-pressed", () => {
    render(<ResolutionToggle />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: "Daily" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Weekly" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
  });
});

describe("ExploreChart", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("shows empty message when no series provided", () => {
    render(<ExploreChart series={[]} />, { wrapper: createWrapper() });
    expect(screen.getByText("Select metrics from the picker to start exploring.")).toBeDefined();
  });

  it("shows no data message when series have no points", () => {
    const series = [{ source: "checkins", field: "energy", unit: "score", points: [] }];
    render(<ExploreChart series={series} />, { wrapper: createWrapper() });
    expect(screen.getByText("No data available for the selected metrics and range.")).toBeDefined();
  });

  it("renders chart container when data is provided", () => {
    const series = [
      {
        source: "checkins",
        field: "energy",
        unit: "score",
        points: [
          { t: "2026-03-01T00:00:00Z", v: 7, n: 1 },
          { t: "2026-03-02T00:00:00Z", v: 6, n: 1 },
        ],
      },
    ];
    render(<ExploreChart series={series} />, { wrapper: createWrapper() });
    expect(screen.getByTestId("xy-container")).toBeDefined();
    expect(screen.getByTestId("vis-line")).toBeDefined();
  });

  it("renders multiple lines for multiple series", () => {
    const series = [
      {
        source: "checkins",
        field: "energy",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 7, n: 1 }],
      },
      {
        source: "checkins",
        field: "mood",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 8, n: 1 }],
      },
    ];
    render(<ExploreChart series={series} />, { wrapper: createWrapper() });
    const lines = screen.getAllByTestId("vis-line");
    expect(lines).toHaveLength(2);
  });

  it("does not render hidden metrics", () => {
    useExploreStore.setState({
      selectedMetrics: [
        { source: "checkins", field: "energy" },
        { source: "checkins", field: "mood" },
      ],
      hiddenMetrics: new Set(["checkins:energy"]),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });

    const series = [
      {
        source: "checkins",
        field: "energy",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 7, n: 1 }],
      },
      {
        source: "checkins",
        field: "mood",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 8, n: 1 }],
      },
    ];
    render(<ExploreChart series={series} />, { wrapper: createWrapper() });
    const lines = screen.getAllByTestId("vis-line");
    expect(lines).toHaveLength(1);
  });
});

describe("ChartLegend", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("renders nothing when no series", () => {
    const { container } = render(<ChartLegend series={[]} />, {
      wrapper: createWrapper(),
    });
    expect(container.textContent).toBe("");
  });

  it("renders legend items for each series", () => {
    const series = [
      {
        source: "checkins",
        field: "energy",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 7, n: 1 }],
      },
      {
        source: "checkins",
        field: "mood",
        unit: "score",
        points: [],
      },
    ];
    render(<ChartLegend series={series} />, { wrapper: createWrapper() });
    expect(screen.getByLabelText("Toggle energy visibility")).toBeDefined();
    expect(screen.getByLabelText("Toggle mood visibility")).toBeDefined();
    expect(screen.getByText("- no data")).toBeDefined();
  });

  it("clicking legend item toggles visibility in store", async () => {
    const user = userEvent.setup();
    const series = [
      {
        source: "checkins",
        field: "energy",
        unit: "score",
        points: [{ t: "2026-03-01T00:00:00Z", v: 7, n: 1 }],
      },
    ];
    render(<ChartLegend series={series} />, { wrapper: createWrapper() });

    await user.click(screen.getByLabelText("Toggle energy visibility"));
    expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(true);

    await user.click(screen.getByLabelText("Toggle energy visibility"));
    expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(false);
  });
});

describe("SavedChartCard", () => {
  const chart: SavedChart = {
    id: "chart-1",
    name: "My Weekly Chart",
    config: {
      version: 1,
      metrics: [
        { source: "checkins", field: "energy" },
        { source: "checkins", field: "mood" },
      ],
      range: { preset: "30d" },
      resolution: "weekly",
    },
    created_at: "2026-03-01T00:00:00Z",
    updated_at: "2026-03-01T00:00:00Z",
  };

  it("renders chart name and metadata", () => {
    render(<SavedChartCard chart={chart} onLoad={() => {}} onDelete={() => {}} />, {
      wrapper: createWrapper(),
    });
    expect(screen.getByText("My Weekly Chart")).toBeDefined();
    expect(screen.getByText(/2 metrics/)).toBeDefined();
    expect(screen.getByText(/30D/)).toBeDefined();
    expect(screen.getByText(/weekly/)).toBeDefined();
  });

  it("calls onLoad when clicked", async () => {
    const user = userEvent.setup();
    const onLoad = vi.fn();
    render(<SavedChartCard chart={chart} onLoad={onLoad} onDelete={() => {}} />, {
      wrapper: createWrapper(),
    });

    await user.click(screen.getByText("My Weekly Chart"));
    expect(onLoad).toHaveBeenCalledOnce();
  });

  it("calls onLoad when Enter pressed on card", async () => {
    const user = userEvent.setup();
    const onLoad = vi.fn();
    render(<SavedChartCard chart={chart} onLoad={onLoad} onDelete={() => {}} />, {
      wrapper: createWrapper(),
    });

    const card = screen.getByText("My Weekly Chart").closest("button");
    card?.focus();
    await user.keyboard("{Enter}");
    expect(onLoad).toHaveBeenCalledOnce();
  });

  it("calls onDelete with confirmation on delete click", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<SavedChartCard chart={chart} onLoad={() => {}} onDelete={onDelete} />, {
      wrapper: createWrapper(),
    });

    await user.click(screen.getByLabelText("Delete chart My Weekly Chart"));
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("does not call onDelete when confirmation is cancelled", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    vi.spyOn(window, "confirm").mockReturnValue(false);

    render(<SavedChartCard chart={chart} onLoad={() => {}} onDelete={onDelete} />, {
      wrapper: createWrapper(),
    });

    await user.click(screen.getByLabelText("Delete chart My Weekly Chart"));
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("shows custom date range for non-preset configs", () => {
    const customChart: SavedChart = {
      ...chart,
      config: {
        ...chart.config,
        range: { start: "2025-01-01", end: "2025-06-01" },
      },
    };
    render(<SavedChartCard chart={customChart} onLoad={() => {}} onDelete={() => {}} />, {
      wrapper: createWrapper(),
    });
    expect(screen.getByText(/2025-01-01 - 2025-06-01/)).toBeDefined();
  });
});
