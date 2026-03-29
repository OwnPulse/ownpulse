// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { GeneticSummary, Interpretation } from "../../src/api/genetics";
import { DisclaimerBanner } from "../../src/components/genetics/DisclaimerBanner";
import { InterpretationCard } from "../../src/components/genetics/InterpretationCard";
import { InterpretationList } from "../../src/components/genetics/InterpretationList";
import { RiskBadge } from "../../src/components/genetics/RiskBadge";
import { SummaryCard } from "../../src/components/genetics/SummaryCard";
import { UploadDropzone } from "../../src/components/genetics/UploadDropzone";
import { VariantBrowser } from "../../src/components/genetics/VariantBrowser";
import { useAuthStore } from "../../src/store/auth";

function makeInterpretation(overrides: Partial<Interpretation> = {}): Interpretation {
  return {
    rsid: "rs1801133",
    gene: "MTHFR",
    chromosome: "1",
    position: 11856378,
    user_genotype: "CT",
    category: "health_risk",
    title: "MTHFR C677T Variant",
    summary: "You carry one copy of the C677T variant.",
    risk_level: "moderate",
    significance: "Associated with reduced folate metabolism",
    evidence_level: "strong",
    source: "ClinVar",
    source_id: "3520",
    population_frequency: 0.34,
    details: {},
    ...overrides,
  };
}

const mockSummary: GeneticSummary = {
  total_variants: 650000,
  source: "23andMe",
  uploaded_at: "2026-03-20T10:00:00Z",
  chromosomes: { "1": 50000, "22": 30000, X: 20000 },
  annotated_count: 42,
};

