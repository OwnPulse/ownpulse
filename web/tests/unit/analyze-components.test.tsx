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
import { useAuthStore } from "../../src/store/auth";

// Mock unovis (SVG/D3 doesn't render in jsdom)
vi.mock("@unovis/react", () => ({
  VisXYContainer: ({ children }: { children: ReactNode }) => (
    <div data-testid="xy-container">{children}</div>
  ),
  VisLine: () => <div data-testid="vis-line" />,
  VisScatter: () => <div data-testid="vis-scatter" />,
  VisGroupedBar: () => <div data-testid="vis-grouped-bar" />,
  VisPlotline: () => <div data-testid="vis-plotline" />,
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
  ],
};

const interventionsResponse = [
  {
    id: "int-1",
    user_id: "user-1",
    substance: "Magnesium",
    dose: 400,
    unit: "mg",
    route: "oral",
    administered_at: "2026-01-15T08:00:00Z",
    fasted: false,
    created_at: "2026-01-15T08:00:00Z",
  },
  {
    id: "int-2",
    user_id: "user-1",
    substance: "Creatine",
    dose: 5,
    unit: "g",
    route: "oral",
    administered_at: "2026-01-16T08:00:00Z",
    fasted: false,
    created_at: "2026-01-16T08:00:00Z",
  },
];

const beforeAfterResponse = {
  intervention_substance: "Magnesium",
  first_dose: "2026-01-15T08:00:00Z",
  last_dose: null,
  metric: { source: "checkins", field: "energy" },
  before: {
    mean: 5.2,
    std_dev: 1.1,
    n: 30,
    points: [
      { t: "2026-01-01T00:00:00Z", v: 5 },
      { t: "2026-01-02T00:00:00Z", v: 6 },
    ],
  },
  after: {
    mean: 6.8,
    std_dev: 0.9,
    n: 30,
    points: [
      { t: "2026-01-16T00:00:00Z", v: 7 },
      { t: "2026-01-17T00:00:00Z", v: 6 },
    ],
  },
  change_pct: 30.8,
  p_value: 0.003,
  significant: true,
  test_used: "welch_t",
};

const correlateResponse = {
  metric_a: { source: "checkins", field: "energy" },
  metric_b: { source: "checkins", field: "mood" },
  r: 0.72,
  p_value: 0.001,
  n: 60,
  significant: true,
  method: "pearson",
  interpretation: "Strong positive correlation",
  scatter: [
    { a: 5, b: 6, t: "2026-01-01T00:00:00Z" },
    { a: 7, b: 8, t: "2026-01-02T00:00:00Z" },
  ],
};

const lagCorrelateResponse = {
  metric_a: { source: "checkins", field: "energy" },
  metric_b: { source: "checkins", field: "mood" },
  lags: [
    { lag: 0, r: 0.5, p_value: 0.01, n: 60 },
    { lag: 1, r: 0.72, p_value: 0.001, n: 59 },
    { lag: 2, r: 0.4, p_value: 0.05, n: 58 },
  ],
  best_lag: { lag: 1, r: 0.72, p_value: 0.001, n: 59 },
  method: "pearson",
};

const server = setupServer(
  http.get("/api/v1/explore/metrics", () => {
    return HttpResponse.json(metricsResponse);
  }),
  http.get("/api/v1/interventions", () => {
    return HttpResponse.json(interventionsResponse);
  }),
  http.post("/api/v1/stats/before-after", () => {
    return HttpResponse.json(beforeAfterResponse);
  }),
  http.post("/api/v1/stats/correlate", () => {
    return HttpResponse.json(correlateResponse);
  }),
  http.post("/api/v1/stats/lag-correlate", () => {
    return HttpResponse.json(lagCorrelateResponse);
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

function createWrapperWithRoute(initialRoute: string) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={[initialRoute]}>{children}</MemoryRouter>
      </QueryClientProvider>
    );
  };
}

describe("StatsCard", () => {
  // Dynamically import to avoid hoisting issues with mock
  it("renders items correctly", async () => {
    const { StatsCard } = await import("../../src/components/analyze/StatsCard");
    render(
      <StatsCard
        items={[
          { label: "r", value: "0.72" },
          { label: "p-value", value: "0.001" },
        ]}
        significant={true}
      />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByText("r")).toBeDefined();
    expect(screen.getByText("0.72")).toBeDefined();
    expect(screen.getByText("p-value")).toBeDefined();
    expect(screen.getByText("0.001")).toBeDefined();
    expect(screen.getByTestId("significance").textContent).toContain("Statistically significant");
  });

  it("renders not significant correctly", async () => {
    const { StatsCard } = await import("../../src/components/analyze/StatsCard");
    render(<StatsCard items={[{ label: "r", value: "0.10" }]} significant={false} />, {
      wrapper: createWrapper(),
    });
    expect(screen.getByTestId("significance").textContent).toContain(
      "Not statistically significant",
    );
  });

  it("does not render significance when undefined", async () => {
    const { StatsCard } = await import("../../src/components/analyze/StatsCard");
    render(<StatsCard items={[{ label: "Lags", value: "7" }]} />, { wrapper: createWrapper() });
    expect(screen.queryByTestId("significance")).toBeNull();
  });
});

describe("ScatterChart", () => {
  it("renders scatter chart with data", async () => {
    const { ScatterChart } = await import("../../src/components/analyze/ScatterChart");
    render(
      <ScatterChart
        data={[
          { a: 5, b: 6, t: "2026-01-01T00:00:00Z" },
          { a: 7, b: 8, t: "2026-01-02T00:00:00Z" },
        ]}
        labelA="Energy"
        labelB="Mood"
      />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByTestId("xy-container")).toBeDefined();
    expect(screen.getByTestId("vis-scatter")).toBeDefined();
  });

  it("renders empty message with no data", async () => {
    const { ScatterChart } = await import("../../src/components/analyze/ScatterChart");
    render(<ScatterChart data={[]} labelA="A" labelB="B" />, { wrapper: createWrapper() });
    expect(screen.getByText("No scatter data available.")).toBeDefined();
  });
});

describe("LagChart", () => {
  it("renders bar chart with lag data", async () => {
    const { LagChart } = await import("../../src/components/analyze/LagChart");
    render(
      <LagChart
        lags={[
          { lag: 0, r: 0.5, p_value: 0.01, n: 60 },
          { lag: 1, r: 0.72, p_value: 0.001, n: 59 },
        ]}
        bestLag={1}
      />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByTestId("xy-container")).toBeDefined();
    expect(screen.getByTestId("vis-grouped-bar")).toBeDefined();
  });

  it("renders empty message with no data", async () => {
    const { LagChart } = await import("../../src/components/analyze/LagChart");
    render(<LagChart lags={[]} bestLag={0} />, { wrapper: createWrapper() });
    expect(screen.getByText("No lag data available.")).toBeDefined();
  });
});

describe("BeforeAfterForm", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders form fields", async () => {
    const { BeforeAfterForm } = await import("../../src/components/analyze/BeforeAfterForm");
    render(<BeforeAfterForm />, { wrapper: createWrapper() });

    expect(screen.getByLabelText("Substance")).toBeDefined();
    expect(screen.getByLabelText("Metric")).toBeDefined();
    expect(screen.getByLabelText("Before (days)")).toBeDefined();
    expect(screen.getByLabelText("After (days)")).toBeDefined();
    expect(screen.getByRole("button", { name: /analyze/i })).toBeDefined();
  });

  it("disables submit when fields are empty", async () => {
    const { BeforeAfterForm } = await import("../../src/components/analyze/BeforeAfterForm");
    render(<BeforeAfterForm />, { wrapper: createWrapper() });

    const btn = screen.getByRole("button", { name: /analyze/i });
    expect(btn.hasAttribute("disabled")).toBe(true);
  });

  it("submits and shows results", async () => {
    const user = userEvent.setup();
    const { BeforeAfterForm } = await import("../../src/components/analyze/BeforeAfterForm");
    render(<BeforeAfterForm />, { wrapper: createWrapper() });

    // Wait for metrics and substances to load
    await waitFor(() => {
      expect(screen.getByLabelText("Metric").querySelectorAll("option").length).toBeGreaterThan(1);
    });

    // Fill substance
    await user.type(screen.getByLabelText("Substance"), "Magnesium");

    // Select metric
    await user.selectOptions(screen.getByLabelText("Metric"), "checkins:energy");

    // Submit
    await user.click(screen.getByRole("button", { name: /analyze/i }));

    // Wait for results
    await waitFor(() => {
      expect(screen.getByTestId("ba-results")).toBeDefined();
    });

    expect(screen.getByText("5.20")).toBeDefined();
    expect(screen.getByText("6.80")).toBeDefined();
    expect(screen.getByText("+30.8%")).toBeDefined();
    expect(screen.getByText("Correlation does not imply causation.")).toBeDefined();
  });

  it("shows error on failure", async () => {
    server.use(
      http.post("/api/v1/stats/before-after", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    const user = userEvent.setup();
    const { BeforeAfterForm } = await import("../../src/components/analyze/BeforeAfterForm");
    render(<BeforeAfterForm />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText("Metric").querySelectorAll("option").length).toBeGreaterThan(1);
    });

    await user.type(screen.getByLabelText("Substance"), "Magnesium");
    await user.selectOptions(screen.getByLabelText("Metric"), "checkins:energy");
    await user.click(screen.getByRole("button", { name: /analyze/i }));

    await waitFor(() => {
      expect(screen.getByTestId("ba-error")).toBeDefined();
    });
  });
});

