// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import InviteLanding from "../../src/pages/InviteLanding";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

function renderWithRouter(code: string) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/invite/${code}`]}>
        <Routes>
          <Route path="/invite/:code" element={<InviteLanding />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("InviteLanding", () => {
  it("shows loading state while checking invite", () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        // Never resolve to keep loading state
        return new Promise(() => {});
      }),
    );

    renderWithRouter("VALID-CODE");
    expect(screen.getByText("Checking invite...")).toBeInTheDocument();
  });

  it("renders branded page for valid invite", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: true,
          label: "For Tony's friends",
          expires_at: "2026-12-31T00:00:00Z",
          inviter_name: "Tony",
        });
      }),
    );

    renderWithRouter("VALID-CODE");

    await waitFor(() => {
      expect(screen.getByText("You've been invited to OwnPulse")).toBeInTheDocument();
    });
    expect(screen.getByText("Tony")).toBeInTheDocument();
    expect(screen.getByText("For Tony's friends")).toBeInTheDocument();
    expect(screen.getByText(/2026/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Create Account" })).toHaveAttribute(
      "href",
      "/register?invite=VALID-CODE",
    );
  });

  it("shows 'Expires soon' badge when invite expires within 24 hours", async () => {
    const soonExpiry = new Date(Date.now() + 6 * 60 * 60 * 1000).toISOString();
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: true,
          label: null,
          expires_at: soonExpiry,
          inviter_name: "Tony",
        });
      }),
    );

    renderWithRouter("SOON-CODE");

    await waitFor(() => {
      expect(screen.getByText("Expires soon")).toBeInTheDocument();
    });
  });

  it("shows error for expired invite", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: false,
          label: null,
          expires_at: null,
          inviter_name: null,
          reason: "expired",
        });
      }),
    );

    renderWithRouter("EXPIRED-CODE");

    await waitFor(() => {
      expect(screen.getByText("Invite unavailable")).toBeInTheDocument();
    });
    expect(screen.getByText(/This invite has expired/)).toBeInTheDocument();
  });

  it("shows error for revoked invite", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: false,
          label: null,
          expires_at: null,
          inviter_name: null,
          reason: "revoked",
        });
      }),
    );

    renderWithRouter("REVOKED-CODE");

    await waitFor(() => {
      expect(screen.getByText("Invite unavailable")).toBeInTheDocument();
    });
    expect(screen.getByText(/This invite is no longer valid/)).toBeInTheDocument();
  });

  it("shows error for exhausted invite", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: false,
          label: null,
          expires_at: null,
          inviter_name: null,
          reason: "exhausted",
        });
      }),
    );

    renderWithRouter("FULL-CODE");

    await waitFor(() => {
      expect(screen.getByText("Invite unavailable")).toBeInTheDocument();
    });
    expect(screen.getByText(/already been used the maximum/)).toBeInTheDocument();
  });

  it("shows network error state", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.error();
      }),
    );

    renderWithRouter("BAD-CODE");

    await waitFor(() => {
      expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    });
  });

  it("renders valid invite without optional fields", async () => {
    server.use(
      http.get("/api/v1/invites/:code/check", () => {
        return HttpResponse.json({
          valid: true,
          label: null,
          expires_at: null,
          inviter_name: null,
        });
      }),
    );

    renderWithRouter("MINIMAL-CODE");

    await waitFor(() => {
      expect(screen.getByText("You've been invited to OwnPulse")).toBeInTheDocument();
    });
    expect(screen.getByRole("link", { name: "Create Account" })).toBeInTheDocument();
    expect(screen.queryByText("Invited by")).not.toBeInTheDocument();
    expect(screen.queryByText("Note")).not.toBeInTheDocument();
    expect(screen.queryByText("Expires")).not.toBeInTheDocument();
  });
});
