// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Database access functions for genetic records and SNP annotations.

use sqlx::PgPool;
use uuid::Uuid;

use crate::genetics::parser::ParsedVariant;
use crate::models::genetics::{ChromosomeCount, GeneticRecordRow, InterpretationRow, SourceInfo};

/// Result of a bulk insert operation.
pub struct InsertResult {
    pub total: i64,
    pub inserted: i64,
}

/// Bulk insert genetic variants in batches of `batch_size`.
/// Uses ON CONFLICT DO NOTHING for rsid dedup per user.
pub async fn bulk_insert(
    pool: &PgPool,
    user_id: Uuid,
    variants: &[ParsedVariant],
    source: &str,
    batch_size: usize,
) -> Result<InsertResult, sqlx::Error> {
    let total = variants.len() as i64;
    let mut inserted: i64 = 0;

    for chunk in variants.chunks(batch_size) {
        let count = insert_batch(pool, user_id, chunk, source).await?;
        inserted += count;
    }

    Ok(InsertResult { total, inserted })
}

/// Insert a single batch of variants. Returns number of rows actually inserted.
async fn insert_batch(
    pool: &PgPool,
    user_id: Uuid,
    variants: &[ParsedVariant],
    source: &str,
) -> Result<i64, sqlx::Error> {
    if variants.is_empty() {
        return Ok(0);
    }

    // Build a multi-value INSERT dynamically.
    // Values: (user_id, source, rsid, chromosome, position, genotype)
    let mut query = String::from(
        "INSERT INTO genetic_records (user_id, source, rsid, chromosome, position, genotype) VALUES ",
    );

    let mut params_idx = 1u32;
    for (i, _) in variants.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "(${}, ${}, ${}, ${}, ${}, ${})",
            params_idx,
            params_idx + 1,
            params_idx + 2,
            params_idx + 3,
            params_idx + 4,
            params_idx + 5,
        ));
        params_idx += 6;
    }
    query.push_str(" ON CONFLICT (user_id, rsid) DO NOTHING");

    let mut q = sqlx::query(&query);
    for variant in variants {
        q = q
            .bind(user_id)
            .bind(source)
            .bind(&variant.rsid)
            .bind(&variant.chromosome)
            .bind(variant.position)
            .bind(&variant.genotype);
    }

    let result = q.execute(pool).await?;
    Ok(result.rows_affected() as i64)
}

/// List genetic records with pagination and optional filters.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    per_page: i64,
    chromosome: Option<&str>,
    rsid_search: Option<&str>,
) -> Result<(Vec<GeneticRecordRow>, i64), sqlx::Error> {
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM genetic_records
         WHERE user_id = $1
           AND ($2::text IS NULL OR chromosome = $2)
           AND ($3::text IS NULL OR rsid LIKE '%' || $3 || '%')",
    )
    .bind(user_id)
    .bind(chromosome)
    .bind(rsid_search)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query_as::<_, GeneticRecordRow>(
        "SELECT id, user_id, source, rsid, chromosome, position, genotype,
                uploaded_file_id, created_at
         FROM genetic_records
         WHERE user_id = $1
           AND ($2::text IS NULL OR chromosome = $2)
           AND ($3::text IS NULL OR rsid LIKE '%' || $3 || '%')
         ORDER BY chromosome, position
         LIMIT $4 OFFSET $5",
    )
    .bind(user_id)
    .bind(chromosome)
    .bind(rsid_search)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows, total.0))
}

/// Get summary statistics for a user's genetic data.
pub async fn summary_source(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<SourceInfo>, sqlx::Error> {
    sqlx::query_as::<_, SourceInfo>(
        "SELECT source, MIN(created_at) as uploaded_at
         FROM genetic_records
         WHERE user_id = $1
         GROUP BY source
         ORDER BY uploaded_at
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Count total variants for a user.
pub async fn count_total(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM genetic_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Count variants per chromosome.
pub async fn chromosome_counts(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ChromosomeCount>, sqlx::Error> {
    sqlx::query_as::<_, ChromosomeCount>(
        "SELECT chromosome, COUNT(*) as count
         FROM genetic_records
         WHERE user_id = $1 AND chromosome IS NOT NULL
         GROUP BY chromosome
         ORDER BY chromosome",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Count how many of the user's variants have annotations in snp_annotations.
pub async fn annotated_count(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT gr.rsid)
         FROM genetic_records gr
         INNER JOIN snp_annotations sa ON gr.rsid = sa.rsid
         WHERE gr.user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Get interpretations: join user genetic records with snp_annotations.
pub async fn interpretations(
    pool: &PgPool,
    user_id: Uuid,
    category: Option<&str>,
) -> Result<Vec<InterpretationRow>, sqlx::Error> {
    sqlx::query_as::<_, InterpretationRow>(
        "SELECT sa.rsid, sa.gene, gr.chromosome, gr.position,
                gr.genotype as user_genotype,
                sa.category, sa.title, sa.summary,
                sa.risk_allele, sa.normal_allele,
                sa.significance, sa.evidence_level,
                sa.source, sa.source_id,
                sa.population_frequency, sa.details
         FROM genetic_records gr
         INNER JOIN snp_annotations sa ON gr.rsid = sa.rsid
         WHERE gr.user_id = $1
           AND ($2::text IS NULL OR sa.category = $2)
         ORDER BY sa.category, sa.gene, sa.rsid",
    )
    .bind(user_id)
    .bind(category)
    .fetch_all(pool)
    .await
}

/// Delete all genetic records for a user. Returns the count deleted.
pub async fn delete_all(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM genetic_records WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() as i64)
}
