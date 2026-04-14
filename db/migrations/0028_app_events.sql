-- Anonymous app telemetry: crash reports + flow analytics
-- No user_id column — intentionally anonymous

CREATE TABLE app_events (
    id BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    device_id TEXT,
    payload JSONB NOT NULL,
    app_version TEXT,
    platform TEXT DEFAULT 'ios',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_app_events_type_time ON app_events(event_type, created_at);
