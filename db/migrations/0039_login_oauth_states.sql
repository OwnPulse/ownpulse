-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Server-side state for the Google *login* redirect flow.
--
-- Separate from `oauth_states`, whose `user_id NOT NULL` contract is "proves
-- which authenticated user started a connect flow". Login starts
-- unauthenticated, so it needs its own table rather than a weakened column.
--
-- Everything the callback needs beyond the authorization code lives here
-- instead of in cookies: cookies are scoped per-site, not per-origin, so a
-- sibling subdomain could set `oauth_platform` / `invite_code` values the
-- callback would then trust. `state` is still echoed in a host-only cookie —
-- that binds the callback to the browser that started the flow, which a row
-- alone cannot do.
--
-- `link_user_id` NULL means login/register; non-NULL binds the flow to that
-- user at initiation, so the account a Google identity attaches to is decided
-- when the user clicks "Link Google", not by whatever cookie the browser
-- happens to present at callback time.
--
-- Rows are single-use: the callback deletes on read. Abandoned rows are swept
-- by the hourly expiry sweep job.
--
-- No row-level security here, unlike `oauth_states`. A policy keyed on
-- `app.current_user_id` could not admit this table's inserts anyway: login
-- starts unauthenticated, so there is no current user to key on. The table
-- holds no user data — a nonce, a platform flag, an invite code and an
-- optional user id — and every read is a primary-key lookup of a value the
-- caller already had to present.
--
-- Note for anyone hardening the database role: nothing in the API sets
-- `app.current_user_id`, so the policies on `oauth_states` and the 0007
-- tables are not in force for the app connection today. Putting them in
-- force would silently make both expiry sweeps delete nothing, since their
-- predicates would match no rows under those policies.
CREATE TABLE login_oauth_states (
    state UUID PRIMARY KEY,
    is_native BOOLEAN NOT NULL DEFAULT false,
    invite_code TEXT CHECK (invite_code IS NULL OR length(invite_code) <= 64),
    link_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The sweep deletes by age; without this it seq-scans the table every hour.
CREATE INDEX idx_login_oauth_states_created_at ON login_oauth_states (created_at);
