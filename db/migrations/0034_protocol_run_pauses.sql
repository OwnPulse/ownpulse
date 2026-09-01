-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Pause intervals for a protocol run. A run can be paused and resumed
-- multiple times; each row is one [paused_on, resumed_on) interval
-- (resumed_on NULL = still paused, an open interval). Days inside any
-- interval are excluded from adherence scheduling entirely (not scheduled,
-- not missed, not counted in the denominator) across every adherence
-- computation — see dose_status::is_paused and its callers. Pausing stops
-- the adherence clock; it does not keep accruing missed days against a
-- schedule the user explicitly put on hold.
CREATE TABLE run_pauses (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id          UUID NOT NULL REFERENCES protocol_runs(id) ON DELETE CASCADE,
    paused_on       DATE NOT NULL,
    resumed_on      DATE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_run_pauses_run ON run_pauses (run_id);
