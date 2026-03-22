-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Switch login from username to email. Email becomes the primary identifier.

-- Backfill email from username for any users that don't have one
UPDATE users SET email = username WHERE email IS NULL;

-- Email is now required
ALTER TABLE users ALTER COLUMN email SET NOT NULL;

-- Username becomes optional display name
ALTER TABLE users ALTER COLUMN username DROP NOT NULL;

-- Replace username unique with email unique
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_username_key;
DROP INDEX IF EXISTS users_email_auth_provider_idx;
CREATE UNIQUE INDEX users_email_unique ON users (email);
