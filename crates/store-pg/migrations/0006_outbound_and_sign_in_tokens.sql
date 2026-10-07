-- Invitations, outbound intents and the tokens of magic links and invitations (ADRs 0008, 0042 and 0056).
-- No table holds a token in plain text: the token tables hold the SHA-256 hash only.

CREATE TABLE invitation (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    email text NOT NULL,
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 100),
    role text NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    -- Empty for an invitation that `tada bootstrap` created (ADR 0036).
    invited_by uuid REFERENCES app_user (id),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'revoked')),
    created_at timestamptz NOT NULL,
    accepted_at timestamptz,
    revoked_at timestamptz,
    UNIQUE (organization_id, id)
);

-- A message that tada will send (ADR 0042). The command that causes it writes it in its own transaction,
-- together with the send job. The intent holds no token; the worker creates the token when it sends.
-- A magic-link intent belongs to a user, not to an organization, so `organization_id` is empty for it.
CREATE TABLE outbound_intent (
    id uuid PRIMARY KEY,
    organization_id uuid,
    user_id uuid REFERENCES app_user (id),
    invitation_id uuid,
    purpose text NOT NULL CHECK (purpose IN ('magic-link', 'invitation')),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'failed', 'unknown')),
    -- The left part of the `Message-ID` header. The worker adds the host of `TADA_PUBLIC_URL`.
    message_id text NOT NULL UNIQUE,
    request_id uuid,
    created_at timestamptz NOT NULL,
    finished_at timestamptz,
    CHECK ((purpose = 'invitation') = (organization_id IS NOT NULL AND invitation_id IS NOT NULL)),
    CHECK ((purpose = 'magic-link') = (user_id IS NOT NULL)),
    FOREIGN KEY (organization_id, invitation_id) REFERENCES invitation (organization_id, id)
);

-- An invitation can have one mailed token and one printed token.
-- Acceptance deletes all tokens of the invitation.
CREATE TABLE invitation_token (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    organization_id uuid NOT NULL,
    invitation_id uuid NOT NULL,
    expires_at timestamptz NOT NULL,
    FOREIGN KEY (organization_id, invitation_id) REFERENCES invitation (organization_id, id)
);

CREATE TABLE magic_link (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    user_id uuid NOT NULL REFERENCES app_user (id),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL
);
