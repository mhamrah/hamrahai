CREATE TABLE IF NOT EXISTS user_auth_providers (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider <> ''),
    provider_id TEXT NOT NULL CHECK (provider_id <> ''),
    email TEXT NOT NULL,
    name TEXT,
    picture TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider, provider_id),
    UNIQUE (user_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_user_auth_providers_user_id
    ON user_auth_providers(user_id);

INSERT INTO user_auth_providers (
    user_id, provider, provider_id, email, name, picture, last_used_at
)
SELECT id, provider, provider_id, email, name, picture, COALESCE(last_login_at, NOW())
FROM users
WHERE provider IS NOT NULL
  AND provider <> ''
  AND provider_id IS NOT NULL
  AND provider_id <> ''
ON CONFLICT DO NOTHING;
