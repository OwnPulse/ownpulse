// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, beforeAll, afterAll, afterEach, beforeEach } from "vitest";
import { setupServer } from "msw/node";
import { http, HttpResponse } from "msw";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("auth multi-provider API", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  it("getAuthMethods GETs /api/v1/auth/methods", async () => {
    const methods = [
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ];

    server.use(
      http.get("/api/v1/auth/methods", () => {
        return HttpResponse.json(methods);
      }),
    );

    const { getAuthMethods } = await import("../../src/api/auth");
    const result = await getAuthMethods();

    expect(result).toEqual(methods);
  });

  it("unlinkAuth DELETEs /api/v1/auth/link/:provider", async () => {
    const remaining = [
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ];

    let capturedProvider: string | undefined;

    server.use(
      http.delete("/api/v1/auth/link/:provider", ({ params }) => {
        capturedProvider = params["provider"] as string;
        return HttpResponse.json(remaining);
      }),
    );

    const { unlinkAuth } = await import("../../src/api/auth");
    const result = await unlinkAuth("apple");

    expect(capturedProvider).toBe("apple");
    expect(result).toEqual(remaining);
  });
});
