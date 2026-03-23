// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Register from "../../src/pages/Register";
import { useAuthStore } from "../../src/store/auth";

vi.mock("../../src/hooks/useAuth", () => ({
  useAuth: () => ({ loading: false }),
}));

const mockRegister = vi.fn();
vi.mock("../../src/api/auth", () => ({
  register: (...args: unknown[]) => mockRegister(...args),
}));

function renderRegister(initialRoute = "/register") {
  return render(
    <MemoryRouter initialEntries={[initialRoute]}>
      <Register />
    </MemoryRouter>,
  );
}

describe("Register", () => {
  beforeEach(() => {
    useAuthStore.setState({
      token: null,
      isAuthenticated: false,
      role: null,
    });
    mockRegister.mockReset();
  });

  it("renders form fields with invite code from URL", () => {
    renderRegister("/register?invite=ABC123");

    expect(screen.getByLabelText(/invite code/i)).toBeDefined();
    expect((screen.getByLabelText(/invite code/i) as HTMLInputElement).value).toBe("ABC123");
    expect(screen.getByLabelText(/email/i)).toBeDefined();
    expect(screen.getByLabelText(/^password$/i)).toBeDefined();
    expect(screen.getByLabelText(/confirm password/i)).toBeDefined();
    expect(screen.getByRole("button", { name: /create account/i })).toBeDefined();
    expect(screen.getByText(/sign up with google/i)).toBeDefined();
  });

  it("shows message when no invite code is provided", () => {
    renderRegister("/register");

    expect(screen.getByText(/you need an invite code to sign up/i)).toBeDefined();
    expect(screen.getByText(/already have an account\? sign in/i)).toBeDefined();
  });

  it("shows sign-in link", () => {
    renderRegister("/register?invite=ABC123");

    expect(screen.getByText(/already have an account\? sign in/i)).toBeDefined();
  });

  it("calls register and navigates on successful submission", async () => {
    mockRegister.mockResolvedValue(undefined);

    renderRegister("/register?invite=ABC123");

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/email/i), "new@example.com");
    await user.type(screen.getByLabelText(/^password$/i), "securepassword");
    await user.type(screen.getByLabelText(/confirm password/i), "securepassword");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    await waitFor(() => {
      expect(mockRegister).toHaveBeenCalledWith("new@example.com", "securepassword", "ABC123");
    });
  });

  it("shows error and does not call register when passwords do not match", async () => {
    renderRegister("/register?invite=ABC123");

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/email/i), "new@example.com");
    await user.type(screen.getByLabelText(/^password$/i), "securepassword");
    await user.type(screen.getByLabelText(/confirm password/i), "differentpassword");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    await waitFor(() => {
      expect(screen.getByText(/passwords do not match/i)).toBeDefined();
    });
    expect(mockRegister).not.toHaveBeenCalled();
  });

  it("shows error message when register API rejects", async () => {
    mockRegister.mockRejectedValue(new Error("server error"));

    renderRegister("/register?invite=ABC123");

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/email/i), "new@example.com");
    await user.type(screen.getByLabelText(/^password$/i), "securepassword");
    await user.type(screen.getByLabelText(/confirm password/i), "securepassword");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    await waitFor(() => {
      expect(screen.getByText(/registration failed/i)).toBeDefined();
    });
  });
});
