// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useAuthStore } from "../store/auth";
import { api } from "./client";

export interface GeneticRecord {
  rsid: string;
  chromosome: string;
  position: number;
  genotype: string;
  created_at: string;
}

export interface GeneticListResponse {
  records: GeneticRecord[];
  total: number;
  page: number;
  per_page: number;
}

export interface GeneticSummary {
  total_variants: number;
  source: string | null;
  uploaded_at: string | null;
  chromosomes: Record<string, number>;
  annotated_count: number;
}

export interface UploadResult {
  total_variants: number;
  new_variants: number;
  duplicates_skipped: number;
  format: string;
  source: string;
}

export interface Interpretation {
  rsid: string;
  gene: string | null;
  chromosome: string;
  position: number;
  user_genotype: string;
  category: "health_risk" | "trait" | "pharmacogenomics" | "carrier_status";
  title: string;
  summary: string;
  risk_level:
    | "high"
    | "moderate"
    | "low"
    | "normal"
    | "poor_metabolizer"
    | "intermediate"
    | "rapid";
  significance: string;
  evidence_level: "strong" | "moderate" | "limited" | "preliminary";
  source: string;
  source_id: string | null;
  population_frequency: number | null;
  details: Record<string, unknown>;
}

export interface InterpretationsResponse {
  interpretations: Interpretation[];
  disclaimer: string;
}

/**
 * Upload genetic data file (23andMe, AncestryDNA, VCF).
 * Uses fetch directly because the shared api client forces JSON Content-Type,
 * which is incompatible with FormData/multipart uploads.
 * Auth token is read from the Zustand auth store (in-memory only).
 */
async function uploadGeneticFile(file: File): Promise<UploadResult> {
  const token = useAuthStore.getState().token;
  const formData = new FormData();
  formData.append("file", file);

  const headers: Record<string, string> = {};
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch("/api/v1/genetics/upload", {
    method: "POST",
    headers,
    body: formData,
    credentials: "include",
  });

  if (response.status === 401) {
    useAuthStore.getState().logout();
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    const body = await response.text();
    throw new Error(body);
  }

  return response.json() as Promise<UploadResult>;
}

export const geneticsApi = {
  upload: uploadGeneticFile,

  list: (params?: { page?: number; per_page?: number; chromosome?: string; rsid?: string }) => {
    const searchParams = new URLSearchParams();
    if (params?.page) searchParams.set("page", String(params.page));
    if (params?.per_page) searchParams.set("per_page", String(params.per_page));
    if (params?.chromosome) searchParams.set("chromosome", params.chromosome);
    if (params?.rsid) searchParams.set("rsid", params.rsid);
    const qs = searchParams.toString();
    return api.get<GeneticListResponse>(`/api/v1/genetics${qs ? `?${qs}` : ""}`);
  },

  summary: () => api.get<GeneticSummary>("/api/v1/genetics/summary"),

  interpretations: (category?: string) => {
    const url = category
      ? `/api/v1/genetics/interpretations?category=${category}`
      : "/api/v1/genetics/interpretations";
    return api.get<InterpretationsResponse>(url);
  },

  deleteAll: () => api.delete<void>("/api/v1/genetics"),
};
