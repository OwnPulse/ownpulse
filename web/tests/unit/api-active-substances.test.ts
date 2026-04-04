// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { protocolsApi } from "../../src/api/protocols";
import { useAuthStore } from "../../src/store/auth";

const substances = [
  {
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    protocol_name: "BPC Stack",
    protocol_id: "proto-1",
  },
  {
    substance: "TB-500",
    dose: 2,
    unit: "mg",
    route: "SubQ",
    protocol_name: "BPC Stack",
    protocol_id: "proto-1",
  },
];

const server = setupServer(
  http.get("/api/v1/protocols/active-substances", () => {
    return HttpResponse.json(substances);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("protocolsApi.activeSubstances", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  it("fetches active substances", async () => {
    const result = await protocolsApi.activeSubstances();
    expect(result).toHaveLength(2);
    expect(result[0].substance).toBe("BPC-157");
    expect(result[0].dose).toBe(250);
    expect(result[0].unit).toBe("mcg");
    expect(result[0].route).toBe("SubQ");
    expect(result[1].substance).toBe("TB-500");
  });

  it("handles 401 error", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return new HttpResponse("Unauthorized", { status: 401 });
      }),
    );
    await expect(protocolsApi.activeSubstances()).rejects.toThrow("Unauthorized");
  });

  it("handles 403 error", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return new HttpResponse("Forbidden", { status: 403 });
      }),
    );
    await expect(protocolsApi.activeSubstances()).rejects.toThrow("Forbidden");
  });

  it("handles 500 error", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return new HttpResponse("Internal Server Error", { status: 500 });
      }),
    );
    await expect(protocolsApi.activeSubstances()).rejects.toThrow("Internal Server Error");
  });

  it("returns empty array when no active substances", async () => {
    server.use(
      http.get("/api/v1/protocols/active-substances", () => {
        return HttpResponse.json([]);
      }),
    );
    const result = await protocolsApi.activeSubstances();
    expect(result).toEqual([]);
  });
});
