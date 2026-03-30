// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { Insight } from "../../src/api/insights";
import { InsightCard } from "../../src/components/dashboard/InsightCard";
import { InsightCards } from "../../src/components/dashboard/InsightCards";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

const SAMPLE_INSIGHTS: Insight[] = [
  {
    id: "i1",
    insight_type: "trend",
    headline: "Energy trending up 15%",
    detail: "Average went from 5.2 to 6.0",
    metadata: { explore_params: { source: "checkins", field: "energy", preset: "30d" } },
    created_at: "2026-03-28T06:00:00Z",
  },
  {
    id: "i2",
    insight_type: "streak",
    headline: "14-day check-in streak!",
    detail: null,
    metadata: {},
    created_at: "2026-03-28T06:00:00Z",
  },
  {
    id: "i3",
    insight_type: "anomaly",
    headline: "Sleep score dropped sharply",
    detail: "Last night was 42, your average is 78",
    metadata: { explore_params: { source: "sleep", field: "score" } },
    created_at: "2026-03-28T06:00:00Z",
  },
  {
    id: "i4",
    insight_type: "missing_data",
    headline: "No check-in for 3 days",
    detail: null,
    metadata: {},
    created_at: "2026-03-28T06:00:00Z",
  },
  {
    id: "i5",
    insight_type: "correlation",
    headline: "Mood correlates with sleep duration",
    detail: "r=0.72 over 30 days",
    metadata: { explore_params: { source: "checkins", field: "mood", preset: "30d" } },
    created_at: "2026-03-28T06:00:00Z",
  },
];

const server = setupServer(
  http.get("/api/v1/insights", () => {
    return HttpResponse.json(SAMPLE_INSIGHTS);
  }),
  http.post("/api/v1/insights/:id/dismiss", () => {
    return new HttpResponse(null, { status: 204 });
  }),
  http.post("/api/v1/insights/generate", () => {
    return HttpResponse.json(SAMPLE_INSIGHTS);
  }),
);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function createWrapper() {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    const qc = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    return (
      <QueryClientProvider client={qc}>
        <MemoryRouter>{children}</MemoryRouter>
      </QueryClientProvider>
    );
  };
}

describe("InsightCards", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders loading state", () => {
    render(<InsightCards />, { wrapper: createWrapper() });
    expect(screen.getByText("Loading insights...")).toBeDefined();
  });

  it("renders insight list after loading", async () => {
    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy trending up 15%")).toBeDefined();
    });
    expect(screen.getByText("14-day check-in streak!")).toBeDefined();
    expect(screen.getByText("Sleep score dropped sharply")).toBeDefined();
    expect(screen.getByText("No check-in for 3 days")).toBeDefined();
    expect(screen.getByText("Mood correlates with sleep duration")).toBeDefined();
  });

  it("renders empty state when no insights", async () => {
    server.use(
      http.get("/api/v1/insights", () => {
        return HttpResponse.json([]);
      }),
    );

    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("No insights right now. Check back later.")).toBeDefined();
    });
  });

  it("renders error state when API fails", async () => {
    server.use(
      http.get("/api/v1/insights", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Failed to load insights.")).toBeDefined();
    });
  });

  it("dismisses an insight card", async () => {
    const user = userEvent.setup();
    let dismissedId: string | null = null;

    server.use(
      http.post("/api/v1/insights/:id/dismiss", ({ params }) => {
        dismissedId = params.id as string;
        return new HttpResponse(null, { status: 204 });
      }),
    );

    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy trending up 15%")).toBeDefined();
    });

    const dismissBtn = screen.getByLabelText("Dismiss insight: Energy trending up 15%");
    await user.click(dismissBtn);

    await waitFor(() => {
      expect(dismissedId).toBe("i1");
    });
  });

  it("refresh button calls generate and refetches list", async () => {
    const user = userEvent.setup();
    let generateCalled = false;

    server.use(
      http.post("/api/v1/insights/generate", () => {
        generateCalled = true;
        return HttpResponse.json(SAMPLE_INSIGHTS);
      }),
    );

    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy trending up 15%")).toBeDefined();
    });

    const refreshBtn = screen.getByLabelText("Refresh insights");
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(generateCalled).toBe(true);
    });
  });

  it("shows spinner while generating", async () => {
    const user = userEvent.setup();

    // Make generate hang so we can observe the spinner
    server.use(
      http.post("/api/v1/insights/generate", async () => {
        await new Promise((resolve) => setTimeout(resolve, 500));
        return HttpResponse.json(SAMPLE_INSIGHTS);
      }),
    );

    render(<InsightCards />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("Energy trending up 15%")).toBeDefined();
    });

    const refreshBtn = screen.getByLabelText("Refresh insights");
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(screen.getByTestId("refresh-spinner")).toBeDefined();
    });
  });

  it("has correct section aria-label", async () => {
    render(<InsightCards />, { wrapper: createWrapper() });

    const section = screen.getByRole("region", { name: "Insights" });
    expect(section).toBeDefined();
  });
});

