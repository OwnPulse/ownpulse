// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import Protocols from "../../src/pages/Protocols";
import { useAuthStore } from "../../src/store/auth";

// Mock sub-components to isolate tests
vi.mock("../../src/components/protocols/ImportModal", () => ({
  ImportModal: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="import-modal">
      <button type="button" onClick={onClose}>
        Close Import
      </button>
    </div>
  ),
}));

vi.mock("../../src/components/protocols/TemplateCard", () => ({
  TemplateCard: ({ template }: { template: { name: string } }) => (
    <div data-testid="template-card">{template.name}</div>
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

const protocols = [
  {
    id: "p1",
    name: "BPC-157 Stack",
    status: "active",
    duration_days: 28,
    created_at: "2026-03-01T00:00:00Z",
  },
];

const activeRuns = [
  {
    id: "run-1",
    protocol_id: "p1",
    protocol_name: "BPC-157 Stack",
    user_id: "user-1",
    start_date: "2026-03-28",
    duration_days: 28,
    status: "active",
    notify: false,
    notify_time: null,
    notify_times: [],
    repeat_reminders: false,
    repeat_interval_minutes: 30,
    progress_pct: 17.86,
    doses_today: 2,
    doses_completed_today: 0,
    created_at: "2026-03-28T10:00:00Z",
  },
];

function renderWithProviders() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Protocols />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Protocols page", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
  });

  it("renders loading state", () => {
    server.use(
      http.get("/api/v1/protocols", () => {
        // Never respond — keep loading
        return new Promise(() => {});
      }),
      http.get("/api/v1/protocols/runs/active", () => {
        return new Promise(() => {});
      }),
      http.get("/api/v1/protocols/templates", () => {
        return new Promise(() => {});
      }),
    );

    renderWithProviders();
    expect(screen.getByText("Loading...")).toBeDefined();
  });

  it("renders error state", async () => {
    server.use(
      http.get("/api/v1/protocols", () => new HttpResponse("Error", { status: 500 })),
      http.get("/api/v1/protocols/runs/active", () => HttpResponse.json([])),
      http.get("/api/v1/protocols/templates", () => HttpResponse.json([])),
    );

    renderWithProviders();
    await waitFor(() => {
      expect(screen.getByText("Error loading protocols.")).toBeDefined();
    });
  });

  it("renders Active Runs section and My Protocols section", async () => {
    server.use(
      http.get("/api/v1/protocols", () => HttpResponse.json(protocols)),
      http.get("/api/v1/protocols/runs/active", () => HttpResponse.json(activeRuns)),
      http.get("/api/v1/protocols/templates", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Active Runs")).toBeDefined();
    });

    // Active run card + My Protocols both show the name
    expect(screen.getAllByText("BPC-157 Stack").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("2 doses pending")).toBeDefined();

    // My Protocols section
    expect(screen.getByText("My Protocols")).toBeDefined();
    expect(screen.getByText("Start")).toBeDefined();
  });

  it("renders empty state when no protocols", async () => {
    server.use(
      http.get("/api/v1/protocols", () => HttpResponse.json([])),
      http.get("/api/v1/protocols/runs/active", () => HttpResponse.json([])),
      http.get("/api/v1/protocols/templates", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(
        screen.getByText("No protocols yet. Create your first dosing protocol."),
      ).toBeDefined();
    });
  });

  it("opens Start Run modal when Start button is clicked", async () => {
    server.use(
      http.get("/api/v1/protocols", () => HttpResponse.json(protocols)),
      http.get("/api/v1/protocols/runs/active", () => HttpResponse.json([])),
      http.get("/api/v1/protocols/templates", () => HttpResponse.json([])),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Start")).toBeDefined();
    });

    await user.click(screen.getByText("Start"));
    expect(screen.getByTestId("start-run-modal")).toBeDefined();
  });

  it("shows dose badge as All done when all doses completed", async () => {
    const completedRuns = [
      {
        ...activeRuns[0],
        doses_completed_today: 2,
      },
    ];

    server.use(
      http.get("/api/v1/protocols", () => HttpResponse.json(protocols)),
      http.get("/api/v1/protocols/runs/active", () => HttpResponse.json(completedRuns)),
      http.get("/api/v1/protocols/templates", () => HttpResponse.json([])),
    );

    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("All done")).toBeDefined();
    });
  });
});
