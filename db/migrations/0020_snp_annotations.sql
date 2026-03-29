-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Reference SNP annotations for genetic interpretation.
-- Populated from public databases (ClinVar, PharmGKB).
-- NOT user data — shared across all users.
CREATE TABLE snp_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rsid TEXT NOT NULL,
    gene TEXT,
    category TEXT NOT NULL,           -- 'health_risk', 'trait', 'pharmacogenomics', 'carrier_status'
    title TEXT NOT NULL,              -- "MTHFR C677T Variant"
    summary TEXT NOT NULL,            -- "Associated with reduced folate metabolism"
    risk_allele TEXT,                 -- "T" or "CT"
    normal_allele TEXT,               -- "CC"
    significance TEXT NOT NULL,       -- 'pathogenic', 'likely_pathogenic', 'risk_factor', 'protective', 'benign', 'drug_response'
    evidence_level TEXT NOT NULL,     -- 'strong', 'moderate', 'limited', 'preliminary'
    source TEXT NOT NULL,             -- 'clinvar', 'pharmgkb', 'snpedia', 'gwas_catalog'
    source_id TEXT,                   -- ClinVar variation ID, PharmGKB annotation ID, etc.
    population_frequency DOUBLE PRECISION,  -- minor allele frequency
    details JSONB NOT NULL DEFAULT '{}',    -- additional structured data
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_snp_annotations_rsid_source ON snp_annotations(rsid, source);
CREATE INDEX idx_snp_annotations_rsid ON snp_annotations(rsid);
CREATE INDEX idx_snp_annotations_category ON snp_annotations(category);
CREATE INDEX idx_snp_annotations_gene ON snp_annotations(gene) WHERE gene IS NOT NULL;
