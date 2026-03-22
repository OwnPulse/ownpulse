// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, vi, beforeAll, afterAll, afterEach, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { setupServer } from "msw/node";
import { http, HttpResponse } from "msw";
import { useAuthStore } from "../../src/store/auth";
import type { AuthMethod } from "../../src/api/auth";

const TOKEN = "test-jwt";

const TWO_METHODS: AuthMethod[] = [
  { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
  { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
];

const ONE_METHOD: AuthMethod[] = [
  { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
];

const server = setupServer(
  // Default: return empty arrays for endpoints the Settings page queries
  http.get("/api/v1/source-preferences", () => HttpResponse.json([])),
);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

async function renderSettings() {
  const { default: Settings } = await import("../../src/pages/Settings");
  return render(<Settings />, { wrapper });
}

describe("Settings — Linked Accounts", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  it("shows linked accounts list", async () => {
    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(TWO_METHODS)),
    );

    await renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Google")).toBeDefined();
      expect(screen.getByText("Apple")).toBeDefined();
      expect(screen.getByText("user@example.com")).toBeDefined();
    });
  });

  it("shows Unlink buttons only when more than one method exists", async () => {
    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(TWO_METHODS)),
    );

    await renderSettings();

    await waitFor(() => {
      const unlinkBtns = screen.getAllByRole("button", { name: /unlink/i });
      expect(unlinkBtns.length).toBe(2);
    });
  });

  it("hides Unlink button when only one method exists", async () => {
    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(ONE_METHOD)),
    );

    await renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Google")).toBeDefined();
    });
    expect(screen.queryByRole("button", { name: /unlink/i })).toBeNull();
  });

  it("calls unlinkAuth when Unlink is clicked and confirmed", async () => {
    let capturedProvider: string | undefined;

    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(TWO_METHODS)),
      http.delete("/api/v1/auth/link/:provider", ({ params }) => {
        capturedProvider = params["provider"] as string;
        return HttpResponse.json(ONE_METHOD);
      }),
    );

    vi.spyOn(window, "confirm").mockReturnValue(true);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /unlink/i }).length).toBe(2);
    });

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /unlink google/i }));

    await waitFor(() => {
      expect(capturedProvider).toBe("google");
    });
  });

  it("does not unlink when confirmation is cancelled", async () => {
    let unlinkCalled = false;

    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(TWO_METHODS)),
      http.delete("/api/v1/auth/link/:provider", () => {
        unlinkCalled = true;
        return HttpResponse.json(ONE_METHOD);
      }),
    );

    vi.spyOn(window, "confirm").mockReturnValue(false);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /unlink/i }).length).toBe(2);
    });

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /unlink google/i }));

    expect(unlinkCalled).toBe(false);
  });

  it("shows error message when unlink fails", async () => {
    server.use(
      http.get("/api/v1/auth/methods", () => HttpResponse.json(TWO_METHODS)),
      http.delete("/api/v1/auth/link/:provider", () =>
        new HttpResponse("Server error", { status: 500 }),
      ),
    );

    vi.spyOn(window, "confirm").mockReturnValue(true);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /unlink/i }).length).toBe(2);
    });

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /unlink google/i }));

    await waitFor(() => {
      expect(screen.getByText("Server error")).toBeDefined();
    });
  });
});
