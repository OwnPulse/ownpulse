// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Admin from "../../src/pages/Admin";
import { useAuthStore } from "../../src/store/auth";

const mockUsers = [
  {
    id: "u1",
    email: "admin@example.com",
    username: "admin",
    auth_provider: "password",
    role: "admin",
    status: "active",
    data_region: "us",
    created_at: "2025-01-01T00:00:00Z",
  },
  {
    id: "u2",
    email: "user@example.com",
    auth_provider: "google",
    role: "user",
    status: "disabled",
    data_region: "us",
    created_at: "2025-06-01T00:00:00Z",
  },
];

const mockInvites = [
  {
    id: "inv1",
    code: "INVITE-ABC",
    label: "For friends",
    max_uses: 10,
    use_count: 3,
    expires_at: null,
    revoked_at: null,
    created_at: "2025-01-01T00:00:00Z",
  },
  {
    id: "inv2",
    code: "INVITE-DEF",
    label: null,
    max_uses: null,
    use_count: 5,
    expires_at: "2024-01-01T00:00:00Z",
    revoked_at: null,
    created_at: "2025-01-01T00:00:00Z",
  },
];

vi.mock("../../src/api/admin", () => ({
  adminApi: {
    listUsers: vi.fn().mockImplementation(() => Promise.resolve(mockUsers)),
    listInvites: vi.fn().mockImplementation(() => Promise.resolve(mockInvites)),
    updateRole: vi.fn(),
    updateUserStatus: vi.fn(),
    deleteUser: vi.fn(),
    createInvite: vi.fn(),
    revokeInvite: vi.fn(),
  },
  // Re-export types are not needed at runtime, but the module mock needs to
  // provide the named exports used in the component import
}));

function renderAdmin() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <Admin />
    </QueryClientProvider>,
  );
}

describe("Admin page", () => {
  beforeEach(() => {
    // Set a fake JWT so the component can decode a user ID
    // JWT payload: { "sub": "u1" } encoded
    const fakeToken = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1MSJ9.abc";
    useAuthStore.setState({
      token: fakeToken,
      isAuthenticated: true,
      role: "admin",
    });
  });

  it("renders users table with status column", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("admin@example.com")).toBeDefined();
    });

    expect(screen.getByText("user@example.com")).toBeDefined();
    // Status badges (multiple "active" badges may exist across users and invites sections)
    expect(screen.getAllByText("active").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("disabled").length).toBeGreaterThanOrEqual(1);
  });

  it("renders invites section with invite data", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("INVITE-ABC")).toBeDefined();
    });

    expect(screen.getByText("For friends")).toBeDefined();
    expect(screen.getByText("INVITE-DEF")).toBeDefined();
    // Uses display
    expect(screen.getByText(/3\/10/)).toBeDefined();
  });

  it("shows Create Invite button", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /create invite/i })).toBeDefined();
    });
  });

  it("does not show action buttons for the current user", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("admin@example.com")).toBeDefined();
    });

    // u2 (not self) should have Disable/Enable and Delete buttons
    const enableBtn = screen.getByRole("button", { name: /enable/i });
    expect(enableBtn).toBeDefined();
    const deleteBtn = screen.getByRole("button", { name: /delete/i });
    expect(deleteBtn).toBeDefined();

    // u1 (self) row should NOT have Disable or Delete buttons
    // There should only be one Delete button (for u2)
    const deleteButtons = screen.getAllByRole("button", { name: /delete/i });
    expect(deleteButtons).toHaveLength(1);
  });
});
