// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! CSV export of health records, streamed as a single response body.

use axum::body::{Body, Bytes};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::health_record::HealthRecordRow;

/// Build a streaming CSV export body containing all health records for the
/// given user.
///
/// The CSV includes a header row followed by one row per health record:
/// `id,source,record_type,value,unit,start_time,end_time`
pub async fn stream_csv_export(pool: &PgPool, user_id: Uuid) -> Result<Body, sqlx::Error> {
    let records = sqlx::query_as::<_, HealthRecordRow>(
        "SELECT id, user_id, source, record_type, value, unit, start_time, \
         end_time, metadata, source_id, source_instance, duplicate_of, \
         healthkit_written, created_at \
         FROM health_records WHERE user_id = $1 ORDER BY start_time",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut csv = String::from("id,source,record_type,value,unit,start_time,end_time\n");

    for r in &records {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            r.id,
            escape_csv(&r.source),
            escape_csv(&r.record_type),
            r.value.map_or(String::new(), |v| v.to_string()),
            r.unit.as_deref().unwrap_or(""),
            r.start_time.to_rfc3339(),
            r.end_time.map_or(String::new(), |t| t.to_rfc3339()),
        ));
    }

    let stream =
        futures::stream::once(async move { Ok::<Bytes, std::io::Error>(Bytes::from(csv)) });

    Ok(Body::from_stream(stream))
}

/// Minimal CSV escaping: if the value contains a comma, quote, or newline,
/// wrap it in double quotes and escape internal quotes.
fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
