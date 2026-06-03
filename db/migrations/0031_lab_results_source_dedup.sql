-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Strengthen lab-result deduplication for externally-sourced rows and preserve
-- the LOINC code from FHIR imports.
--
-- The original dedup index (0021) keyed on
-- (user_id, source, marker, panel_date, source_id). For FHIR sources the
-- stable identity is the resource id alone: the display text (`marker`) and
-- even the value can be amended by the lab without the resource id changing.
-- Keying on `marker` therefore let a re-sync insert a DUPLICATE clinical row
-- whenever the provider tweaked the display string. The correct identity for a
-- source-id-bearing row is (user_id, source, source_id).
--
-- Add a unique index on that triple so ON CONFLICT can upsert the mutable
-- attributes (marker, value, unit, reference range) in place.
CREATE UNIQUE INDEX idx_lab_results_source_dedup
    ON lab_results (user_id, source, source_id)
    WHERE source_id IS NOT NULL;

-- Preserve the LOINC code from FHIR Observation codings. Open-schema /
-- no-lock-in: capture it at import time; backfilling later is unreliable.
ALTER TABLE lab_results ADD COLUMN loinc_code TEXT;
