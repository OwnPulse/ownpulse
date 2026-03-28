-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

CREATE TABLE observer_polls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    custom_prompt TEXT,
    dimensions JSONB NOT NULL DEFAULT '["energy","mood","focus","recovery","appearance"]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_observer_polls_user
    ON observer_polls(user_id) WHERE deleted_at IS NULL;

CREATE TABLE observer_poll_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id UUID NOT NULL REFERENCES observer_polls(id) ON DELETE CASCADE,
    observer_id UUID REFERENCES users(id) ON DELETE SET NULL,
    invite_token UUID NOT NULL DEFAULT gen_random_uuid(),
    invite_expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + INTERVAL '7 days',
    accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(poll_id, observer_id)
);

CREATE INDEX idx_observer_poll_members_token
    ON observer_poll_members(invite_token)
    WHERE accepted_at IS NULL;

CREATE INDEX idx_observer_poll_members_observer
    ON observer_poll_members(observer_id)
    WHERE accepted_at IS NOT NULL;

CREATE TABLE observer_responses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id UUID NOT NULL REFERENCES observer_polls(id) ON DELETE CASCADE,
    member_id UUID NOT NULL REFERENCES observer_poll_members(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    scores JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(member_id, date)
);

CREATE INDEX idx_observer_responses_poll_date
    ON observer_responses(poll_id, date);
