// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct SavedMedicineRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSavedMedicine {
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSavedMedicine {
    pub substance: Option<String>,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub sort_order: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_saved_medicine_serde_roundtrip() {
        let json = serde_json::json!({
            "substance": "caffeine",
            "dose": 200.0,
            "unit": "mg",
            "route": "oral"
        });
        let parsed: CreateSavedMedicine = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed.substance, "caffeine");
        assert_eq!(parsed.dose, Some(200.0));
        assert_eq!(parsed.unit.as_deref(), Some("mg"));
        assert_eq!(parsed.route.as_deref(), Some("oral"));

        // Minimal — only required field
        let minimal = serde_json::json!({"substance": "melatonin"});
        let parsed: CreateSavedMedicine = serde_json::from_value(minimal).unwrap();
        assert_eq!(parsed.substance, "melatonin");
        assert!(parsed.dose.is_none());
        assert!(parsed.unit.is_none());
        assert!(parsed.route.is_none());
    }

    #[test]
    fn test_update_saved_medicine_serde_roundtrip() {
        // All fields present
        let json = serde_json::json!({
            "substance": "updated",
            "dose": 100.0,
            "unit": "mcg",
            "route": "sublingual",
            "sort_order": 5
        });
        let parsed: UpdateSavedMedicine = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.substance.as_deref(), Some("updated"));
        assert_eq!(parsed.dose, Some(100.0));
        assert_eq!(parsed.sort_order, Some(5));

        // Empty object — all None
        let empty = serde_json::json!({});
        let parsed: UpdateSavedMedicine = serde_json::from_value(empty).unwrap();
        assert!(parsed.substance.is_none());
        assert!(parsed.dose.is_none());
        assert!(parsed.unit.is_none());
        assert!(parsed.route.is_none());
        assert!(parsed.sort_order.is_none());
    }
}
