ALTER TABLE music_connections
    ADD COLUMN IF NOT EXISTS provider_account_name TEXT;
