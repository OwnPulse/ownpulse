// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::io::{BufRead, BufReader, Cursor};

use axum::Json;
use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db;
use crate::db::genetics as db_genetics;
use crate::error::ApiError;
use crate::genetics::interpret;
use crate::genetics::parser;
use crate::models::genetics::{
    DeleteConfirmation, GeneticSummary, GeneticsListQuery, GeneticsListResponse,
    InterpretationsQuery, InterpretationsResponse, UploadResult,
};
use crate::routes::events::publish_event;

/// Maximum upload file size: 50 MB.
const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// Batch size for bulk insert operations.
const INSERT_BATCH_SIZE: usize = 1000;

/// POST /genetics/upload — upload a genetic data file.
pub async fn upload(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResult>), ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("multipart error: {e}")))?
        .ok_or_else(|| ApiError::BadRequest("no file uploaded".to_string()))?;

    let _filename = field.file_name().map(|s| s.to_string());

    let bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::BadRequest(format!("failed to read upload: {e}")))?;

    if bytes.len() > MAX_FILE_SIZE {
        return Err(ApiError::BadRequest(format!(
            "file too large (max {} MB)",
            MAX_FILE_SIZE / (1024 * 1024)
        )));
    }

    if bytes.is_empty() {
        return Err(ApiError::BadRequest("empty file".to_string()));
    }

    // Detect format from first few lines
    let first_lines = read_first_lines(&bytes, 20);
    let format = parser::detect_format(&first_lines)?;

    // Parse all variants
    let reader: Box<dyn BufRead> = Box::new(BufReader::new(Cursor::new(bytes.to_vec())));
    let results = parser::parse_stream(reader, format);

    let mut variants = Vec::new();
    let mut parse_errors = 0u64;
    for result in results {
        match result {
            Ok(v) => variants.push(v),
            Err(_) => parse_errors += 1,
        }
    }

    if variants.is_empty() {
        return Err(ApiError::BadRequest(
            "no valid variants found in file".to_string(),
        ));
    }

    if parse_errors > 0 {
        tracing::info!(
            user_id = %user_id,
            parse_errors,
            valid_variants = variants.len(),
            "some lines failed to parse during genetic upload"
        );
    }

    let total_variants = variants.len() as i64;

    let insert_result = db_genetics::bulk_insert(
        &state.pool,
        user_id,
        &variants,
        format.source_name(),
        INSERT_BATCH_SIZE,
    )
    .await?;

    publish_event(&state.event_tx, user_id, "genetics", None);

    Ok((
        StatusCode::CREATED,
        Json(UploadResult {
            total_variants,
            new_variants: insert_result.inserted,
            duplicates_skipped: total_variants - insert_result.inserted,
            format: format.to_string(),
            source: format.source_name().to_string(),
        }),
    ))
}

/// GET /genetics — list genetic records with pagination.
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<GeneticsListQuery>,
) -> Result<Json<GeneticsListResponse>, ApiError> {
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);

    let (records, total) = db_genetics::list(
        &state.pool,
        user_id,
        page,
        per_page,
        query.chromosome.as_deref(),
        query.rsid.as_deref(),
    )
    .await?;

    Ok(Json(GeneticsListResponse {
        records,
        total,
        page,
        per_page,
    }))
}

/// GET /genetics/summary — summary statistics for the user's genetic data.
pub async fn summary(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<GeneticSummary>, ApiError> {
    let total_variants = db_genetics::count_total(&state.pool, user_id).await?;

    if total_variants == 0 {
        return Ok(Json(GeneticSummary {
            total_variants: 0,
            source: None,
            uploaded_at: None,
            chromosomes: std::collections::HashMap::new(),
            annotated_count: 0,
        }));
    }

    let source_info = db_genetics::summary_source(&state.pool, user_id).await?;
    let chr_counts = db_genetics::chromosome_counts(&state.pool, user_id).await?;
    let annotated = db_genetics::annotated_count(&state.pool, user_id).await?;

    let mut chromosomes = std::collections::HashMap::new();
    for cc in chr_counts {
        if let Some(chr) = cc.chromosome {
            chromosomes.insert(chr, cc.count.unwrap_or(0));
        }
    }

    Ok(Json(GeneticSummary {
        total_variants,
        source: source_info.as_ref().and_then(|s| s.source.clone()),
        uploaded_at: source_info.and_then(|s| s.uploaded_at),
        chromosomes,
        annotated_count: annotated,
    }))
}

/// GET /genetics/interpretations — user genotypes matched against annotation database.
pub async fn interpretations(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<InterpretationsQuery>,
) -> Result<Json<InterpretationsResponse>, ApiError> {
    let rows =
        db_genetics::interpretations(&state.pool, user_id, query.category.as_deref()).await?;

    let interpreted = interpret::interpret(rows);

    Ok(Json(InterpretationsResponse {
        interpretations: interpreted,
        disclaimer: interpret::DISCLAIMER.to_string(),
    }))
}

/// DELETE /genetics — delete all genetic records for the user.
pub async fn delete_all(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<DeleteConfirmation>,
) -> Result<StatusCode, ApiError> {
    if !body.confirm {
        return Err(ApiError::BadRequest(
            "must set confirm: true to delete all genetic data".to_string(),
        ));
    }

    let count = db_genetics::delete_all(&state.pool, user_id).await?;

    // Fire-and-forget audit log
    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(e) =
            db::audit::log_access(&pool, user_id, "delete", "genetics", None, None).await
        {
            tracing::warn!(error = %e, user_id = %user_id, "audit log insert failed");
        }
    });

    if count > 0 {
        publish_event(&state.event_tx, user_id, "genetics", None);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Read the first N lines from a byte buffer.
fn read_first_lines(bytes: &[u8], n: usize) -> Vec<String> {
    let reader = BufReader::new(Cursor::new(bytes));
    reader.lines().take(n).filter_map(|l| l.ok()).collect()
}
