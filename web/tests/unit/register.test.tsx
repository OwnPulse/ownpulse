// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { render, screen } from "@testing-library/react";
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
    expect(screen.getByLabelText(/username/i)).toBeDefined();
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
});
