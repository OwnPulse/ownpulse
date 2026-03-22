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

  it("renders email/password inputs and Google link", () => {
    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText(/email/i)).toBeDefined();
    expect(screen.getByLabelText(/password/i)).toBeDefined();
    expect(screen.getByText(/sign in with google/i)).toBeDefined();
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
