-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Add 'declined' status to friend_shares and update indexes.

-- Add CHECK constraint for valid statuses
ALTER TABLE friend_shares ADD CONSTRAINT friend_shares_status_check
  CHECK (status IN ('pending', 'accepted', 'revoked', 'declined'));

-- Recreate partial index to exclude both terminal statuses
DROP INDEX idx_friend_shares_owner;
CREATE INDEX idx_friend_shares_owner ON friend_shares(owner_id)
  WHERE status NOT IN ('revoked', 'declined');
