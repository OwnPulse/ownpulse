// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A row from the `genetic_records` table.
#[derive(FromRow, Serialize)]
pub struct GeneticRecordRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source: String,
    pub rsid: Option<String>,
    pub chromosome: Option<String>,
    pub position: Option<i64>,
    pub genotype: Option<String>,
    pub uploaded_file_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Result of a genetic data upload.
#[derive(Serialize)]
pub struct UploadResult {
    pub total_variants: i64,
    pub new_variants: i64,
    pub duplicates_skipped: i64,
    pub format: String,
    pub source: String,
}

/// Query params for listing genetic records.
#[derive(Deserialize)]
pub struct GeneticsListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    pub chromosome: Option<String>,
    pub rsid: Option<String>,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    50
}

/// Response for the genetics list endpoint.
#[derive(Serialize)]
pub struct GeneticsListResponse {
    pub records: Vec<GeneticRecordRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Summary of a user's genetic data.
#[derive(Serialize)]
pub struct GeneticSummary {
    pub total_variants: i64,
    pub source: Option<String>,
    pub uploaded_at: Option<DateTime<Utc>>,
    pub chromosomes: std::collections::HashMap<String, i64>,
    pub annotated_count: i64,
}

/// Query params for the interpretations endpoint.
#[derive(Deserialize)]
pub struct InterpretationsQuery {
    pub category: Option<String>,
}

/// A single interpreted variant — the user's genotype joined with annotation data.
#[derive(FromRow, Serialize, Clone)]
pub struct InterpretationRow {
    pub rsid: String,
    pub gene: Option<String>,
    pub chromosome: Option<String>,
    pub position: Option<i64>,
    pub user_genotype: Option<String>,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub risk_allele: Option<String>,
    pub normal_allele: Option<String>,
    pub significance: String,
    pub evidence_level: String,
    pub source: String,
    pub source_id: Option<String>,
    pub population_frequency: Option<f64>,
    pub details: serde_json::Value,
}

/// A fully interpreted variant with computed risk level and personalized summary.
#[derive(Serialize)]
pub struct Interpretation {
    pub rsid: String,
    pub gene: Option<String>,
    pub chromosome: Option<String>,
    pub position: Option<i64>,
    pub user_genotype: Option<String>,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub risk_level: String,
    pub significance: String,
    pub evidence_level: String,
    pub source: String,
    pub source_id: Option<String>,
    pub population_frequency: Option<f64>,
    pub details: serde_json::Value,
}

/// Response for the interpretations endpoint.
#[derive(Serialize)]
pub struct InterpretationsResponse {
    pub interpretations: Vec<Interpretation>,
    pub disclaimer: String,
}

/// Request body for delete confirmation.
#[derive(Deserialize)]
pub struct DeleteConfirmation {
    pub confirm: bool,
}

/// DB row for chromosome count aggregation.
#[derive(FromRow)]
pub struct ChromosomeCount {
    pub chromosome: Option<String>,
    pub count: Option<i64>,
}

/// DB row for summary source info.
#[derive(FromRow)]
pub struct SourceInfo {
    pub source: Option<String>,
    pub uploaded_at: Option<DateTime<Utc>>,
}
