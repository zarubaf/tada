-- Schedules, the AI-PM reminders and the use of the model API (Slice 2c).
-- The migration only adds: the previous image still starts on the new schema (ADR 0006).

-- One recurring job of the worker. An organization has its own schedule; an empty organization is a schedule of
-- the installation. The worker runs a schedule when `next_run_at` has passed, and skips runs that are older
-- than `missed_run_limit` periods.
CREATE TABLE schedule (
    id uuid PRIMARY KEY,
    organization_id uuid REFERENCES organization (id),
    kind text NOT NULL,
    local_time time NOT NULL,
    time_zone text NOT NULL,
    -- Bit 0 is Monday, bit 6 is Sunday.
    weekdays smallint NOT NULL CHECK (weekdays BETWEEN 1 AND 127),
    next_run_at timestamptz NOT NULL,
    last_run_at timestamptz,
    missed_run_limit smallint NOT NULL CHECK (missed_run_limit >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE NULLS NOT DISTINCT (organization_id, kind)
);
CREATE INDEX schedule_next_run_at ON schedule (next_run_at);

-- The reminder policy of an organization. An organization without a row uses the defaults; the first set inserts.
CREATE TABLE pm_policy (
    organization_id uuid PRIMARY KEY REFERENCES organization (id),
    local_time time NOT NULL,
    time_zone text NOT NULL,
    weekdays smallint NOT NULL CHECK (weekdays BETWEEN 1 AND 127),
    quiet_start time NOT NULL,
    quiet_end time NOT NULL,
    cooldown_hours smallint NOT NULL CHECK (cooldown_hours BETWEEN 1 AND 168),
    follow_up_working_days smallint NOT NULL CHECK (follow_up_working_days BETWEEN 1 AND 10),
    look_ahead_working_days smallint NOT NULL CHECK (look_ahead_working_days BETWEEN 0 AND 10),
    paused boolean NOT NULL DEFAULT false,
    ai_enabled boolean NOT NULL DEFAULT false,
    version bigint NOT NULL DEFAULT 1
);

ALTER TABLE app_user
    ADD COLUMN time_zone text NOT NULL DEFAULT 'Europe/Zurich',
    ADD COLUMN reminders_opt_in_at timestamptz;

-- Set when the Bot API refuses a message to the user, for example after the user blocked the bot.
ALTER TABLE telegram_identity ADD COLUMN blocked_at timestamptz;

-- A Telegram digest is an outbound intent of an organization and a user, without an invitation.
-- The `message_id` of such an intent is the text of its ID.
ALTER TABLE outbound_intent
    DROP CONSTRAINT outbound_intent_purpose_check,
    DROP CONSTRAINT outbound_intent_status_check,
    DROP CONSTRAINT outbound_intent_check,
    DROP CONSTRAINT outbound_intent_check1;
ALTER TABLE outbound_intent
    ADD CONSTRAINT outbound_intent_purpose_check
        CHECK (purpose IN ('magic-link', 'invitation', 'telegram-digest')),
    ADD CONSTRAINT outbound_intent_status_check
        CHECK (status IN ('pending', 'sent', 'failed', 'unknown', 'canceled')),
    ADD CONSTRAINT outbound_intent_columns_check CHECK (
        CASE purpose
            WHEN 'invitation' THEN organization_id IS NOT NULL AND invitation_id IS NOT NULL AND user_id IS NULL
            WHEN 'magic-link' THEN user_id IS NOT NULL AND invitation_id IS NULL
            WHEN 'telegram-digest' THEN organization_id IS NOT NULL AND user_id IS NOT NULL AND invitation_id IS NULL
            ELSE false
        END
    );

-- One item of a digest. The opaque `id` is the reference in a button; it holds no free text.
CREATE TABLE digest_item (
    id uuid PRIMARY KEY,
    intent_id uuid NOT NULL REFERENCES outbound_intent (id) ON DELETE CASCADE,
    organization_id uuid NOT NULL REFERENCES organization (id),
    record_kind text NOT NULL CHECK (record_kind IN ('action', 'commitment')),
    record_id uuid NOT NULL,
    record_version bigint NOT NULL,
    reason text NOT NULL CHECK (reason IN ('overdue', 'due-soon', 'blocked', 'follow-up')),
    position smallint NOT NULL,
    UNIQUE (intent_id, position)
);

-- What tada told one user about one record, and what the user answered.
CREATE TABLE reminder_state (
    organization_id uuid NOT NULL REFERENCES organization (id),
    record_kind text NOT NULL CHECK (record_kind IN ('action', 'commitment')),
    record_id uuid NOT NULL,
    user_id uuid NOT NULL REFERENCES app_user (id),
    notified_version bigint NOT NULL,
    notified_at timestamptz NOT NULL,
    response text CHECK (response IN ('on-track', 'blocked', 'done', 'fulfilled')),
    response_version bigint,
    response_at timestamptz,
    PRIMARY KEY (organization_id, record_kind, record_id, user_id)
);

-- Prices of a model for a period, in CHF for one million tokens.
CREATE TABLE ai_price (
    model_id text NOT NULL,
    valid_from date NOT NULL,
    input_chf_per_mtok numeric NOT NULL CHECK (input_chf_per_mtok >= 0),
    output_chf_per_mtok numeric NOT NULL CHECK (output_chf_per_mtok >= 0),
    PRIMARY KEY (model_id, valid_from)
);

-- The monthly limit of an organization. An organization without a row uses the operator default.
CREATE TABLE ai_cap (
    organization_id uuid PRIMARY KEY REFERENCES organization (id),
    monthly_chf numeric NOT NULL CHECK (monthly_chf > 0),
    version bigint NOT NULL DEFAULT 1
);

-- One call to the model API. The row holds counts and a cost, no text.
CREATE TABLE ai_usage (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    event_id uuid,
    task text NOT NULL,
    prompt_version integer NOT NULL,
    model_id text NOT NULL,
    input_tokens integer NOT NULL CHECK (input_tokens >= 0),
    output_tokens integer NOT NULL CHECK (output_tokens >= 0),
    cost_chf numeric NOT NULL CHECK (cost_chf >= 0),
    at timestamptz NOT NULL,
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);
CREATE INDEX ai_usage_organization_at ON ai_usage (organization_id, at);

-- The role, version and commit of the binary that wrote the heartbeat. A previous binary writes none.
ALTER TABLE worker_heartbeat
    ADD COLUMN role text,
    ADD COLUMN version text,
    ADD COLUMN commit text;
