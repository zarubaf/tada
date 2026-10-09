-- Users, memberships and the audit log (ADR 0008, ADR 0039, ADR 0052, ADR 0056).
-- The table is not named "user", because that is a reserved word.

CREATE TABLE app_user (
    id uuid PRIMARY KEY,
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 100),
    locale text NOT NULL DEFAULT 'de-CH',
    created_at timestamptz NOT NULL
);

-- One email address belongs to at most one user (ADR 0056).
-- `Email` normalizes the address before it reaches this table.
CREATE TABLE email_identity (
    user_id uuid PRIMARY KEY REFERENCES app_user (id),
    email text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL
);

CREATE TABLE organization_membership (
    organization_id uuid NOT NULL REFERENCES organization (id),
    user_id uuid NOT NULL REFERENCES app_user (id),
    role text NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    version bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    PRIMARY KEY (organization_id, user_id)
);

-- The event role of a member (ADR 0052).
-- Removing the organization membership removes the event memberships.
CREATE TABLE event_membership (
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    user_id uuid NOT NULL,
    event_role text NOT NULL CHECK (event_role IN ('event-manager', 'event-contributor', 'event-viewer')),
    version bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    PRIMARY KEY (event_id, user_id),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, user_id)
        REFERENCES organization_membership (organization_id, user_id) ON DELETE CASCADE
);

-- Who did what (ADR 0039). The log holds pseudonymous IDs only: no personal data and no free text.
CREATE TABLE audit_event (
    id uuid PRIMARY KEY,
    organization_id uuid REFERENCES organization (id),
    occurred_at timestamptz NOT NULL,
    actor_kind text NOT NULL,
    actor_id uuid NOT NULL,
    principal_id uuid,
    channel text NOT NULL,
    request_id uuid,
    action text NOT NULL,
    record_kind text NOT NULL,
    record_id uuid
);

-- Only the debug development authenticator created Telegram rows without a user (ADR 0053).
DELETE FROM telegram_link_code WHERE user_id NOT IN (SELECT id FROM app_user);
DELETE FROM telegram_identity WHERE user_id NOT IN (SELECT id FROM app_user);
ALTER TABLE telegram_link_code ADD FOREIGN KEY (user_id) REFERENCES app_user (id);
ALTER TABLE telegram_identity ADD FOREIGN KEY (user_id) REFERENCES app_user (id);