describe("CorrelationForm", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders form fields", async () => {
    const { CorrelationForm } = await import("../../src/components/analyze/CorrelationForm");
    render(<CorrelationForm />, { wrapper: createWrapper() });

    expect(screen.getByLabelText("Metric A")).toBeDefined();
    expect(screen.getByLabelText("Metric B")).toBeDefined();
    expect(screen.getByLabelText("Start Date")).toBeDefined();
    expect(screen.getByLabelText("End Date")).toBeDefined();
    expect(screen.getByRole("button", { name: "Pearson" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Spearman" })).toBeDefined();
    expect(screen.getByRole("button", { name: /correlate/i })).toBeDefined();
  });

  it("renders with initial metrics from URL params", async () => {
    const { CorrelationForm } = await import("../../src/components/analyze/CorrelationForm");
    render(
      <CorrelationForm
        initialMetricA={{ source: "checkins", field: "energy" }}
        initialMetricB={{ source: "checkins", field: "mood" }}
      />,
      { wrapper: createWrapper() },
    );

    await waitFor(() => {
      const metricA = screen.getByLabelText("Metric A") as HTMLSelectElement;
      expect(metricA.value).toBe("checkins:energy");
    });
  });

  it("submits and shows results", async () => {
    const user = userEvent.setup();
    const { CorrelationForm } = await import("../../src/components/analyze/CorrelationForm");
    render(<CorrelationForm />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText("Metric A").querySelectorAll("option").length).toBeGreaterThan(
        1,
      );
    });

    await user.selectOptions(screen.getByLabelText("Metric A"), "checkins:energy");
    await user.selectOptions(screen.getByLabelText("Metric B"), "checkins:mood");

    const startInput = screen.getByLabelText("Start Date");
    const endInput = screen.getByLabelText("End Date");
    await user.type(startInput, "2026-01-01");
    await user.type(endInput, "2026-03-01");

    await user.click(screen.getByRole("button", { name: /^correlate$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("corr-results")).toBeDefined();
    });

    expect(screen.getByText("0.7200")).toBeDefined();
    expect(screen.getByText("Strong positive correlation")).toBeDefined();
    expect(screen.getByText("Correlation does not imply causation.")).toBeDefined();
  });

  it("shows error on failure", async () => {
    server.use(
      http.post("/api/v1/stats/correlate", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    const user = userEvent.setup();
    const { CorrelationForm } = await import("../../src/components/analyze/CorrelationForm");
    render(<CorrelationForm />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText("Metric A").querySelectorAll("option").length).toBeGreaterThan(
        1,
      );
    });

    await user.selectOptions(screen.getByLabelText("Metric A"), "checkins:energy");
    await user.selectOptions(screen.getByLabelText("Metric B"), "checkins:mood");
    await user.type(screen.getByLabelText("Start Date"), "2026-01-01");
    await user.type(screen.getByLabelText("End Date"), "2026-03-01");
    await user.click(screen.getByRole("button", { name: /^correlate$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("corr-error")).toBeDefined();
    });
  });

  it("toggles method between Pearson and Spearman", async () => {
    const user = userEvent.setup();
    const { CorrelationForm } = await import("../../src/components/analyze/CorrelationForm");
    render(<CorrelationForm />, { wrapper: createWrapper() });

    const pearsonBtn = screen.getByRole("button", { name: "Pearson" });
    const spearmanBtn = screen.getByRole("button", { name: "Spearman" });

    expect(pearsonBtn.getAttribute("aria-pressed")).toBe("true");
    expect(spearmanBtn.getAttribute("aria-pressed")).toBe("false");

    await user.click(spearmanBtn);
    expect(spearmanBtn.getAttribute("aria-pressed")).toBe("true");
    expect(pearsonBtn.getAttribute("aria-pressed")).toBe("false");
  });
});

