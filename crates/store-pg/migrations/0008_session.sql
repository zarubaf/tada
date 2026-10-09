-- Sessions (ADR 0008) and the organization of a session (ADR 0056).
-- The table holds the SHA-256 hash of the token only, never the token.
-- The organization is empty until the member chooses one.

CREATE TABLE session (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    user_id uuid NOT NULL REFERENCES app_user (id) ON DELETE CASCADE,
    organization_id uuid REFERENCES organization (id),
    created_at timestamptz NOT NULL,
    last_used_at timestamptz NOT NULL,
    user_agent text,
    CHECK (last_used_at >= created_at)
);

-- Revocation deletes all sessions of a user (ADR 0008).
CREATE INDEX session_user_id ON session (user_id);
