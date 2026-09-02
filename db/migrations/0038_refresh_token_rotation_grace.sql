-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Refresh rotation deleted the presented token, so concurrent refreshes
-- from web tabs sharing the one httpOnly cookie raced: the loser's token
-- was already gone, producing a spurious 401 (and logout) plus a false
-- replay warning. Rotation now marks the token instead, so the handler
-- can honor a short grace window after rotation and treat only post-grace
-- reuse as theft (revoking the whole family). NULL = active token.
--
-- successor_ciphertext holds the successor token AES-256-GCM-encrypted so a
-- within-grace presentation returns the SAME successor instead of minting a
-- fork. One shared successor keeps a thief and the legitimate client on one
-- chain, so they keep colliding and post-grace reuse detection still fires;
-- independent forks would let a thief hold a session undetectably forever.
-- The ciphertext lives only until the rotated row is swept (grace + next
-- rotation).
ALTER TABLE refresh_tokens ADD COLUMN rotated_at TIMESTAMPTZ;
ALTER TABLE refresh_tokens ADD COLUMN successor_ciphertext TEXT;
