-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

ALTER TABLE protocols ADD COLUMN is_template BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE protocols ADD COLUMN tags JSONB DEFAULT '[]';
ALTER TABLE protocols ADD COLUMN source_url TEXT;

-- Templates have user_id = NULL (no owner).
ALTER TABLE protocols ALTER COLUMN user_id DROP NOT NULL;

CREATE INDEX idx_protocols_template ON protocols (is_template) WHERE is_template = true;
