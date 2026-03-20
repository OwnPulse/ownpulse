-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Sleep records — nightly sleep sessions from HealthKit, wearables, or manual entry.

CREATE TABLE sleep_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    date DATE NOT NULL,
    sleep_start TIMESTAMPTZ,
    sleep_end TIMESTAMPTZ,
    duration_minutes INTEGER NOT NULL,
    deep_minutes INTEGER,
    light_minutes INTEGER,
    rem_minutes INTEGER,
    awake_minutes INTEGER,
    score INTEGER CHECK (score BETWEEN 0 AND 100),
    source TEXT NOT NULL DEFAULT 'manual',
    source_id TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, date, source)
);

CREATE INDEX idx_sleep_records_user_date
    ON sleep_records(user_id, date);
