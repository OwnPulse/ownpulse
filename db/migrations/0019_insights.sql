-- Insight cards — automated pattern detection surfaced on the dashboard.
-- Each row is a generated insight for a user (trends, anomalies, streaks, etc.).

CREATE TABLE insights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    insight_type TEXT NOT NULL,
    headline TEXT NOT NULL,
    detail TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    dismissed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_insights_user_active
    ON insights(user_id, created_at DESC)
    WHERE dismissed_at IS NULL;
