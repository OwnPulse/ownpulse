-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Interventions synced from Apple Health (medication dose events) had no
-- server-side dedup: the client's device-local guard cannot survive an app
-- reinstall or cover a second device, so replayed dose events inserted
-- fresh rows every time. Add `source`, a nullable `source_id`, and a
-- partial unique index so synced rows with a deterministic id (the
-- HealthKit dose-event UUID) insert with `ON CONFLICT ... DO NOTHING`. The index is
-- partial (`WHERE source_id IS NOT NULL`) so manual entries — which never
-- set `source_id` — are never constrained; they may legitimately repeat.
-- Rows created before this migration have NULL `source_id` and are not
-- deduped retroactively.
-- `source` is NOT NULL DEFAULT 'manual' to match lab_results, observations,
-- and health_records: index NULLs are distinct in Postgres, so a NULL
-- source would silently bypass the unique index below.
ALTER TABLE interventions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE interventions ADD COLUMN source_id TEXT;

CREATE UNIQUE INDEX idx_interventions_user_source_source_id
    ON interventions (user_id, source, source_id)
    WHERE source_id IS NOT NULL;
