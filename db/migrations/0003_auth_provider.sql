-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Add Google OAuth support: make password_hash nullable and track auth provider.

ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN auth_provider TEXT NOT NULL DEFAULT 'local';
ALTER TABLE users ADD COLUMN email TEXT;
