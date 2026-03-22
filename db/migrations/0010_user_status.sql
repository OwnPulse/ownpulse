-- Add status column to users table for account lifecycle management.
-- Supports active/disabled states checked by the auth extractor.

ALTER TABLE users ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
