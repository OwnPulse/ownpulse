-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Add user status column and invite codes table.

ALTER TABLE users ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
  CHECK (status IN ('active', 'disabled'));

CREATE TABLE invite_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code TEXT NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    label TEXT,
    max_uses INTEGER,
    use_count INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_invite_codes_code ON invite_codes(code) WHERE revoked_at IS NULL;
