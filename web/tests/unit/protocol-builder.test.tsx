// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import ProtocolBuilder from "../../src/pages/ProtocolBuilder";

const DRAFT_KEY = "protocol-builder-draft";

const server = setupServer(
  http.get("/api/v1/interventions", () => {
    return HttpResponse.json([]);
  }),
);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => {
  server.resetHandlers();
  sessionStorage.clear();
});
afterAll(() => server.close());

function renderBuilder() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <ProtocolBuilder />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("ProtocolBuilder", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the form with name input and duration presets", () => {
    renderBuilder();

    expect(screen.getByLabelText("Name")).toBeDefined();
    expect(screen.getByText("2W")).toBeDefined();
    expect(screen.getByText("4W")).toBeDefined();
    expect(screen.getByText("8W")).toBeDefined();
    expect(screen.getByText("12W")).toBeDefined();
    expect(screen.getByText("Create Protocol")).toBeDefined();
  });

  it("does not render a start date field", () => {
    renderBuilder();

    expect(screen.queryByLabelText("Start Date")).toBeNull();
  });

  it("renders Interventions section header", () => {
    renderBuilder();

    expect(screen.getByText("Interventions")).toBeDefined();
    expect(screen.getByText("+ Add Intervention")).toBeDefined();
  });

  it("saves state to sessionStorage after debounce", async () => {
    renderBuilder();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    const nameInput = screen.getByLabelText("Name");
    await user.type(nameInput, "My Protocol");

    // Advance past the 300ms debounce
    act(() => {
      vi.advanceTimersByTime(350);
    });

    const stored = sessionStorage.getItem(DRAFT_KEY);
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored as string);
    expect(parsed.name).toBe("My Protocol");
    expect(parsed.weeks).toBe(4);
    expect(Array.isArray(parsed.lines)).toBe(true);
    // startDate should not be persisted
    expect(parsed.startDate).toBeUndefined();
  });

  it("restores state from sessionStorage on mount", () => {
    const draft = {
      name: "Restored Protocol",
      weeks: 8,
      lines: [
        {
          id: 0,
          substance: "BPC-157",
          dose: "250",
          unit: "mcg",
          route: "SubQ",
          time_of_day: "AM",
          schedule_pattern: Array(56).fill(true),
        },
      ],
    };
    sessionStorage.setItem(DRAFT_KEY, JSON.stringify(draft));

    renderBuilder();

    const nameInput = screen.getByLabelText("Name") as HTMLInputElement;
    expect(nameInput.value).toBe("Restored Protocol");

    const substanceInput = screen.getByLabelText("Substance") as HTMLInputElement;
    expect(substanceInput.value).toBe("BPC-157");
  });

  it("clears draft on Start Over click", async () => {
    const draft = {
      name: "Draft Protocol",
      weeks: 4,
      lines: [
        {
          id: 0,
          substance: "TB-500",
          dose: "",
          unit: "",
          route: "",
          time_of_day: "",
          schedule_pattern: Array(28).fill(true),
        },
      ],
    };
    sessionStorage.setItem(DRAFT_KEY, JSON.stringify(draft));

    renderBuilder();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    // Verify draft was loaded
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Draft Protocol");

    await user.click(screen.getByText("Start Over"));

    // Name should be cleared
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("");

    // sessionStorage should be cleared immediately
    // (the subsequent save effect will re-save empty state after debounce,
    // but clearDraft() runs synchronously)
    // Advance timers to let the save effect fire
    act(() => {
      vi.advanceTimersByTime(350);
    });

    const stored = sessionStorage.getItem(DRAFT_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      expect(parsed.name).toBe("");
    }
  });

  it("clears draft on successful submission", async () => {
    server.use(
      http.post("/api/v1/protocols", () => {
        return HttpResponse.json({
          id: "proto-123",
          user_id: "user-1",
          name: "Test",
          description: null,
          status: "active",
          start_date: null,
          duration_days: 28,
          share_token: null,
          created_at: "2026-01-01T00:00:00Z",
          updated_at: "2026-01-01T00:00:00Z",
          lines: [],
        });
      }),
    );

    // Pre-populate draft
    sessionStorage.setItem(
      DRAFT_KEY,
      JSON.stringify({
        name: "Test",
        weeks: 4,
        lines: [
          {
            id: 0,
            substance: "BPC-157",
            dose: "",
            unit: "",
            route: "",
            time_of_day: "",
            schedule_pattern: Array(28).fill(true),
          },
        ],
      }),
    );

    renderBuilder();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    // Submit the form
    await user.click(screen.getByText("Create Protocol"));

    await waitFor(() => {
      expect(sessionStorage.getItem(DRAFT_KEY)).toBeNull();
    });
  });

  it("handles invalid JSON in sessionStorage gracefully", () => {
    sessionStorage.setItem(DRAFT_KEY, "not-valid-json{{{");

    // Should not throw — renders with default state
    renderBuilder();

    const nameInput = screen.getByLabelText("Name") as HTMLInputElement;
    expect(nameInput.value).toBe("");
  });

  it("handles malformed draft shape gracefully", () => {
    sessionStorage.setItem(DRAFT_KEY, JSON.stringify({ name: 42, weeks: "not a number" }));

    renderBuilder();

    const nameInput = screen.getByLabelText("Name") as HTMLInputElement;
    expect(nameInput.value).toBe("");
  });

  it("shows error message on submission failure", async () => {
    server.use(
      http.post("/api/v1/protocols", () => {
        return HttpResponse.json({ error: "Internal server error" }, { status: 500 });
      }),
    );

    renderBuilder();
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    const nameInput = screen.getByLabelText("Name");
    await user.type(nameInput, "Test Protocol");

    const substanceInput = screen.getByLabelText("Substance");
    await user.type(substanceInput, "BPC-157");

    await user.click(screen.getByText("Create Protocol"));

    await waitFor(() => {
      expect(screen.getByText(/Error:/)).toBeDefined();
    });
  });
});
