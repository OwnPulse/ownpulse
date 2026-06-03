-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors

-- Add a nullable metadata column to integration_tokens.
--
-- Some integrations need to persist non-secret connection parameters alongside
-- the OAuth tokens. The first consumer is MyChart (SMART-on-FHIR): the FHIR
-- server base URL and token endpoint are discovered per-provider at connect
-- time and must be available later when the background sync job runs.
--
-- This is non-secret connection metadata only. OAuth tokens stay in the
-- encrypted access_token / refresh_token columns — never put secrets here.
ALTER TABLE integration_tokens ADD COLUMN metadata JSONB;
