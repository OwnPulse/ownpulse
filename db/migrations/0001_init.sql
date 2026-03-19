-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Initial schema for OwnPulse. This is the source of truth for all tables,
-- indexes, and constraints. Never edit this migration — add new ones.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ============================================================================
-- Users
-- ============================================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    data_region TEXT NOT NULL DEFAULT 'us',
    federation_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- Health records — all wearable/device measurements
-- ============================================================================

CREATE TABLE health_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    source TEXT NOT NULL,
    record_type TEXT NOT NULL,
    value DOUBLE PRECISION,
    unit TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    metadata JSONB,
    source_id TEXT,
    source_instance TEXT,
    duplicate_of UUID REFERENCES health_records(id),
    healthkit_written BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, source, record_type, start_time, source_id)
);

CREATE INDEX idx_health_records_user_source_type_time
    ON health_records(user_id, source, record_type, start_time);

-- ============================================================================
-- Interventions — substances, meds, supplements (no name validation)
-- ============================================================================

CREATE TABLE interventions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    substance TEXT NOT NULL,
    dose DOUBLE PRECISION,
    unit TEXT,
    route TEXT,
    administered_at TIMESTAMPTZ NOT NULL,
    fasted BOOLEAN,
    timing_relative_to TEXT,
    notes TEXT,
    healthkit_written BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_interventions_user_time
    ON interventions(user_id, administered_at);

-- ============================================================================
-- Daily check-ins — five 1-10 subjective scores
-- ============================================================================

CREATE TABLE daily_checkins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    date DATE NOT NULL,
    energy INTEGER CHECK (energy BETWEEN 1 AND 10),
    mood INTEGER CHECK (mood BETWEEN 1 AND 10),
    focus INTEGER CHECK (focus BETWEEN 1 AND 10),
    recovery INTEGER CHECK (recovery BETWEEN 1 AND 10),
    libido INTEGER CHECK (libido BETWEEN 1 AND 10),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, date)
);

CREATE INDEX idx_checkins_user_date
    ON daily_checkins(user_id, date);

-- ============================================================================
-- Lab results — blood panel data with reference ranges
-- ============================================================================

CREATE TABLE lab_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    panel_date DATE NOT NULL,
    lab_name TEXT,
    marker TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    unit TEXT NOT NULL,
    reference_low DOUBLE PRECISION,
    reference_high DOUBLE PRECISION,
    out_of_range BOOLEAN GENERATED ALWAYS AS (
        value < reference_low OR value > reference_high
    ) STORED,
    source TEXT NOT NULL DEFAULT 'manual',
    uploaded_file_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_lab_results_user_date
    ON lab_results(user_id, panel_date);

-- ============================================================================
-- Calendar days — meeting aggregates from Google Calendar
-- ============================================================================

CREATE TABLE calendar_days (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    date DATE NOT NULL,
    meeting_minutes INTEGER NOT NULL DEFAULT 0,
    meeting_count INTEGER NOT NULL DEFAULT 0,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, date)
);

-- ============================================================================
-- Observations — flexible user-defined data (events, scales, symptoms, etc.)
-- ============================================================================

CREATE TABLE observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    type TEXT NOT NULL,
    name TEXT NOT NULL,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    value JSONB,
    source TEXT NOT NULL DEFAULT 'manual',
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_observations_user_type_time
    ON observations(user_id, type, start_time);
CREATE INDEX idx_observations_name_gin
    ON observations USING gin(to_tsvector('english', name));

-- ============================================================================
-- Source preferences — per-metric source-of-truth preference
-- ============================================================================

CREATE TABLE source_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    metric_type TEXT NOT NULL,
    preferred_source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, metric_type)
);

-- ============================================================================
-- HealthKit write-back queue — pending writes to HealthKit via iOS
-- ============================================================================

CREATE TABLE healthkit_write_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    hk_type TEXT NOT NULL,
    value JSONB NOT NULL,
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    confirmed_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    error TEXT,
    source_record_id UUID,
    source_table TEXT
);

CREATE INDEX idx_hk_write_queue_pending
    ON healthkit_write_queue(user_id, scheduled_at)
    WHERE confirmed_at IS NULL AND failed_at IS NULL;

-- ============================================================================
-- Uploaded files — lab PDFs, genetic data files
-- ============================================================================

CREATE TABLE uploaded_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    file_type TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    file_size_bytes BIGINT,
    processed BOOLEAN NOT NULL DEFAULT false,
    processed_at TIMESTAMPTZ,
    processing_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Add FK now that uploaded_files exists
ALTER TABLE lab_results
    ADD CONSTRAINT fk_lab_results_uploaded_file
    FOREIGN KEY (uploaded_file_id) REFERENCES uploaded_files(id);

-- ============================================================================
-- Genetic records — SNP variants, stored verbatim
-- ============================================================================

CREATE TABLE genetic_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    source TEXT NOT NULL,
    rsid TEXT,
    chromosome TEXT,
    position BIGINT,
    genotype TEXT,
    uploaded_file_id UUID REFERENCES uploaded_files(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, rsid)
);

-- ============================================================================
-- Integration tokens — OAuth tokens for all integrations (encrypted)
-- ============================================================================

CREATE TABLE integration_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    source TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at TIMESTAMPTZ,
    last_synced_at TIMESTAMPTZ,
    last_sync_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, source)
);

-- ============================================================================
-- Refresh tokens — JWT refresh tokens
-- ============================================================================

CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- Sharing consents — cooperative data sharing consent
-- ============================================================================

CREATE TABLE sharing_consents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    dataset TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'cooperative_aggregate',
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    UNIQUE(user_id, dataset, scope)
);

CREATE INDEX idx_sharing_consents_active
    ON sharing_consents(user_id) WHERE revoked_at IS NULL;

-- ============================================================================
-- Export jobs — export audit log
-- ============================================================================

CREATE TABLE export_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    format TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    record_count INTEGER
);
