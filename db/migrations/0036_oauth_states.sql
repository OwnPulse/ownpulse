-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Server-side state for browser-redirect OAuth connect flows (Google
-- Calendar today; Garmin/Oura if they grow the same browser-navigation
-- entry point later). `state` is the CSRF token handed to the provider and
-- echoed back on the callback redirect — looking it up here both validates
-- CSRF (a guessed/replayed value won't be a row) and recovers which user
-- started the flow, without needing a cookie to carry identity across the
-- redirect. Rows are single-use: the callback deletes on read.
CREATE TABLE oauth_states (
    state UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE oauth_states ENABLE ROW LEVEL SECURITY;
CREATE POLICY user_isolation ON oauth_states FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
