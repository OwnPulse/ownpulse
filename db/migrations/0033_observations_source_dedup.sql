-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Observations had no way to dedupe re-synced wearable data (e.g. a Garmin/Oura
-- sleep observation re-fetched on every sync cycle inserted a fresh row every
-- time, since `observations` carries no analogue of `health_records.source_id`).
-- Add a nullable `source_id` and a partial unique index so job-authored rows
-- with a deterministic id (e.g. `garmin-sleep-2026-03-28`) can be inserted with
-- `ON CONFLICT ... DO NOTHING`. The index is partial (`WHERE source_id IS NOT
-- NULL`) so manual entries — which never set `source_id` — are never
-- constrained; they may legitimately repeat.
ALTER TABLE observations ADD COLUMN source_id TEXT;

CREATE UNIQUE INDEX idx_observations_user_source_source_id
    ON observations (user_id, source, source_id)
    WHERE source_id IS NOT NULL;