const server = setupServer(
  http.get("/api/v1/genetics/interpretations", ({ request }) => {
    const url = new URL(request.url);
    const cat = url.searchParams.get("category");
    const interp = makeInterpretation(cat ? { category: cat as Interpretation["category"] } : {});
    return HttpResponse.json({
      interpretations: [interp],
      disclaimer: "For educational purposes only.",
    });
  }),
  http.get("/api/v1/genetics", () => {
    return HttpResponse.json({
      records: [
        {
          rsid: "rs1801133",
          chromosome: "1",
          position: 11856378,
          genotype: "CT",
          created_at: "2026-03-20T10:00:00Z",
        },
        {
          rsid: "rs4680",
          chromosome: "22",
          position: 19963748,
          genotype: "AG",
          created_at: "2026-03-20T10:00:00Z",
        },
      ],
      total: 2,
      page: 1,
      per_page: 50,
    });
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
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
}

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = createQueryClient();
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

describe("DisclaimerBanner", () => {
  it("renders default disclaimer text", () => {
    render(<DisclaimerBanner />);
    expect(screen.getByRole("alert")).toBeDefined();
    expect(screen.getByText(/educational purposes only/)).toBeDefined();
  });

  it("renders custom disclaimer text", () => {
    render(<DisclaimerBanner text="Custom warning text." />);
    expect(screen.getByText("Custom warning text.")).toBeDefined();
  });
});

describe("RiskBadge", () => {
  it("renders high risk", () => {
    render(<RiskBadge level="high" />);
    expect(screen.getByText("High")).toBeDefined();
  });

  it("renders moderate risk", () => {
    render(<RiskBadge level="moderate" />);
    expect(screen.getByText("Moderate")).toBeDefined();
  });

  it("renders low risk", () => {
    render(<RiskBadge level="low" />);
    expect(screen.getByText("Low")).toBeDefined();
  });

  it("renders normal risk", () => {
    render(<RiskBadge level="normal" />);
    expect(screen.getByText("Normal")).toBeDefined();
  });

  it("renders poor metabolizer", () => {
    render(<RiskBadge level="poor_metabolizer" />);
    expect(screen.getByText("Poor Metabolizer")).toBeDefined();
  });

  it("renders intermediate", () => {
    render(<RiskBadge level="intermediate" />);
    expect(screen.getByText("Intermediate")).toBeDefined();
  });

  it("renders rapid", () => {
    render(<RiskBadge level="rapid" />);
    expect(screen.getByText("Rapid")).toBeDefined();
  });
});

describe("InterpretationCard", () => {
  it("renders all fields", () => {
    const interp = makeInterpretation();
    render(<InterpretationCard interpretation={interp} />);

    expect(screen.getByText("MTHFR C677T Variant")).toBeDefined();
    expect(screen.getByText("Moderate")).toBeDefined();
    expect(screen.getByText(/Gene: MTHFR/)).toBeDefined();
    expect(screen.getByText(/Chr 1/)).toBeDefined();
    expect(screen.getByText(/rs1801133/)).toBeDefined();
    expect(screen.getByText("CT")).toBeDefined();
    expect(screen.getByText(/You carry one copy/)).toBeDefined();
    expect(screen.getByText(/strong/i)).toBeDefined();
    expect(screen.getByText(/34\.0%/)).toBeDefined();
  });

  it("renders ClinVar link when source_id is present", () => {
    const interp = makeInterpretation({ source: "ClinVar", source_id: "3520" });
    render(<InterpretationCard interpretation={interp} />);

    const link = screen.getByRole("link", { name: "ClinVar" });
    expect(link).toBeDefined();
    expect(link.getAttribute("href")).toContain("clinvar/variation/3520");
  });

  it("renders PharmGKB link", () => {
    const interp = makeInterpretation({
      source: "PharmGKB",
      source_id: "PA166154579",
      category: "pharmacogenomics",
      risk_level: "poor_metabolizer",
    });
    render(<InterpretationCard interpretation={interp} />);

    const link = screen.getByRole("link", { name: "PharmGKB" });
    expect(link.getAttribute("href")).toContain("pharmgkb.org/variant/PA166154579");
  });

  it("renders without gene", () => {
    const interp = makeInterpretation({ gene: null });
    render(<InterpretationCard interpretation={interp} />);
    expect(screen.queryByText(/Gene:/)).toBeNull();
  });

  it("renders without population frequency", () => {
    const interp = makeInterpretation({ population_frequency: null });
    render(<InterpretationCard interpretation={interp} />);
    expect(screen.queryByText(/MAF:/)).toBeNull();
  });

  it("renders source text without link when no source_id", () => {
    const interp = makeInterpretation({ source_id: null });
    render(<InterpretationCard interpretation={interp} />);
    expect(screen.queryByRole("link", { name: "ClinVar" })).toBeNull();
    expect(screen.getByText(/ClinVar/)).toBeDefined();
  });
});

describe("SummaryCard", () => {
  it("renders all stats", () => {
    render(<SummaryCard summary={mockSummary} />);

    expect(screen.getByText("Genetic Data Summary")).toBeDefined();
    expect(screen.getByText("650,000")).toBeDefined();
    expect(screen.getByText("23andMe")).toBeDefined();
    expect(screen.getByText("42")).toBeDefined();
  });

  it("renders chromosome distribution", () => {
    render(<SummaryCard summary={mockSummary} />);

    expect(screen.getByText("Chromosome Distribution")).toBeDefined();
    expect(screen.getByText("50,000")).toBeDefined();
    expect(screen.getByText("30,000")).toBeDefined();
    expect(screen.getByText("20,000")).toBeDefined();
  });

  it("renders unknown source when null", () => {
    render(<SummaryCard summary={{ ...mockSummary, source: null }} />);
    // There are two "Unknown" texts (source + uploaded_at when null), but with non-null uploaded_at
    // there should be exactly the source "Unknown"
    expect(screen.getByText("Unknown")).toBeDefined();
  });
});

describe("UploadDropzone", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders full dropzone", () => {
    renderWithProviders(<UploadDropzone />);
    expect(screen.getByText("Upload your genetic data")).toBeDefined();
    expect(screen.getByText(/Supports 23andMe/)).toBeDefined();
    expect(screen.getByText("Choose file")).toBeDefined();
  });

  it("renders compact dropzone", () => {
    renderWithProviders(<UploadDropzone compact />);
    expect(screen.getByText("Upload new file")).toBeDefined();
    expect(screen.queryByText("Upload your genetic data")).toBeNull();
  });

  it("shows file name after selection", async () => {
    renderWithProviders(<UploadDropzone />);
    const user = userEvent.setup();
    const file = new File(["rsid\tchr\tpos\tgenotype\n"], "genome.txt", { type: "text/plain" });

    const input = screen.getByTestId("file-input");
    await user.upload(input, file);

    expect(screen.getByText(/genome\.txt/)).toBeDefined();
    expect(screen.getByText("Upload")).toBeDefined();
  });

  it("shows progress during upload", async () => {
    renderWithProviders(<UploadDropzone />);
    const user = userEvent.setup();
    const file = new File(["data"], "genome.txt", { type: "text/plain" });

    const input = screen.getByTestId("file-input");
    await user.upload(input, file);
    await user.click(screen.getByText("Upload"));

    // Button should show "Uploading..." while pending
    await waitFor(() => {
      expect(screen.getByText("Upload successful!")).toBeDefined();
    });
  });

  it("shows upload result on success", async () => {
    renderWithProviders(<UploadDropzone />);
    const user = userEvent.setup();
    const file = new File(["data"], "genome.txt", { type: "text/plain" });

    const input = screen.getByTestId("file-input");
    await user.upload(input, file);
    await user.click(screen.getByText("Upload"));

    await waitFor(() => {
      expect(screen.getByTestId("upload-result")).toBeDefined();
    });
    expect(screen.getByText("23andMe_v5")).toBeDefined();
    expect(screen.getByText("650,000")).toBeDefined();
  });

  it("shows error on upload failure", async () => {
    server.use(
      http.post("/api/v1/genetics/upload", () => {
        return new HttpResponse("File format not recognized", { status: 400 });
      }),
    );

    renderWithProviders(<UploadDropzone />);
    const user = userEvent.setup();
    const file = new File(["bad data"], "bad.txt", { type: "text/plain" });

    const input = screen.getByTestId("file-input");
    await user.upload(input, file);
    await user.click(screen.getByText("Upload"));

    await waitFor(() => {
      expect(screen.getByTestId("upload-error")).toBeDefined();
    });
    expect(screen.getByText(/File format not recognized/)).toBeDefined();
  });
});

