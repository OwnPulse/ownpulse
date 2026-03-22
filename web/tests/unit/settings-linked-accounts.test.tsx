// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// --- mocks ---

const mockGetAuthMethods = vi.fn();
const mockUnlinkAuth = vi.fn();
const mockExportJson = vi.fn();
const mockExportCsv = vi.fn();
const mockSourcePreferencesApi = { list: vi.fn().mockResolvedValue([]) };
const mockAccountApi = { delete: vi.fn() };
const mockLogout = vi.fn();

vi.mock("../../src/api/auth", () => ({
  getAuthMethods: (...args: unknown[]) => mockGetAuthMethods(...args),
  unlinkAuth: (...args: unknown[]) => mockUnlinkAuth(...args),
  logout: (...args: unknown[]) => mockLogout(...args),
}));

vi.mock("../../src/api/export", () => ({
  exportJson: (...args: unknown[]) => mockExportJson(...args),
  exportCsv: (...args: unknown[]) => mockExportCsv(...args),
}));

vi.mock("../../src/api/source-preferences", () => ({
  sourcePreferencesApi: mockSourcePreferencesApi,
}));

vi.mock("../../src/api/account", () => ({
  accountApi: mockAccountApi,
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

// Lazy import to pick up mocks
async function renderSettings() {
  const { default: Settings } = await import("../../src/pages/Settings");
  return render(<Settings />, { wrapper });
}

describe("Settings — Linked Accounts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSourcePreferencesApi.list.mockResolvedValue([]);
  });

  it("shows linked accounts list", async () => {
    mockGetAuthMethods.mockResolvedValue([
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
      { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
    ]);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getByText("google")).toBeDefined();
      expect(screen.getByText("apple")).toBeDefined();
      expect(screen.getByText("user@example.com")).toBeDefined();
    });
  });

  it("shows Unlink buttons only when more than one method exists", async () => {
    mockGetAuthMethods.mockResolvedValue([
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
      { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
    ]);

    await renderSettings();

    await waitFor(() => {
      const unlinkBtns = screen.getAllByRole("button", { name: /unlink/i });
      expect(unlinkBtns.length).toBe(2);
    });
  });

  it("hides Unlink button when only one method exists", async () => {
    mockGetAuthMethods.mockResolvedValue([
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ]);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getByText("google")).toBeDefined();
    });
    expect(screen.queryByRole("button", { name: /unlink/i })).toBeNull();
  });

  it("calls unlinkAuth when Unlink is clicked", async () => {
    mockGetAuthMethods.mockResolvedValue([
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
      { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
    ]);
    mockUnlinkAuth.mockResolvedValue([
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ]);

    await renderSettings();

    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /unlink/i }).length).toBe(2);
    });

    const user = userEvent.setup();
    // Click the first Unlink button (google or apple depending on order)
    await user.click(screen.getAllByRole("button", { name: /unlink/i })[0]);

    await waitFor(() => {
      expect(mockUnlinkAuth).toHaveBeenCalledOnce();
    });
  });
});
