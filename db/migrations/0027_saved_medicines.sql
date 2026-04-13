CREATE TABLE saved_medicines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    substance TEXT NOT NULL,
    dose DOUBLE PRECISION,
    unit TEXT,
    route TEXT,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_saved_medicines_user
    ON saved_medicines(user_id, sort_order);
