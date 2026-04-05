-- Allow multiple check-ins per day by dropping the unique constraint.
-- The old UNIQUE(user_id, date) prevented more than one check-in per day.
ALTER TABLE daily_checkins DROP CONSTRAINT daily_checkins_user_id_date_key;

-- Keep an index for query performance
CREATE INDEX idx_daily_checkins_user_date ON daily_checkins (user_id, date);
