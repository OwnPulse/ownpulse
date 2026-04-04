-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Protocol runs: an execution of a protocol recipe.
-- Protocols are reusable recipes; runs are executions with start_date and status.
CREATE TABLE protocol_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id     UUID NOT NULL REFERENCES protocols(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_date      DATE NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed', 'archived')),
    notify          BOOLEAN NOT NULL DEFAULT false,
    notify_time     TEXT,
    notify_times    JSONB DEFAULT '[]',
    repeat_reminders BOOLEAN NOT NULL DEFAULT false,
    repeat_interval_minutes INT DEFAULT 30,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_protocol_runs_user_active ON protocol_runs(user_id) WHERE status = 'active';
CREATE INDEX idx_protocol_runs_protocol ON protocol_runs(protocol_id);

-- Add run_id to protocol_doses so doses can be tied to a specific run.
ALTER TABLE protocol_doses ADD COLUMN run_id UUID REFERENCES protocol_runs(id) ON DELETE CASCADE;

-- Backfill: create a run for each existing active protocol that has a start_date,
-- then link existing doses to that run.
DO $$
DECLARE
    rec RECORD;
    new_run_id UUID;
BEGIN
    FOR rec IN
        SELECT p.id AS protocol_id, p.user_id, p.start_date, p.status
        FROM protocols p
        WHERE p.user_id IS NOT NULL
          AND p.start_date IS NOT NULL
          AND p.status IN ('active', 'paused')
    LOOP
        INSERT INTO protocol_runs (protocol_id, user_id, start_date, status)
        VALUES (rec.protocol_id, rec.user_id, rec.start_date, rec.status)
        RETURNING id INTO new_run_id;

        -- Link existing doses for this protocol's lines to the new run
        UPDATE protocol_doses pd
        SET run_id = new_run_id
        FROM protocol_lines pl
        WHERE pd.protocol_line_id = pl.id
          AND pl.protocol_id = rec.protocol_id
          AND pd.run_id IS NULL;
    END LOOP;
END $$;

-- Make start_date nullable on protocols (it belongs to runs now).
ALTER TABLE protocols ALTER COLUMN start_date DROP NOT NULL;

-- Update status constraint to include 'draft', and change default to 'draft'.
ALTER TABLE protocols DROP CONSTRAINT IF EXISTS protocols_status_check;
ALTER TABLE protocols ADD CONSTRAINT protocols_status_check
    CHECK (status IN ('active', 'paused', 'completed', 'archived', 'draft'));
ALTER TABLE protocols ALTER COLUMN status SET DEFAULT 'draft';

-- User notification preferences
CREATE TABLE user_notification_preferences (
    user_id                 UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    default_notify          BOOLEAN NOT NULL DEFAULT false,
    default_notify_times    JSONB NOT NULL DEFAULT '["08:00"]',
    repeat_reminders        BOOLEAN NOT NULL DEFAULT false,
    repeat_interval_minutes INT NOT NULL DEFAULT 30,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Push notification tokens
CREATE TABLE push_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_token    TEXT NOT NULL,
    platform        TEXT NOT NULL CHECK (platform IN ('ios', 'web')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, device_token)
);
