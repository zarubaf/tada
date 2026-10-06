-- Organizations (the tenants) and their events (ADR 0049).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

CREATE TABLE organization (
    id uuid PRIMARY KEY,
    slug text NOT NULL UNIQUE,
    name text NOT NULL,
    created_at timestamptz NOT NULL
);

CREATE TABLE event (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    key text NOT NULL CHECK (key ~ '^[A-Z0-9]{2,8}$'),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    time_zone text NOT NULL,
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    CONSTRAINT event_key_unique UNIQUE (organization_id, key)
);
