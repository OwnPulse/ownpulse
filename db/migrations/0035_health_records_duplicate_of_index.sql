-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Support the reverse direction of the source_preferences dedup-partner walk
-- (`other.duplicate_of = hr.id`, used by SOURCE_PREFERENCE_EXCLUSION) with an
-- index. `duplicate_of` already has an implicit FK lookup path forward (by
-- primary key), but the reverse lookup — "which row points at me?" — was a
-- full scan without this.
CREATE INDEX idx_health_records_duplicate_of
    ON health_records (duplicate_of)
    WHERE duplicate_of IS NOT NULL;
