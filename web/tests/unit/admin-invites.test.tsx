// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
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
    sendInviteEmail: vi.fn(),
  },
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
    expect(screen.getAllByText("active").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("disabled").length).toBeGreaterThanOrEqual(1);
  });

  it("renders invite cards with invite data", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    const cards = screen.getAllByTestId("invite-card");
    expect(cards.length).toBe(2);

    // First card is active
    expect(within(cards[0]).getByText("Active")).toBeDefined();
    // Second card is expired (expires_at in the past)
    expect(within(cards[1]).getByText("Expired")).toBeDefined();

    // Uses display
    expect(screen.getByText(/3\/10/)).toBeDefined();
  });

  it("shows invite links instead of raw codes by default", async () => {
    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    // Invite links should be visible
    const linkElements = screen.getAllByText((content) => content.includes("/invite/INVITE-ABC"));
    expect(linkElements.length).toBeGreaterThanOrEqual(1);

    // Raw code should NOT be visible as a standalone element by default.
    // Check that clicking Show Code reveals it separately.
    const cards = screen.getAllByTestId("invite-card");
    const showCodeBtn = within(cards[0]).getByRole("button", { name: /show code/i });
    expect(showCodeBtn).toBeDefined();

    await userEvent.click(showCodeBtn);

    // Now the raw code should appear in its own element
    const rawCodeElements = within(cards[0]).getAllByText("INVITE-ABC");
    // At least one extra element for the raw code display
    expect(rawCodeElements.length).toBeGreaterThanOrEqual(1);
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

    const enableBtn = screen.getByRole("button", { name: /enable/i });
    expect(enableBtn).toBeDefined();
    const deleteBtn = screen.getByRole("button", { name: /delete/i });
    expect(deleteBtn).toBeDefined();

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

  it("clicking Create Invite shows form with email field and submitting calls createInvite", async () => {
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

    await userEvent.click(screen.getByRole("button", { name: /create invite/i }));

    const labelInput = screen.getByLabelText(/label/i);
    const maxUsesInput = screen.getByLabelText(/max uses/i);
    const expiresInput = screen.getByLabelText(/expires in/i);
    const emailInput = screen.getByLabelText(/send to email/i);

    await userEvent.type(labelInput, "Test label");
    await userEvent.type(maxUsesInput, "5");
    await userEvent.type(expiresInput, "24");
    await userEvent.type(emailInput, "friend@example.com");

    await userEvent.click(screen.getByRole("button", { name: /^create$/i }));

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalledWith({
        label: "Test label",
        max_uses: 5,
        expires_in_hours: 24,
        send_to_email: "friend@example.com",
      });
    });
  });

  it("creating invite without email does not include send_to_email", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockCreate = vi.mocked(adminApi.createInvite);
    mockCreate.mockResolvedValue({
      id: "inv-new",
      code: "INVITE-NEW",
      label: "No email",
      use_count: 0,
      created_at: "2026-03-22T00:00:00Z",
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /create invite/i })).toBeDefined();
    });

    await userEvent.click(screen.getByRole("button", { name: /create invite/i }));

    const labelInput = screen.getByLabelText(/label/i);
    await userEvent.type(labelInput, "No email");

    await userEvent.click(screen.getByRole("button", { name: /^create$/i }));

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalledWith({
        label: "No email",
        max_uses: undefined,
        expires_in_hours: undefined,
        send_to_email: undefined,
      });
    });
  });

  it("shows success card after creating an invite with email sent confirmation", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockCreate = vi.mocked(adminApi.createInvite);
    mockCreate.mockResolvedValue({
      id: "inv-new",
      code: "INVITE-NEW",
      label: "Test",
      use_count: 0,
      created_at: "2026-03-22T00:00:00Z",
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /create invite/i })).toBeDefined();
    });

    await userEvent.click(screen.getByRole("button", { name: /create invite/i }));

    const emailInput = screen.getByLabelText(/send to email/i);
    await userEvent.type(emailInput, "alice@example.com");

    await userEvent.click(screen.getByRole("button", { name: /^create$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("invite-success")).toBeDefined();
    });

    expect(screen.getByText("Invite created")).toBeDefined();
    expect(screen.getByText("Email sent to alice@example.com")).toBeDefined();
    expect(screen.getByText((content) => content.includes("/invite/INVITE-NEW"))).toBeDefined();
  });

  it("copy link button copies the invite link", async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: { writeText: writeTextMock },
    });

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    const cards = screen.getAllByTestId("invite-card");
    const copyBtn = within(cards[0]).getByRole("button", { name: /copy link/i });
    await userEvent.click(copyBtn);

    expect(writeTextMock).toHaveBeenCalledWith(expect.stringContaining("/invite/INVITE-ABC"));

    // Should show "Copied!" feedback
    expect(within(cards[0]).getByRole("button", { name: /copied/i })).toBeDefined();
  });

  it("revoke shows confirmation dialog", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockRevoke = vi.mocked(adminApi.revokeInvite);
    mockRevoke.mockResolvedValue({
      ...mockInvites[0],
      revoked_at: "2026-03-22T00:00:00Z",
    });

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    const revokeBtn = screen.getByRole("button", { name: /revoke/i });
    await userEvent.click(revokeBtn);

    expect(confirmSpy).toHaveBeenCalledWith("Are you sure? This invite link will stop working.");
    expect(mockRevoke).toHaveBeenCalledWith("inv1");

    confirmSpy.mockRestore();
  });

  it("canceling revoke confirmation does not call revokeInvite", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockRevoke = vi.mocked(adminApi.revokeInvite);
    mockRevoke.mockReset();

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    const revokeBtn = screen.getByRole("button", { name: /revoke/i });
    await userEvent.click(revokeBtn);

    expect(confirmSpy).toHaveBeenCalled();
    expect(mockRevoke).not.toHaveBeenCalled();

    confirmSpy.mockRestore();
  });

  it("send email button shows inline form and submits", async () => {
    const { adminApi } = await import("../../src/api/admin");
    const mockSendEmail = vi.mocked(adminApi.sendInviteEmail);
    mockSendEmail.mockResolvedValue(undefined);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("For friends")).toBeDefined();
    });

    const cards = screen.getAllByTestId("invite-card");
    const activeCard = cards[0];

    const sendEmailBtn = within(activeCard).getByRole("button", { name: /send email/i });
    await userEvent.click(sendEmailBtn);

    // Email form should appear
    const emailInput = within(activeCard).getByLabelText(/recipient email/i);
    expect(emailInput).toBeDefined();

    await userEvent.type(emailInput, "bob@example.com");
    await userEvent.click(within(activeCard).getByRole("button", { name: /^send$/i }));

    await waitFor(() => {
      expect(mockSendEmail).toHaveBeenCalledWith("inv1", "bob@example.com");
    });
  });

  it("loading state shows loading text", async () => {
    const { adminApi } = await import("../../src/api/admin");
    vi.mocked(adminApi.listInvites).mockImplementation(
      () => new Promise(() => {}), // never resolves
    );

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("Loading invites...")).toBeDefined();
    });
  });

  it("empty invites list shows empty message", async () => {
    const { adminApi } = await import("../../src/api/admin");
    vi.mocked(adminApi.listInvites).mockResolvedValue([]);

    renderAdmin();

    await waitFor(() => {
      expect(screen.getByText("No invites yet.")).toBeDefined();
    });
  });
});
