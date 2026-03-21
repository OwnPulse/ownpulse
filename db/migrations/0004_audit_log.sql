-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Audit log for sensitive data operations (exports, deletes, account actions).
-- Never edit this migration — add new ones.

CREATE TABLE data_access_log (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID,
    ip_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_data_access_log_user_time ON data_access_log(user_id, created_at);
