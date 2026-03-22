-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Add user roles and friend sharing tables.

-- Add role column to users
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';

-- Friend shares: user A shares data with user B
CREATE TABLE friend_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id),
    friend_id UUID REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'pending',
    invite_token TEXT UNIQUE,
    invite_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    UNIQUE(owner_id, friend_id),
    CHECK (owner_id != friend_id)
);

CREATE INDEX idx_friend_shares_friend ON friend_shares(friend_id) WHERE status = 'accepted';
CREATE INDEX idx_friend_shares_owner ON friend_shares(owner_id) WHERE status != 'revoked';
CREATE INDEX idx_friend_shares_token ON friend_shares(invite_token) WHERE invite_token IS NOT NULL AND status = 'pending';

-- Per-data-type permissions within a share
CREATE TABLE friend_share_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_id UUID NOT NULL REFERENCES friend_shares(id) ON DELETE CASCADE,
    data_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(share_id, data_type)
);
