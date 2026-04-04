// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import ProtocolBuilder from "../../src/pages/ProtocolBuilder";
import { useAuthStore } from "../../src/store/auth";

// Mock sub-components
vi.mock("../../src/components/protocols/PatternSelector", () => ({
  default: () => <div data-testid="pattern-selector">Pattern</div>,
}));

vi.mock("../../src/components/protocols/SequencerGrid", () => ({
  default: () => <div data-testid="sequencer-grid">Grid</div>,
}));

vi.mock("../../src/components/protocols/StartRunModal", () => ({
  StartRunModal: ({
    protocolName,
    onClose,
    onStarted,
  }: {
    protocolName: string;
    onClose: () => void;
    onStarted?: () => void;
  }) => (
    <div data-testid="start-run-modal">
      <span>{protocolName}</span>
      <button
        type="button"
        onClick={() => {
          onStarted?.();
          onClose();
        }}
      >
        Confirm Start
      </button>
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

function renderBuilder() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/protocols/new"]}>
        <Routes>
          <Route path="/protocols/new" element={<ProtocolBuilder />} />
          <Route path="/protocols/:id" element={<div data-testid="protocol-view">View</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("ProtocolBuilder Start Now prompt", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true });
    // Clear sessionStorage draft
    sessionStorage.removeItem("protocol-builder-draft");
  });

  it("shows Start Now prompt after successful creation", async () => {
    server.use(
      http.get("/api/v1/interventions", () => HttpResponse.json([])),
      http.post("/api/v1/protocols", () =>
        HttpResponse.json({
          id: "new-proto",
          user_id: "user-1",
          name: "Test Protocol",
          description: null,
          status: "draft",
          start_date: "2026-03-28",
          duration_days: 28,
          share_token: null,
          created_at: "2026-03-28T00:00:00Z",
          updated_at: "2026-03-28T00:00:00Z",
          lines: [],
        }),
      ),
    );

    renderBuilder();
    const user = userEvent.setup();

    // Fill in name
    const nameInput = screen.getByLabelText("Name");
    await user.type(nameInput, "Test Protocol");

    // Fill in substance (first line)
    const substanceInput = screen.getByLabelText("Substance");
    await user.type(substanceInput, "BPC-157");

    // Submit
    await user.click(screen.getByText("Create Protocol"));

    // Should show "Start Now?" prompt
    await waitFor(() => {
      expect(screen.getByText("Protocol Created")).toBeDefined();
    });

    expect(screen.getByText("Test Protocol", { selector: "strong" })).toBeDefined();
    expect(screen.getByText("Start Now")).toBeDefined();
    expect(screen.getByText("View Protocol")).toBeDefined();
  });

  it("opens StartRunModal when Start Now is clicked", async () => {
    server.use(
      http.get("/api/v1/interventions", () => HttpResponse.json([])),
      http.post("/api/v1/protocols", () =>
        HttpResponse.json({
          id: "new-proto",
          user_id: "user-1",
          name: "Test Protocol",
          description: null,
          status: "draft",
          start_date: "2026-03-28",
          duration_days: 28,
          share_token: null,
          created_at: "2026-03-28T00:00:00Z",
          updated_at: "2026-03-28T00:00:00Z",
          lines: [],
        }),
      ),
    );

    renderBuilder();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText("Name"), "Test Protocol");
    await user.type(screen.getByLabelText("Substance"), "BPC-157");
    await user.click(screen.getByText("Create Protocol"));

    await waitFor(() => {
      expect(screen.getByText("Protocol Created")).toBeDefined();
    });

    await user.click(screen.getByText("Start Now"));
    expect(screen.getByTestId("start-run-modal")).toBeDefined();
  });
});