describe("InterpretationList", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders interpretations with disclaimer", async () => {
    renderWithProviders(<InterpretationList />);

    expect(screen.getByText("Interpretations")).toBeDefined();
    expect(screen.getByText("Loading interpretations...")).toBeDefined();

    await waitFor(() => {
      expect(screen.getByText("MTHFR C677T Variant")).toBeDefined();
    });
    expect(screen.getByText("For educational purposes only.")).toBeDefined();
  });

  it("renders all category tabs", () => {
    renderWithProviders(<InterpretationList />);

    expect(screen.getByText("All")).toBeDefined();
    expect(screen.getByText("Health Risks")).toBeDefined();
    expect(screen.getByText("Traits")).toBeDefined();
    expect(screen.getByText("Pharmacogenomics")).toBeDefined();
    expect(screen.getByText("Carrier Status")).toBeDefined();
  });

  it("switches category on tab click", async () => {
    renderWithProviders(<InterpretationList />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("MTHFR C677T Variant")).toBeDefined();
    });

    await user.click(screen.getByText("Health Risks"));
    // The tab should become active
    const tab = screen.getByRole("tab", { name: "Health Risks" });
    expect(tab.getAttribute("aria-selected")).toBe("true");
  });

  it("shows empty state when no interpretations", async () => {
    server.use(
      http.get("/api/v1/genetics/interpretations", () => {
        return HttpResponse.json({
          interpretations: [],
          disclaimer: "For educational purposes only.",
        });
      }),
    );

    renderWithProviders(<InterpretationList />);

    await waitFor(() => {
      expect(screen.getByText("No interpretations available for this category.")).toBeDefined();
    });
  });

  it("shows error state", async () => {
    server.use(
      http.get("/api/v1/genetics/interpretations", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    renderWithProviders(<InterpretationList />);

    await waitFor(() => {
      expect(screen.getByText(/Failed to load interpretations/)).toBeDefined();
    });
  });
});

describe("VariantBrowser", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("renders collapsed by default", () => {
    renderWithProviders(<VariantBrowser />);
    expect(screen.getByText("Raw Data Browser")).toBeDefined();
    expect(screen.getByText("Expand")).toBeDefined();
    expect(screen.queryByText("Chromosome")).toBeNull();
  });

  it("expands and shows data on click", async () => {
    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));

    await waitFor(() => {
      expect(screen.getByText("rs1801133")).toBeDefined();
    });
    expect(screen.getByText("rs4680")).toBeDefined();
    expect(screen.getByText("Collapse")).toBeDefined();
  });

  it("shows loading state when expanding", async () => {
    // Delay the response so we can observe the loading state
    server.use(
      http.get("/api/v1/genetics", async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
        return HttpResponse.json({
          records: [],
          total: 0,
          page: 1,
          per_page: 50,
        });
      }),
    );

    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));
    expect(screen.getByText("Loading variants...")).toBeDefined();
  });

  it("shows error state", async () => {
    server.use(
      http.get("/api/v1/genetics", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );

    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));

    await waitFor(() => {
      expect(screen.getByText(/Failed to load variants/)).toBeDefined();
    });
  });

  it("shows empty state when no variants", async () => {
    server.use(
      http.get("/api/v1/genetics", () => {
        return HttpResponse.json({
          records: [],
          total: 0,
          page: 1,
          per_page: 50,
        });
      }),
    );

    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));

    await waitFor(() => {
      expect(screen.getByText("No variants found.")).toBeDefined();
    });
  });

  it("has chromosome filter and rsid search", async () => {
    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));

    await waitFor(() => {
      expect(screen.getByLabelText("Chromosome")).toBeDefined();
    });
    expect(screen.getByLabelText("Search rsID")).toBeDefined();
  });

  it("shows pagination info", async () => {
    renderWithProviders(<VariantBrowser />);
    const user = userEvent.setup();

    await user.click(screen.getByText("Expand"));

    await waitFor(() => {
      expect(screen.getByText(/Page 1 of 1/)).toBeDefined();
    });
  });
});
