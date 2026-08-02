-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Foundation for intended-vs-actual dose tracking.
--
-- The original UNIQUE(protocol_line_id, day_number) constraint (from
-- 0022_protocols.sql) made a SECOND run of the same protocol collide with
-- the first run's dose log on the same day_number. Replace it with a
-- constraint scoped to the run. Postgres 17's NULLS NOT DISTINCT keeps the
-- legacy protocol-level dose rows (run_id IS NULL, from the pre-runs
-- `log_dose`/`skip_dose` endpoints) unique among themselves too.
ALTER TABLE protocol_doses DROP CONSTRAINT protocol_doses_protocol_line_id_day_number_key;

ALTER TABLE protocol_doses ADD CONSTRAINT protocol_doses_line_run_day_key
    UNIQUE NULLS NOT DISTINCT (protocol_line_id, run_id, day_number);

-- Optional reason recorded when a dose is deliberately skipped.
ALTER TABLE protocol_doses ADD COLUMN skip_reason TEXT;

CREATE INDEX idx_protocol_doses_run ON protocol_doses (run_id);

-- Needed by the new PATCH /interventions/:id endpoint.
ALTER TABLE interventions ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
