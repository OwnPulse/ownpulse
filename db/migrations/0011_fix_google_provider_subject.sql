-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Fix: migration 0008 set provider_subject = email for Google users, but the
-- real Google sub is not known at migration time. Set it to NULL so the code
-- falls through to the email-based lookup and backfills the real sub on first
-- login.
UPDATE user_auth_methods
SET provider_subject = NULL
WHERE provider = 'google' AND provider_subject IS NOT NULL
  AND provider_subject = email;
