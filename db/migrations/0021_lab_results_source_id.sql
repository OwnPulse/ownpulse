ALTER TABLE lab_results ADD COLUMN source_id TEXT;

CREATE UNIQUE INDEX idx_lab_results_dedup
    ON lab_results(user_id, source, marker, panel_date, source_id)
    WHERE source_id IS NOT NULL;
