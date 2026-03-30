-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

CREATE TABLE protocols (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,
    start_date      DATE NOT NULL,
    duration_days   INT NOT NULL CHECK (duration_days > 0 AND duration_days <= 365),
    status          TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed', 'archived')),
    share_token     TEXT UNIQUE,
    share_expires_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_protocols_user ON protocols (user_id);
CREATE INDEX idx_protocols_share_token ON protocols (share_token) WHERE share_token IS NOT NULL;

CREATE TABLE protocol_lines (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id     UUID NOT NULL REFERENCES protocols(id) ON DELETE CASCADE,
    substance       TEXT NOT NULL,
    dose            DOUBLE PRECISION,
    unit            TEXT,
    route           TEXT,
    time_of_day     TEXT,
    schedule_pattern JSONB NOT NULL,
    sort_order      INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_protocol_lines_protocol ON protocol_lines (protocol_id);

CREATE TABLE protocol_doses (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_line_id  UUID NOT NULL REFERENCES protocol_lines(id) ON DELETE CASCADE,
    day_number        INT NOT NULL CHECK (day_number >= 0),
    status            TEXT NOT NULL CHECK (status IN ('completed', 'skipped')),
    intervention_id   UUID REFERENCES interventions(id) ON DELETE SET NULL,
    logged_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(protocol_line_id, day_number)
);

CREATE INDEX idx_protocol_doses_line ON protocol_doses (protocol_line_id);
