// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { useAuthStore } from "../../src/store/auth";
import Login from "../../src/pages/Login";

vi.mock("../../src/hooks/useAuth", () => ({
  useAuth: () => ({ loading: false }),
}));

const mockLogin = vi.fn();
vi.mock("../../src/api/auth", () => ({
  login: (...args: unknown[]) => mockLogin(...args),
}));

describe("Login", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false });
    mockLogin.mockReset();
  });

  it("renders email/password inputs, Google link, and Apple link", () => {
    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText(/email/i)).toBeDefined();
    expect(screen.getByLabelText(/password/i)).toBeDefined();

    const googleLink = screen.getByText(/sign in with google/i);
    expect(googleLink).toBeDefined();
    // Bugfix: Google OAuth href was previously /callback, now correctly points to /login
    expect(googleLink).toHaveAttribute("href", "/api/v1/auth/google/login");

    const appleLink = screen.getByText(/sign in with apple/i);
    expect(appleLink).toBeDefined();
    expect(appleLink).toHaveAttribute("href", "/api/v1/auth/apple/login");
  });

  it("shows error on failed login", async () => {
    mockLogin.mockRejectedValue(new Error("bad credentials"));

    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>,
    );

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/email/i), "test@example.com");
    await user.type(screen.getByLabelText(/password/i), "wrongpassword");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/invalid email or password/i),
      ).toBeDefined();
    });
  });
});
