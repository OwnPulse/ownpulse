-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Copyright (C) OwnPulse Contributors
--
-- Waitlist signups from the public site.

CREATE TABLE waitlist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    name TEXT,
    persona TEXT CHECK (persona IN (
        'quantified_self',
        'biohacker',
        'peptide_pioneer',
        'iron_scientist',
        'health_detective',
        'builder',
        'clinician',
        'basics'
    )),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
