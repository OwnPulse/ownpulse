// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import Genetics from "../../src/pages/Genetics";
import { useAuthStore } from "../../src/store/auth";

const emptySummary = {
  total_variants: 0,
  source: null,
  uploaded_at: null,
  chromosomes: {},
  annotated_count: 0,
};

const populatedSummary = {
  total_variants: 650000,
  source: "23andMe",
  uploaded_at: "2026-03-20T10:00:00Z",
  chromosomes: { "1": 50000, "22": 30000 },
  annotated_count: 42,
};

const interpretationsResponse = {
  interpretations: [
    {
      rsid: "rs1801133",
      gene: "MTHFR",
      chromosome: "1",
      position: 11856378,
      user_genotype: "CT",
      category: "health_risk",
      title: "MTHFR C677T Variant",
      summary: "You carry one copy.",
      risk_level: "moderate",
      significance: "Reduced folate metabolism",
      evidence_level: "strong",
      source: "ClinVar",
      source_id: "3520",
      population_frequency: 0.34,
      details: {},
    },
  ],
  disclaimer: "For educational purposes only.",
};

const server = setupServer(
  http.get("/api/v1/genetics/summary", () => {
    return HttpResponse.json(emptySummary);
  }),
  http.get("/api/v1/genetics/interpretations", () => {
    return HttpResponse.json(interpretationsResponse);
  }),
  http.get("/api/v1/genetics", () => {
    return HttpResponse.json({ records: [], total: 0, page: 1, per_page: 50 });
  }),
  http.post("/api/v1/genetics/upload", () => {
    return HttpResponse.json({
      total_variants: 650000,
      new_variants: 649500,
      duplicates_skipped: 500,
      format: "23andMe_v5",
      source: "23andMe",
    });
  }),
  http.delete("/api/v1/genetics", () => {
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <Genetics />
    </QueryClientProvider>,
  );
}

describe("Genetics page", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders loading state", () => {
    renderPage();
    expect(screen.getByText("Loading genetic data...")).toBeDefined();
  });

  it("shows upload dropzone when no data uploaded", async () => {
    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Upload your genetic data")).toBeDefined();
    });
    expect(screen.getByText(/Supports 23andMe/)).toBeDefined();
  });

  it("does not show interpretations or browser when no data", async () => {
    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Upload your genetic data")).toBeDefined();
    });
    expect(screen.queryByText("Interpretations")).toBeNull();
    expect(screen.queryByText("Raw Data Browser")).toBeNull();
  });

  it("shows summary card and interpretations when data exists", async () => {
    server.use(
      http.get("/api/v1/genetics/summary", () => {
        return HttpResponse.json(populatedSummary);
      }),
    );

    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Genetic Data Summary")).toBeDefined();
    });
    expect(screen.getByText("650,000")).toBeDefined();
    expect(screen.getByText("Interpretations")).toBeDefined();
    expect(screen.getByText("Raw Data Browser")).toBeDefined();
  });

  it("shows compact upload and delete button when data exists", async () => {
    server.use(
      http.get("/api/v1/genetics/summary", () => {
        return HttpResponse.json(populatedSummary);
      }),
    );

    renderPage();

    await waitFor(() => {
      expect(screen.getByText("Upload new file")).toBeDefined();
    });
    expect(screen.getByText("Delete All Data")).toBeDefined();
  });

  it("shows delete confirmation modal", async () => {
    server.use(
      http.get("/api/v1/genetics/summary", () => {
        return HttpResponse.json(populatedSummary);
      }),
    );

    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Delete All Data")).toBeDefined();
    });

    await user.click(screen.getByText("Delete All Data"));

    expect(screen.getByTestId("delete-modal")).toBeDefined();
    expect(screen.getByText("Delete all genetic data?")).toBeDefined();
    expect(screen.getByText(/cannot be undone/)).toBeDefined();
  });

  it("cancels delete confirmation", async () => {
    server.use(
      http.get("/api/v1/genetics/summary", () => {
        return HttpResponse.json(populatedSummary);
      }),
    );

    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Delete All Data")).toBeDefined();
    });

    await user.click(screen.getByText("Delete All Data"));
    await user.click(screen.getByText("Cancel"));

    expect(screen.queryByTestId("delete-modal")).toBeNull();
  });

  it("shows error state when summary fetch fails", async () => {
    server.use(
      http.get("/api/v1/genetics/summary", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    renderPage();

    await waitFor(() => {
      expect(screen.getByText(/Failed to load genetic data/)).toBeDefined();
    });
  });
});
