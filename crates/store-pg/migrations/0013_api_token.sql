-- Personal API tokens (ADR 0039) and the switches of each organization (ADR 0036, ADR 0045).

-- A token of one member in one organization. The table holds the SHA-256 hash of the token only, never the token.
-- `notice_version` is the version of the token notice that the member confirmed at `notice_confirmed_at` (ADR 0045).
-- Removing the membership removes its tokens. The authenticator also reads the membership on each request.
CREATE TABLE api_token (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    user_id uuid NOT NULL,
    token_hash bytea NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    scope text NOT NULL CHECK (scope IN ('read', 'propose')),
    expires_at timestamptz NOT NULL,
    notice_version integer NOT NULL CHECK (notice_version >= 1),
    notice_confirmed_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL,
    last_used_at timestamptz,
    revoked_at timestamptz,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, user_id)
        REFERENCES organization_membership (organization_id, user_id) ON DELETE CASCADE,
    CHECK (expires_at > created_at)
);

-- A member lists the own tokens of one organization.
CREATE INDEX api_token_member ON api_token (organization_id, user_id, created_at);

-- The creation of a `propose` token reads the event roles of one member in all events of the organization (ADR 0052).
CREATE INDEX event_membership_member ON event_membership (organization_id, user_id);

-- A switch of one organization, for example `mcp-tokens` (ADR 0036). An owner changes it.
-- A feature without a row has its default value and version 1.
CREATE TABLE organization_feature (
    organization_id uuid NOT NULL REFERENCES organization (id),
    feature text NOT NULL CHECK (feature IN ('mcp-tokens')),
    enabled boolean NOT NULL,
    version bigint NOT NULL CHECK (version >= 2),
    PRIMARY KEY (organization_id, feature)
);
