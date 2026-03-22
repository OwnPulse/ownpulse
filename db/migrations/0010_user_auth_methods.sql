-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

CREATE TABLE user_auth_methods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_subject TEXT,
    email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider, provider_subject),
    UNIQUE(provider, email)
);

CREATE INDEX idx_user_auth_methods_user_id ON user_auth_methods(user_id);

-- Migrate existing data
INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
SELECT id, auth_provider, CASE WHEN auth_provider = 'local' THEN id::TEXT ELSE email END, email
FROM users;