describe("LagCorrelationForm", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders form fields", async () => {
    const { LagCorrelationForm } = await import("../../src/components/analyze/LagCorrelationForm");
    render(<LagCorrelationForm />, { wrapper: createWrapper() });

    expect(screen.getByLabelText("Metric A")).toBeDefined();
    expect(screen.getByLabelText("Metric B")).toBeDefined();
    expect(screen.getByLabelText("Start Date")).toBeDefined();
    expect(screen.getByLabelText("End Date")).toBeDefined();
    expect(screen.getByLabelText("Max Lag (days)")).toBeDefined();
    expect(screen.getByRole("button", { name: /analyze lag/i })).toBeDefined();
  });

  it("submits and shows results", async () => {
    const user = userEvent.setup();
    const { LagCorrelationForm } = await import("../../src/components/analyze/LagCorrelationForm");
    render(<LagCorrelationForm />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText("Metric A").querySelectorAll("option").length).toBeGreaterThan(
        1,
      );
    });

    await user.selectOptions(screen.getByLabelText("Metric A"), "checkins:energy");
    await user.selectOptions(screen.getByLabelText("Metric B"), "checkins:mood");
    await user.type(screen.getByLabelText("Start Date"), "2026-01-01");
    await user.type(screen.getByLabelText("End Date"), "2026-03-01");

    await user.click(screen.getByRole("button", { name: /analyze lag/i }));

    await waitFor(() => {
      expect(screen.getByTestId("lag-results")).toBeDefined();
    });

    expect(screen.getByText("1 days")).toBeDefined();
    expect(screen.getByText("0.7200")).toBeDefined();
    expect(screen.getByText("Correlation does not imply causation.")).toBeDefined();
  });

  it("shows error on failure", async () => {
    server.use(
      http.post("/api/v1/stats/lag-correlate", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    const user = userEvent.setup();
    const { LagCorrelationForm } = await import("../../src/components/analyze/LagCorrelationForm");
    render(<LagCorrelationForm />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText("Metric A").querySelectorAll("option").length).toBeGreaterThan(
        1,
      );
    });

    await user.selectOptions(screen.getByLabelText("Metric A"), "checkins:energy");
    await user.selectOptions(screen.getByLabelText("Metric B"), "checkins:mood");
    await user.type(screen.getByLabelText("Start Date"), "2026-01-01");
    await user.type(screen.getByLabelText("End Date"), "2026-03-01");
    await user.click(screen.getByRole("button", { name: /analyze lag/i }));

    await waitFor(() => {
      expect(screen.getByTestId("lag-error")).toBeDefined();
    });
  });
});

