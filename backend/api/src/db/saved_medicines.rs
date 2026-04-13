// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::saved_medicine::{CreateSavedMedicine, SavedMedicineRow, UpdateSavedMedicine};
use sqlx::PgPool;
use uuid::Uuid;

/// List saved medicines for a user, ordered by sort_order then created_at.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<SavedMedicineRow>, sqlx::Error> {
    sqlx::query_as::<_, SavedMedicineRow>(
        "SELECT id, user_id, substance, dose, unit, route, sort_order, created_at
         FROM saved_medicines
         WHERE user_id = $1
         ORDER BY sort_order, created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Insert a new saved medicine.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    data: &CreateSavedMedicine,
) -> Result<SavedMedicineRow, sqlx::Error> {
    sqlx::query_as::<_, SavedMedicineRow>(
        "INSERT INTO saved_medicines (user_id, substance, dose, unit, route)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, substance, dose, unit, route, sort_order, created_at",
    )
    .bind(user_id)
    .bind(&data.substance)
    .bind(data.dose)
    .bind(&data.unit)
    .bind(&data.route)
    .fetch_one(pool)
    .await
}

/// Update a saved medicine. Only non-None fields are applied.
/// Returns None if the row does not exist for this user.
pub async fn update(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    data: &UpdateSavedMedicine,
) -> Result<Option<SavedMedicineRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, SavedMedicineRow>(
        "UPDATE saved_medicines
         SET substance  = COALESCE($3, substance),
             dose       = COALESCE($4, dose),
             unit       = COALESCE($5, unit),
             route      = COALESCE($6, route),
             sort_order = COALESCE($7, sort_order)
         WHERE user_id = $1 AND id = $2
         RETURNING id, user_id, substance, dose, unit, route, sort_order, created_at",
    )
    .bind(user_id)
    .bind(id)
    .bind(&data.substance)
    .bind(data.dose)
    .bind(&data.unit)
    .bind(&data.route)
    .bind(data.sort_order)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Delete a saved medicine. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM saved_medicines WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
