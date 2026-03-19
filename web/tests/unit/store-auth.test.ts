// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, beforeEach } from "vitest";
import { useAuthStore } from "../../src/store/auth";

describe("useAuthStore", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false });
  });

  it("starts unauthenticated", () => {
    const state = useAuthStore.getState();
    expect(state.token).toBeNull();
    expect(state.isAuthenticated).toBe(false);
  });

  it("login sets token and isAuthenticated", () => {
    useAuthStore.getState().login("test-jwt-token");
    const state = useAuthStore.getState();
    expect(state.token).toBe("test-jwt-token");
    expect(state.isAuthenticated).toBe(true);
  });

  it("logout clears both", () => {
    useAuthStore.getState().login("test-jwt-token");
    useAuthStore.getState().logout();
    const state = useAuthStore.getState();
    expect(state.token).toBeNull();
    expect(state.isAuthenticated).toBe(false);
  });
});
