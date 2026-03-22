-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Add family_id to refresh_tokens for replay detection.
-- Tokens in the same family share a family_id; if a rotated-out token is
-- presented, all tokens in that family are revoked.

ALTER TABLE refresh_tokens ADD COLUMN family_id UUID NOT NULL DEFAULT gen_random_uuid();
CREATE INDEX idx_refresh_tokens_family ON refresh_tokens (family_id);
