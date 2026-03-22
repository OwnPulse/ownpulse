// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("register API", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false, role: null });
  });

  it("register POSTs email, password, invite_code and stores token", async () => {
    let capturedBody: Record<string, unknown> | undefined;

    server.use(
      http.post("/api/v1/auth/register", async ({ request }) => {
        capturedBody = (await request.json()) as Record<string, unknown>;
        return HttpResponse.json({
          access_token: "new-jwt",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    const { register } = await import("../../src/api/auth");
    await register("user@example.com", "securepass", "INVITE123");

    expect(capturedBody).toEqual({
      email: "user@example.com",
      password: "securepass",
      invite_code: "INVITE123",
    });
    expect(useAuthStore.getState().token).toBe("new-jwt");
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
  });

  it("register throws on 400 (invalid invite code)", async () => {
    server.use(
      http.post("/api/v1/auth/register", () =>
        new HttpResponse("Invalid invite code", { status: 400 }),
      ),
    );

    const { register } = await import("../../src/api/auth");

    await expect(register("user@example.com", "securepass", "BAD")).rejects.toMatchObject({
      name: "ApiError",
      status: 400,
    });
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });

  it("register throws on 409 (duplicate email)", async () => {
    server.use(
      http.post("/api/v1/auth/register", () =>
        new HttpResponse("Email already registered", { status: 409 }),
      ),
    );

    const { register } = await import("../../src/api/auth");

    await expect(register("taken@example.com", "securepass", "INVITE123")).rejects.toMatchObject({
      name: "ApiError",
      status: 409,
    });
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });
});
