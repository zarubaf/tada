-- Telegram identity linking (ADR 0011).
-- user_id has no foreign key yet: the users table comes with sign-in in Slice 1 (ADR 0053).

-- A single-use code. The database stores only its SHA-256 hash (ADR 0008).
-- A Telegram account claims the code; the member then confirms the claim in the web client.
CREATE TABLE telegram_link_code (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    user_id uuid NOT NULL,
    code_hash bytea NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    claimed_by bigint,
    claimed_name text,
    claimed_at timestamptz,
    confirmed_at timestamptz,
    created_at timestamptz NOT NULL,
    CHECK ((claimed_by IS NULL) = (claimed_at IS NULL))
);

-- A Telegram account belongs to one user, and a user has one Telegram account.
CREATE TABLE telegram_identity (
    telegram_user_id bigint PRIMARY KEY,
    user_id uuid NOT NULL UNIQUE,
    linked_at timestamptz NOT NULL
);

-- The gateway handles each update once.
CREATE TABLE telegram_update (
    update_id bigint PRIMARY KEY,
    received_at timestamptz NOT NULL
);
