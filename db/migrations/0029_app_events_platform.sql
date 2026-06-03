-- Widen app_events.platform to formally accept 'web' alongside 'ios'.
--
-- The column was created as `TEXT DEFAULT 'ios'` with no constraint, so it
-- already accepted any string at the DB level. This migration adds an explicit,
-- additive CHECK constraint so the column is restricted to the known platform
-- set ('ios', 'web') going forward. This is non-destructive: existing rows are
-- all 'ios' (or NULL via the default), both of which satisfy the constraint.

ALTER TABLE app_events
    ADD CONSTRAINT app_events_platform_check
    CHECK (platform IN ('ios', 'web'));
