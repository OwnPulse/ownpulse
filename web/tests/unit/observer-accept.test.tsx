// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ObserverAccept from "../../src/pages/ObserverAccept";
import { useAuthStore } from "../../src/store/auth";

const mockAccept = vi.fn();

vi.mock("../../src/api/observer-polls", () => ({
  observerPollsApi: {
    accept: (...args: unknown[]) => mockAccept(...args),
  },
}));

function renderWithToken(token: string | null) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const path = token ? `/observe/accept?token=${token}` : "/observe/accept";
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <ObserverAccept />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("ObserverAccept page", () => {
  beforeEach(() => {
    mockAccept.mockReset();
    useAuthStore.setState({
      token: "test-jwt",
      isAuthenticated: true,
      role: "user",
    });
  });

  it("shows loading state while accepting", () => {
    mockAccept.mockReturnValue(new Promise(() => {}));
    renderWithToken("valid-token");

    expect(screen.getByText("Accepting Invite")).toBeDefined();
    expect(screen.getByText("Please wait...")).toBeDefined();
  });

  it("shows success for accepted status", async () => {
    mockAccept.mockResolvedValue({ status: "accepted" });
    renderWithToken("valid-token");

    await waitFor(() => {
      expect(screen.getByText("Observer Invite Accepted")).toBeDefined();
    });

    expect(
      screen.getByText(/added as an observer/),
    ).toBeDefined();
    expect(
      screen.getByRole("link", { name: /go to observer polls/i }),
    ).toBeDefined();
  });

  it("shows message for acknowledged status", async () => {
    mockAccept.mockResolvedValue({ status: "acknowledged" });
    renderWithToken("expired-token");

    await waitFor(() => {
      expect(screen.getByText("Invite No Longer Valid")).toBeDefined();
    });

    expect(
      screen.getByText(/no longer valid/),
    ).toBeDefined();
  });

  it("shows error state on failure", async () => {
    mockAccept.mockRejectedValue(new Error("Token expired"));
    renderWithToken("bad-token");

    await waitFor(() => {
      expect(screen.getByText("Error")).toBeDefined();
    });

    expect(screen.getByText("Token expired")).toBeDefined();
  });

  it("shows invalid link when no token", () => {
    renderWithToken(null);

    expect(screen.getByText("Invalid Link")).toBeDefined();
    expect(
      screen.getByText("No invite token found in this link."),
    ).toBeDefined();
  });

  it("calls accept with the token from URL", async () => {
    mockAccept.mockResolvedValue({ status: "accepted" });
    renderWithToken("my-invite-token");

    await waitFor(() => {
      expect(mockAccept).toHaveBeenCalledWith("my-invite-token");
    });
  });
});