describe("Analyze page", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders with tabs and default to Before/After", async () => {
    const Analyze = (await import("../../src/pages/Analyze")).default;
    render(<Analyze />, { wrapper: createWrapper() });

    expect(screen.getByRole("tab", { name: "Before / After" })).toBeDefined();
    expect(screen.getByRole("tab", { name: "Correlation" })).toBeDefined();
    expect(screen.getByRole("tab", { name: "Lag Correlation" })).toBeDefined();

    // Before/After tab should be active by default
    expect(screen.getByRole("tab", { name: "Before / After" }).getAttribute("aria-selected")).toBe(
      "true",
    );

    // Before/After form should be visible
    expect(screen.getByLabelText("Substance")).toBeDefined();
  });

  it("switches tabs when clicked", async () => {
    const user = userEvent.setup();
    const Analyze = (await import("../../src/pages/Analyze")).default;
    render(<Analyze />, { wrapper: createWrapper() });

    await user.click(screen.getByRole("tab", { name: "Correlation" }));
    expect(screen.getByRole("tab", { name: "Correlation" }).getAttribute("aria-selected")).toBe(
      "true",
    );

    // Correlation form should be visible
    await waitFor(() => {
      expect(screen.getByLabelText("Metric A")).toBeDefined();
    });
  });

  it("reads initial mode and metrics from URL params", async () => {
    const Analyze = (await import("../../src/pages/Analyze")).default;
    render(<Analyze />, {
      wrapper: createWrapperWithRoute(
        "/analyze?mode=correlation&metricA=checkins:energy&metricB=checkins:mood",
      ),
    });

    expect(screen.getByRole("tab", { name: "Correlation" }).getAttribute("aria-selected")).toBe(
      "true",
    );
  });
});

