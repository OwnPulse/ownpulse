// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { geneticsApi } from "../../src/api/genetics";
import { useAuthStore } from "../../src/store/auth";

const mockSummary = {
  total_variants: 650000,
  source: "23andMe",
  uploaded_at: "2026-03-20T10:00:00Z",
  chromosomes: { "1": 50000, "2": 45000, X: 20000 },
  annotated_count: 42,
};

const mockListResponse = {
  records: [
    {
      rsid: "rs1801133",
      chromosome: "1",
      position: 11856378,
      genotype: "CT",
      created_at: "2026-03-20T10:00:00Z",
    },
    {
      rsid: "rs4680",
      chromosome: "22",
      position: 19963748,
      genotype: "AG",
      created_at: "2026-03-20T10:00:00Z",
    },
  ],
  total: 650000,
  page: 1,
  per_page: 50,
};

const mockInterpretations = {
  interpretations: [
    {
      rsid: "rs1801133",
      gene: "MTHFR",
      chromosome: "1",
      position: 11856378,
      user_genotype: "CT",
      category: "health_risk",
      title: "MTHFR C677T Variant",
      summary: "You carry one copy of the C677T variant.",
      risk_level: "moderate",
      significance: "Associated with reduced folate metabolism",
      evidence_level: "strong",
      source: "ClinVar",
      source_id: "3520",
      population_frequency: 0.34,
      details: {},
    },
  ],
  disclaimer: "For educational purposes only.",
};

const mockUploadResult = {
  total_variants: 650000,
  new_variants: 649500,
  duplicates_skipped: 500,
  format: "23andMe_v5",
  source: "23andMe",
};

const server = setupServer(
  http.get("/api/v1/genetics/summary", () => {
    return HttpResponse.json(mockSummary);
  }),
  http.get("/api/v1/genetics", () => {
    return HttpResponse.json(mockListResponse);
  }),
  http.get("/api/v1/genetics/interpretations", () => {
    return HttpResponse.json(mockInterpretations);
  }),
  http.post("/api/v1/genetics/upload", () => {
    return HttpResponse.json(mockUploadResult);
  }),
  http.delete("/api/v1/genetics", () => {
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("geneticsApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("summary", () => {
    it("fetches genetic summary", async () => {
      const result = await geneticsApi.summary();
      expect(result.total_variants).toBe(650000);
      expect(result.source).toBe("23andMe");
      expect(result.chromosomes["1"]).toBe(50000);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/genetics/summary", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(geneticsApi.summary()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/genetics/summary", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(geneticsApi.summary()).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/genetics/summary", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(geneticsApi.summary()).rejects.toThrow("Forbidden");
    });
  });

  describe("list", () => {
    it("fetches genetic records without params", async () => {
      const result = await geneticsApi.list();
      expect(result.records).toHaveLength(2);
      expect(result.total).toBe(650000);
      expect(result.records[0].rsid).toBe("rs1801133");
    });

    it("fetches with chromosome filter", async () => {
      server.use(
        http.get("/api/v1/genetics", ({ request }) => {
          const url = new URL(request.url);
          expect(url.searchParams.get("chromosome")).toBe("1");
          return HttpResponse.json({
            records: [mockListResponse.records[0]],
            total: 50000,
            page: 1,
            per_page: 50,
          });
        }),
      );
      const result = await geneticsApi.list({ chromosome: "1" });
      expect(result.records).toHaveLength(1);
    });

    it("fetches with rsid search", async () => {
      server.use(
        http.get("/api/v1/genetics", ({ request }) => {
          const url = new URL(request.url);
          expect(url.searchParams.get("rsid")).toBe("rs1801133");
          return HttpResponse.json({
            records: [mockListResponse.records[0]],
            total: 1,
            page: 1,
            per_page: 50,
          });
        }),
      );
      const result = await geneticsApi.list({ rsid: "rs1801133" });
      expect(result.records).toHaveLength(1);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/genetics", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(geneticsApi.list()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/genetics", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(geneticsApi.list()).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/genetics", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(geneticsApi.list()).rejects.toThrow("Forbidden");
    });
  });

  describe("interpretations", () => {
    it("fetches all interpretations", async () => {
      const result = await geneticsApi.interpretations();
      expect(result.interpretations).toHaveLength(1);
      expect(result.disclaimer).toBe("For educational purposes only.");
    });

    it("fetches filtered by category", async () => {
      server.use(
        http.get("/api/v1/genetics/interpretations", ({ request }) => {
          const url = new URL(request.url);
          expect(url.searchParams.get("category")).toBe("health_risk");
          return HttpResponse.json(mockInterpretations);
        }),
      );
      const result = await geneticsApi.interpretations("health_risk");
      expect(result.interpretations).toHaveLength(1);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/genetics/interpretations", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(geneticsApi.interpretations()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/genetics/interpretations", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(geneticsApi.interpretations()).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/genetics/interpretations", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(geneticsApi.interpretations()).rejects.toThrow("Forbidden");
    });
  });

  describe("upload", () => {
    it("uploads a genetic file", async () => {
      server.use(
        http.post("/api/v1/genetics/upload", async ({ request }) => {
          const contentType = request.headers.get("content-type");
          expect(contentType).toContain("multipart/form-data");
          expect(request.headers.get("authorization")).toBe("Bearer test-jwt-token");
          return HttpResponse.json(mockUploadResult);
        }),
      );

      const file = new File(["rsid\tchromosome\tposition\tgenotype\n"], "genome.txt", {
        type: "text/plain",
      });
      const result = await geneticsApi.upload(file);
      expect(result.total_variants).toBe(650000);
      expect(result.format).toBe("23andMe_v5");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/genetics/upload", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      const file = new File(["data"], "genome.txt");
      await expect(geneticsApi.upload(file)).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/genetics/upload", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      const file = new File(["data"], "genome.txt");
      await expect(geneticsApi.upload(file)).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.post("/api/v1/genetics/upload", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      const file = new File(["data"], "genome.txt");
      await expect(geneticsApi.upload(file)).rejects.toThrow("Forbidden");
    });
  });

  describe("deleteAll", () => {
    it("deletes all genetic data", async () => {
      await expect(geneticsApi.deleteAll()).resolves.not.toThrow();
    });

    it("handles 401 error", async () => {
      server.use(
        http.delete("/api/v1/genetics", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(geneticsApi.deleteAll()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.delete("/api/v1/genetics", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(geneticsApi.deleteAll()).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.delete("/api/v1/genetics", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(geneticsApi.deleteAll()).rejects.toThrow("Forbidden");
    });
  });
});
