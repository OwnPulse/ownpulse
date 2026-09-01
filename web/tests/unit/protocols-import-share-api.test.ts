// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { ProtocolExport } from "../../src/api/protocols";
import { protocolsApi } from "../../src/api/protocols";
import { useAuthStore } from "../../src/store/auth";

const exportPayload: ProtocolExport = {
  schema: "ownpulse-protocol/v1",
  name: "BPC-157 Stack",
  description: "Healing protocol",
  tags: ["healing"],
  duration_days: 14,
  lines: [
    {
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      pattern: "daily",
    },
  ],
};

const importedProtocol = {
  id: "proto-imported",
  user_id: "user-1",
  name: "BPC-157 Stack",
  description: "Healing protocol",
  status: "draft",
  duration_days: 14,
  share_token: null,
  created_at: "2026-03-28T00:00:00Z", // date-ok
  lines: [],
};

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("protocolsApi - import/share", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("importFromFile", () => {
    it("posts the export payload to /api/v1/protocols/import", async () => {
      let capturedUrl: string | undefined;
      let capturedBody: unknown;

      server.use(
        http.post("/api/v1/protocols/import", async ({ request }) => {
          capturedUrl = request.url;
          capturedBody = await request.json();
          return HttpResponse.json(importedProtocol, { status: 201 });
        }),
      );

      const result = await protocolsApi.importFromFile(exportPayload);

      expect(result).toEqual(importedProtocol);
      expect(capturedUrl).toContain("/api/v1/protocols/import");
      expect(capturedUrl).not.toContain("import-file");
      expect(capturedBody).toEqual(exportPayload);
    });

    it("propagates a 401 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/import",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      await expect(protocolsApi.importFromFile(exportPayload)).rejects.toThrow("Unauthorized");
    });

    it("propagates a 403 error", async () => {
      server.use(
        http.post("/api/v1/protocols/import", () => new HttpResponse("Forbidden", { status: 403 })),
      );
      await expect(protocolsApi.importFromFile(exportPayload)).rejects.toThrow("Forbidden");
    });

    it("propagates a 500 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/import",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );
      await expect(protocolsApi.importFromFile(exportPayload)).rejects.toThrow(
        "Internal Server Error",
      );
    });
  });

  describe("importProtocol", () => {
    it("posts to /api/v1/protocols/import/:token with an empty body", async () => {
      let capturedUrl: string | undefined;
      let capturedToken: string | undefined;
      let capturedBody: unknown;

      server.use(
        http.post("/api/v1/protocols/import/:token", async ({ params, request }) => {
          capturedUrl = request.url;
          capturedToken = params.token as string;
          capturedBody = await request.json();
          return HttpResponse.json(importedProtocol, { status: 201 });
        }),
      );

      const result = await protocolsApi.importProtocol("share-tok-1");

      expect(result).toEqual(importedProtocol);
      expect(capturedToken).toBe("share-tok-1");
      expect(capturedUrl).toContain("/api/v1/protocols/import/share-tok-1");
      expect(capturedBody).toEqual({});
    });

    it("propagates a 401 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/import/:token",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      await expect(protocolsApi.importProtocol("tok")).rejects.toThrow("Unauthorized");
    });

    it("propagates a 403 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/import/:token",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      await expect(protocolsApi.importProtocol("tok")).rejects.toThrow("Forbidden");
    });

    it("propagates a 500 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/import/:token",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );
      await expect(protocolsApi.importProtocol("tok")).rejects.toThrow("Internal Server Error");
    });
  });

  describe("share", () => {
    it("returns { token, expires_at } from /api/v1/protocols/:id/share", async () => {
      server.use(
        http.post("/api/v1/protocols/:id/share", () =>
          HttpResponse.json({ token: "share-abc", expires_at: "2026-04-30T00:00:00Z" }), // date-ok
        ),
      );

      const result = await protocolsApi.share("proto-1");

      expect(result).toEqual({ token: "share-abc", expires_at: "2026-04-30T00:00:00Z" }); // date-ok
    });

    it("propagates a 401 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/share",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      await expect(protocolsApi.share("proto-1")).rejects.toThrow("Unauthorized");
    });

    it("propagates a 403 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/share",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      await expect(protocolsApi.share("proto-1")).rejects.toThrow("Forbidden");
    });

    it("propagates a 500 error", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/share",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );
      await expect(protocolsApi.share("proto-1")).rejects.toThrow("Internal Server Error");
    });
  });
});
