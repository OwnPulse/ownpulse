// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ForgotPassword from "../../src/pages/ForgotPassword";
import ResetPassword from "../../src/pages/ResetPassword";

const mockForgotPassword = vi.fn();
const mockResetPassword = vi.fn();

vi.mock("../../src/api/auth", () => ({
  forgotPassword: (...args: unknown[]) => mockForgotPassword(...args),
  resetPassword: (...args: unknown[]) => mockResetPassword(...args),
}));

describe("ForgotPassword", () => {
  beforeEach(() => {
    mockForgotPassword.mockReset();
  });

  it("renders email form", () => {
    render(
      <MemoryRouter>
        <ForgotPassword />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText(/email/i)).toBeDefined();
    expect(screen.getByRole("button", { name: /send reset link/i })).toBeDefined();
  });

  it("shows success message after submit", async () => {
    mockForgotPassword.mockResolvedValue(undefined);

    render(
      <MemoryRouter>
        <ForgotPassword />
      </MemoryRouter>,
    );

    const emailInput = screen.getByLabelText(/email/i);
    await userEvent.type(emailInput, "test@example.com");
    await userEvent.click(screen.getByRole("button", { name: /send reset link/i }));

    await waitFor(() => {
      expect(screen.getByText(/check your inbox/i)).toBeDefined();
    });

    expect(mockForgotPassword).toHaveBeenCalledWith("test@example.com");
  });
});

describe("ResetPassword", () => {
  beforeEach(() => {
    mockResetPassword.mockReset();
  });

  it("shows error for missing token", () => {
    render(
      <MemoryRouter initialEntries={["/reset-password"]}>
        <ResetPassword />
      </MemoryRouter>,
    );

    expect(screen.getByText(/invalid reset link/i)).toBeDefined();
    expect(screen.getByText(/missing a token/i)).toBeDefined();
  });
});