describe("Correlate button on Explore", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("does not show Correlate button with 0 or 1 metrics selected", async () => {
    const { useExploreStore } = await import("../../src/stores/exploreStore");
    useExploreStore.setState({
      selectedMetrics: [{ source: "checkins", field: "energy" }],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });

    // We need to also mock the batch series endpoint
    server.use(
      http.post("/api/v1/explore/series", () => {
        return HttpResponse.json({ series: [] });
      }),
      http.get("/api/v1/explore/charts", () => {
        return HttpResponse.json([]);
      }),
    );

    const Explore = (await import("../../src/pages/Explore")).default;
    render(<Explore />, { wrapper: createWrapper() });

    expect(screen.queryByRole("button", { name: "Correlate" })).toBeNull();
  });

  it("shows Correlate button when 2+ metrics are selected", async () => {
    const { useExploreStore } = await import("../../src/stores/exploreStore");
    useExploreStore.setState({
      selectedMetrics: [
        { source: "checkins", field: "energy" },
        { source: "checkins", field: "mood" },
      ],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });

    server.use(
      http.post("/api/v1/explore/series", () => {
        return HttpResponse.json({ series: [] });
      }),
      http.get("/api/v1/explore/charts", () => {
        return HttpResponse.json([]);
      }),
    );

    const Explore = (await import("../../src/pages/Explore")).default;
    render(<Explore />, { wrapper: createWrapper() });

    expect(screen.getByRole("button", { name: "Correlate" })).toBeDefined();
  });
});
