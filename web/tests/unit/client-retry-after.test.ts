// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { ApiError, api } from "../../src/api/client";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("api client — Retry-After capture", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true, role: null });
  });

  it("attaches retryAfterSeconds from a 429's Retry-After header", async () => {
    server.use(
      http.post(
        "/api/v1/protected/sync",
        () =>
          new HttpResponse(JSON.stringify({ error: "rate limited" }), {
            status: 429,
            headers: { "retry-after": "12" },
          }),
      ),
    );

    await expect(api.post("/api/v1/protected/sync", {})).rejects.toMatchObject({
      name: "ApiError",
      status: 429,
      retryAfterSeconds: 12,
    });
  });

  it("leaves retryAfterSeconds undefined when the header is absent", async () => {
    server.use(
      http.post("/api/v1/protected/sync", () => new HttpResponse("Error", { status: 500 })),
    );

    const error = await api.post("/api/v1/protected/sync", {}).catch((e) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).retryAfterSeconds).toBeUndefined();
  });

  it("leaves retryAfterSeconds undefined for an HTTP-date form Retry-After (Number.isFinite guard)", async () => {
    // Retry-After is also legal as an HTTP-date (RFC 9110 10.2.3), not just
    // delay-seconds. `Number("Wed, 21 Oct 2026 07:28:00 GMT")` is `NaN`, and
    // the client should treat that as "no usable retry hint" rather than
    // surfacing NaN to callers.
    server.use(
      http.post(
        "/api/v1/protected/sync",
        () =>
          new HttpResponse(JSON.stringify({ error: "rate limited" }), {
            status: 429,
            headers: { "retry-after": "Wed, 21 Oct 2026 07:28:00 GMT" },
          }),
      ),
    );

    const error = await api.post("/api/v1/protected/sync", {}).catch((e) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(429);
    expect((error as ApiError).retryAfterSeconds).toBeUndefined();
  });
});
