// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  it("clicking Enable calls updateUserStatus with correct args", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockUpdateStatus = vi.mocked(adminApi.updateUserStatus);
    mockUpdateStatus.mockResolvedValue({
      ...mockUsers[1],
      status: "active",
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("user@example.com")).toBeDefined();
    });

    const enableBtn = screen.getByRole("button", { name: /enable/i });
    await userEvent.click(enableBtn);

    expect(mockUpdateStatus).toHaveBeenCalledWith("u2", "active");
  });

  it("clicking Delete shows confirm dialog and confirming calls deleteUser", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockDelete = vi.mocked(adminApi.deleteUser);
    mockDelete.mockResolvedValue(undefined);

    // Mock window.confirm to return true
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("user@example.com")).toBeDefined();
    });

    const deleteBtn = screen.getByRole("button", { name: /delete/i });
    await userEvent.click(deleteBtn);

    expect(confirmSpy).toHaveBeenCalledWith("Delete user user@example.com? This cannot be undone.");
    expect(mockDelete).toHaveBeenCalledWith("u2");

    confirmSpy.mockRestore();
  });

  it("clicking Delete and canceling confirm does not call deleteUser", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockDelete = vi.mocked(adminApi.deleteUser);
    mockDelete.mockReset();

    // Mock window.confirm to return false
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("user@example.com")).toBeDefined();
    });

    const deleteBtn = screen.getByRole("button", { name: /delete/i });
    await userEvent.click(deleteBtn);

    expect(confirmSpy).toHaveBeenCalled();
    expect(mockDelete).not.toHaveBeenCalled();

    confirmSpy.mockRestore();
  });

  it("clicking Create Invite shows form and submitting calls createInvite", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockCreate = vi.mocked(adminApi.createInvite);
    mockCreate.mockResolvedValue({
      id: "inv-new",
      code: "INVITE-NEW",
      label: "Test label",
      max_uses: 5,
      use_count: 0,
      created_at: "2026-03-22T00:00:00Z",
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /create invite/i })).toBeDefined();
    });

    // Click Create Invite to show the form
    await userEvent.click(screen.getByRole("button", { name: /create invite/i }));

    // Fill in the form
    const labelInput = screen.getByLabelText(/label/i);
    const maxUsesInput = screen.getByLabelText(/max uses/i);
    const expiresInput = screen.getByLabelText(/expires in/i);

    await userEvent.type(labelInput, "Test label");
    await userEvent.type(maxUsesInput, "5");
    await userEvent.type(expiresInput, "24");

    // Submit the form
    await userEvent.click(screen.getByRole("button", { name: /^create$/i }));

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalledWith({
        label: "Test label",
        max_uses: 5,
        expires_in_hours: 24,
      });
    });
  });

  it("clicking Revoke calls revokeInvite with correct id", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockRevoke = vi.mocked(adminApi.revokeInvite);
    mockRevoke.mockResolvedValue({
      ...mockInvites[0],
      revoked_at: "2026-03-22T00:00:00Z",
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("INVITE-ABC")).toBeDefined();
    });

    // The first invite (INVITE-ABC) is active and should have a Revoke button
    const revokeBtn = screen.getByRole("button", { name: /revoke/i });
    await userEvent.click(revokeBtn);

    expect(mockRevoke).toHaveBeenCalledWith("inv1");
  });
});
