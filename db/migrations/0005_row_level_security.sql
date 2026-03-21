-- Row-Level Security: defense-in-depth user isolation.
-- Enforced when connecting as non-superuser (ownpulse_app role).
-- The superuser (postgres) bypasses RLS for migrations and admin tasks.

ALTER TABLE health_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE interventions ENABLE ROW LEVEL SECURITY;
ALTER TABLE daily_checkins ENABLE ROW LEVEL SECURITY;
ALTER TABLE lab_results ENABLE ROW LEVEL SECURITY;
ALTER TABLE observations ENABLE ROW LEVEL SECURITY;
ALTER TABLE genetic_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE integration_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE source_preferences ENABLE ROW LEVEL SECURITY;
ALTER TABLE sharing_consents ENABLE ROW LEVEL SECURITY;
ALTER TABLE refresh_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE healthkit_write_queue ENABLE ROW LEVEL SECURITY;
ALTER TABLE uploaded_files ENABLE ROW LEVEL SECURITY;
ALTER TABLE export_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE calendar_days ENABLE ROW LEVEL SECURITY;

-- Policies: each user can only see/modify their own rows.
-- current_setting with true = return empty string if not set (avoids errors).
CREATE POLICY user_isolation ON health_records FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON interventions FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON daily_checkins FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON lab_results FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON observations FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON genetic_records FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON integration_tokens FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON source_preferences FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON sharing_consents FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON refresh_tokens FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON healthkit_write_queue FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON uploaded_files FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON export_jobs FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
CREATE POLICY user_isolation ON calendar_days FOR ALL
    USING (user_id = current_setting('app.current_user_id', true)::uuid);
