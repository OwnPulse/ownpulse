// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import SharedProtocol from "../../src/pages/SharedProtocol";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const sharedProtocol = {
  id: "proto-1",
  name: "BPC-157 Stack",
  description: "Healing protocol",
  status: "active",
  duration_days: 14,
  created_at: "2026-03-01T00:00:00Z",
  lines: [
    {
      id: "line-1",
      protocol_id: "proto-1",
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      schedule_pattern: Array(14).fill(true),
      sort_order: 0,
      doses: [],
    },
  ],
};

function renderWithProviders(token = "share-tok-1", isAuthenticated = true) {
  useAuthStore.setState({
    token: isAuthenticated ? "test-jwt" : null,
    isAuthenticated,
  });

  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/protocols/shared/${token}`]}>
        <Routes>
          <Route path="/protocols/shared/:token" element={<SharedProtocol />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SharedProtocol", () => {
  it("renders loading state", () => {
    server.use(http.get("/api/v1/protocols/shared/:token", () => new Promise(() => {})));
    renderWithProviders();
    expect(screen.getByText("Loading...")).toBeDefined();
  });

  it("renders error state for an invalid or expired token", async () => {
    server.use(
      http.get(
        "/api/v1/protocols/shared/:token",
        () => new HttpResponse("Not found", { status: 404 }),
      ),
    );
    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("Protocol Not Found")).toBeDefined();
    });
    expect(screen.getByText("This share link is invalid or has expired.")).toBeDefined();
  });

  it("renders the protocol when the token is valid", async () => {
    server.use(
      http.get("/api/v1/protocols/shared/:token", () => HttpResponse.json(sharedProtocol)),
    );
    renderWithProviders();

    await waitFor(() => {
      expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    });
    expect(screen.getByText("Healing protocol")).toBeDefined();
  });

  it("prompts login when unauthenticated", async () => {
    server.use(
      http.get("/api/v1/protocols/shared/:token", () => HttpResponse.json(sharedProtocol)),
    );
    renderWithProviders("share-tok-1", false);

    await waitFor(() => {
      expect(screen.getByText("BPC-157 Stack")).toBeDefined();
    });
    expect(screen.getByText("Log in")).toBeDefined();
  });

  it("imports the protocol via POST /api/v1/protocols/import/:token when authenticated", async () => {
    let capturedUrl: string | undefined;
    let capturedBody: unknown;

    server.use(
      http.get("/api/v1/protocols/shared/:token", () => HttpResponse.json(sharedProtocol)),
      http.post("/api/v1/protocols/import/:token", async ({ request }) => {
        capturedUrl = request.url;
        capturedBody = await request.json();
        return HttpResponse.json({ id: "proto-copy" }, { status: 201 });
      }),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Copy to My Protocols")).toBeDefined();
    });

    await user.click(screen.getByText("Copy to My Protocols"));

    await waitFor(() => {
      expect(capturedUrl).toBeDefined();
    });

    expect(capturedUrl).toContain("/api/v1/protocols/import/share-tok-1");
    expect(capturedBody).toEqual({});
  });

  it("shows an error message when the import fails", async () => {
    server.use(
      http.get("/api/v1/protocols/shared/:token", () => HttpResponse.json(sharedProtocol)),
      http.post(
        "/api/v1/protocols/import/:token",
        () => new HttpResponse("Internal Server Error", { status: 500 }),
      ),
    );

    renderWithProviders();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText("Copy to My Protocols")).toBeDefined();
    });

    await user.click(screen.getByText("Copy to My Protocols"));

    await waitFor(() => {
      expect(screen.getByText("Failed to copy protocol. Please try again.")).toBeDefined();
    });
  });
});
