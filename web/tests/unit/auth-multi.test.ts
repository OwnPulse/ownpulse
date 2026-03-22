// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
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
      {
        id: "1",
        provider: "google",
        email: "user@example.com",
        created_at: "2026-01-01T00:00:00Z",
      },
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
      {
        id: "1",
        provider: "google",
        email: "user@example.com",
        created_at: "2026-01-01T00:00:00Z",
      },
    ];

    let capturedProvider: string | undefined;

    server.use(
      http.delete("/api/v1/auth/link/:provider", ({ params }) => {
        capturedProvider = params.provider as string;
        return HttpResponse.json(remaining);
      }),
    );

    const { unlinkAuth } = await import("../../src/api/auth");
    const result = await unlinkAuth("apple");

    expect(capturedProvider).toBe("apple");
    expect(result).toEqual(remaining);
  });

  it("getAuthMethods throws on 401 and triggers logout", async () => {
    server.use(
      http.get("/api/v1/auth/methods", () => new HttpResponse("Unauthorized", { status: 401 })),
    );

    const { getAuthMethods } = await import("../../src/api/auth");

    await expect(getAuthMethods()).rejects.toThrow("Unauthorized");
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });

  it("getAuthMethods throws ApiError on 500", async () => {
    server.use(
      http.get(
        "/api/v1/auth/methods",
        () => new HttpResponse("Internal Server Error", { status: 500 }),
      ),
    );

    const { getAuthMethods } = await import("../../src/api/auth");

    await expect(getAuthMethods()).rejects.toMatchObject({
      name: "ApiError",
      status: 500,
    });
  });

  it("unlinkAuth throws on 401 and triggers logout", async () => {
    server.use(
      http.delete(
        "/api/v1/auth/link/:provider",
        () => new HttpResponse("Unauthorized", { status: 401 }),
      ),
    );

    const { unlinkAuth } = await import("../../src/api/auth");

    await expect(unlinkAuth("google")).rejects.toThrow("Unauthorized");
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });

  it("unlinkAuth throws ApiError on 500", async () => {
    server.use(
      http.delete(
        "/api/v1/auth/link/:provider",
        () => new HttpResponse("Server error", { status: 500 }),
      ),
    );

    const { unlinkAuth } = await import("../../src/api/auth");

    await expect(unlinkAuth("google")).rejects.toMatchObject({
      name: "ApiError",
      status: 500,
    });
  });

  it("unlinkAuth throws on invalid provider name", async () => {
    const { unlinkAuth } = await import("../../src/api/auth");

    await expect(unlinkAuth("goo gle")).rejects.toThrow("Invalid provider");
    await expect(unlinkAuth("../admin")).rejects.toThrow("Invalid provider");
    await expect(unlinkAuth("GOOGLE")).rejects.toThrow("Invalid provider");
  });
});