describe("InsightCard", () => {
  let dismissedId: string | null = null;
  const mockDismiss = (id: string) => {
    dismissedId = id;
  };

  beforeEach(() => {
    dismissedId = null;
  });

  function renderCard(insight: Insight) {
    const qc = new QueryClient();
    return render(
      <QueryClientProvider client={qc}>
        <MemoryRouter>
          <InsightCard insight={insight} onDismiss={mockDismiss} />
        </MemoryRouter>
      </QueryClientProvider>,
    );
  }

  it("renders headline and detail", () => {
    renderCard(SAMPLE_INSIGHTS[0]);
    expect(screen.getByText("Energy trending up 15%")).toBeDefined();
    expect(screen.getByText("Average went from 5.2 to 6.0")).toBeDefined();
  });

  it("renders headline without detail when detail is null", () => {
    renderCard(SAMPLE_INSIGHTS[1]);
    expect(screen.getByText("14-day check-in streak!")).toBeDefined();
  });

  it("renders type tag for trend", () => {
    renderCard(SAMPLE_INSIGHTS[0]);
    expect(screen.getByText(/Trend/)).toBeDefined();
  });

  it("renders type tag for anomaly", () => {
    renderCard(SAMPLE_INSIGHTS[2]);
    expect(screen.getByText(/Anomaly/)).toBeDefined();
  });

  it("renders type tag for missing_data", () => {
    renderCard(SAMPLE_INSIGHTS[3]);
    expect(screen.getByText(/Missing/)).toBeDefined();
  });

  it("renders type tag for streak", () => {
    renderCard(SAMPLE_INSIGHTS[1]);
    expect(screen.getByText(/Streak/)).toBeDefined();
  });

  it("renders type tag for correlation", () => {
    renderCard(SAMPLE_INSIGHTS[4]);
    expect(screen.getByText(/Correlation/)).toBeDefined();
  });

  it("renders 'View in Explore' link with correct params for trend", () => {
    renderCard(SAMPLE_INSIGHTS[0]);
    const link = screen.getByText(/View in Explore/);
    expect(link).toBeDefined();
    expect(link.getAttribute("href")).toBe("/explore?source=checkins&field=energy&preset=30d");
  });

  it("renders 'View in Analyze' link for correlation type", () => {
    renderCard(SAMPLE_INSIGHTS[4]);
    const link = screen.getByText(/View in Analyze/);
    expect(link).toBeDefined();
    expect(link.getAttribute("href")).toBe("/analyze?source=checkins&field=mood&preset=30d");
  });

  it("does not render explore link when metadata has no explore_params", () => {
    renderCard(SAMPLE_INSIGHTS[1]);
    expect(screen.queryByText(/View in/)).toBeNull();
  });

  it("calls onDismiss with correct id when dismiss button clicked", async () => {
    const user = userEvent.setup();
    renderCard(SAMPLE_INSIGHTS[0]);

    const dismissBtn = screen.getByLabelText("Dismiss insight: Energy trending up 15%");
    await user.click(dismissBtn);

    expect(dismissedId).toBe("i1");
  });

  it("applies dismissed styling after dismiss click", async () => {
    const user = userEvent.setup();
    renderCard(SAMPLE_INSIGHTS[0]);

    const card = screen.getByTestId("insight-card-i1");
    expect(card.className).not.toContain("cardDismissed");

    const dismissBtn = screen.getByLabelText("Dismiss insight: Energy trending up 15%");
    await user.click(dismissBtn);

    expect(card.className).toContain("cardDismissed");
  });
});
