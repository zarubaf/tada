-- Milestones, decisions, risks and requirements of an event (Slice 2d).
-- They follow `action` (migration 0024) and its redaction columns (migration 0026).
-- The readable IDs `MIL-<n>`, `DEC-<n>`, `RSK-<n>` and `REQ-<n>` count in the event (ADR 0038).
-- The composite key `(organization_id, event_id, id)` keeps a reference in the same event.

-- A milestone: a date that the plan of the event holds.
CREATE TABLE milestone (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    title text,
    description text,
    date date NOT NULL,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    status text NOT NULL CHECK (status IN ('open', 'reached', 'canceled')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    redacted_at timestamptz,
    CONSTRAINT milestone_title_or_redacted CHECK (title IS NOT NULL OR redacted_at IS NOT NULL),
    CONSTRAINT milestone_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(title, description) = 0
    ),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, event_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id)
);

CREATE INDEX milestone_open_of_owner ON milestone (organization_id, owner_user_id)
    WHERE status = 'open';

-- A decision: the wording of what the club decided. A decision that a newer one replaces names it.
CREATE TABLE decision (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    wording text,
    decided_on date NOT NULL,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    status text NOT NULL CHECK (status IN ('in-force', 'superseded', 'revoked')),
    superseded_by uuid,
    revoked_reason text,
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    redacted_at timestamptz,
    CONSTRAINT decision_wording_or_redacted CHECK (wording IS NOT NULL OR redacted_at IS NOT NULL),
    CONSTRAINT decision_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(wording, revoked_reason) = 0
    ),
    CONSTRAINT decision_successor_of_superseded CHECK (
        (superseded_by IS NOT NULL) = (status = 'superseded')
    ),
    CONSTRAINT decision_reason_of_revoked CHECK (
        (revoked_reason IS NOT NULL) = (status = 'revoked') OR (status = 'revoked' AND redacted_at IS NOT NULL)
    ),
    CONSTRAINT decision_not_superseded_by_itself CHECK (superseded_by IS DISTINCT FROM id),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, event_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id),
    FOREIGN KEY (organization_id, event_id, superseded_by)
        REFERENCES decision (organization_id, event_id, id)
);

CREATE INDEX decision_in_force_of_owner ON decision (organization_id, owner_user_id)
    WHERE status = 'in-force';

-- A party that decided: a person or an institution. `position` keeps the order.
CREATE TABLE decision_party (
    organization_id uuid NOT NULL,
    decision_id uuid NOT NULL,
    position smallint NOT NULL CHECK (position >= 0),
    person_id uuid,
    institution_id uuid,
    PRIMARY KEY (decision_id, position),
    CHECK (num_nonnulls(person_id, institution_id) = 1),
    FOREIGN KEY (organization_id, decision_id) REFERENCES decision (organization_id, id),
    FOREIGN KEY (organization_id, person_id) REFERENCES person (organization_id, id),
    FOREIGN KEY (organization_id, institution_id) REFERENCES institution (organization_id, id)
);

CREATE INDEX decision_party_person ON decision_party (organization_id, person_id)
    WHERE person_id IS NOT NULL;
CREATE INDEX decision_party_institution ON decision_party (organization_id, institution_id)
    WHERE institution_id IS NOT NULL;

-- A risk of an event: how likely it is, how much it hurts, and what the club does about it.
CREATE TABLE risk (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    title text,
    description text,
    likelihood text NOT NULL CHECK (likelihood IN ('low', 'medium', 'high')),
    impact text NOT NULL CHECK (impact IN ('low', 'medium', 'high')),
    mitigation text,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    status text NOT NULL CHECK (status IN ('open', 'mitigated', 'occurred', 'closed')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    redacted_at timestamptz,
    CONSTRAINT risk_title_or_redacted CHECK (title IS NOT NULL OR redacted_at IS NOT NULL),
    CONSTRAINT risk_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(title, description, mitigation) = 0
    ),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, event_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id)
);

CREATE INDEX risk_open_of_owner ON risk (organization_id, owner_user_id)
    WHERE status = 'open';

-- A requirement that a person or an institution sets for the event. It can name nobody.
CREATE TABLE requirement (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    text text,
    person_id uuid,
    institution_id uuid,
    due_date date,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    status text NOT NULL CHECK (status IN ('open', 'met', 'waived')),
    waived_reason text,
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    redacted_at timestamptz,
    CONSTRAINT requirement_one_party_at_most CHECK (num_nonnulls(person_id, institution_id) <= 1),
    CONSTRAINT requirement_text_or_redacted CHECK (text IS NOT NULL OR redacted_at IS NOT NULL),
    CONSTRAINT requirement_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(text, waived_reason) = 0
    ),
    CONSTRAINT requirement_reason_of_waived CHECK (
        (waived_reason IS NOT NULL) = (status = 'waived') OR (status = 'waived' AND redacted_at IS NOT NULL)
    ),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, event_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id),
    FOREIGN KEY (organization_id, person_id) REFERENCES person (organization_id, id),
    FOREIGN KEY (organization_id, institution_id) REFERENCES institution (organization_id, id)
);

CREATE INDEX requirement_open_of_owner ON requirement (organization_id, owner_user_id)
    WHERE status = 'open';
CREATE INDEX requirement_person ON requirement (organization_id, person_id)
    WHERE person_id IS NOT NULL;
CREATE INDEX requirement_institution ON requirement (organization_id, institution_id)
    WHERE institution_id IS NOT NULL;

-- An action can start on a date and belong to a milestone of its event.
ALTER TABLE action
    ADD COLUMN start_date date,
    ADD COLUMN milestone_id uuid,
    ADD CONSTRAINT action_start_not_after_due CHECK (
        start_date IS NULL OR due_date IS NULL OR start_date <= due_date
    ),
    ADD CONSTRAINT action_milestone_fkey FOREIGN KEY (organization_id, event_id, milestone_id)
        REFERENCES milestone (organization_id, event_id, id);

-- Evidence can support the new records. Exactly one of the eight record columns is set.
ALTER TABLE record_evidence
    ADD COLUMN milestone_id uuid,
    ADD COLUMN decision_id uuid,
    ADD COLUMN risk_id uuid,
    ADD COLUMN requirement_id uuid,
    DROP CONSTRAINT record_evidence_check,
    ADD CONSTRAINT record_evidence_one_record CHECK (
        num_nonnulls(action_id, commitment_id, person_id, institution_id,
                     milestone_id, decision_id, risk_id, requirement_id) = 1
    ),
    ADD FOREIGN KEY (organization_id, milestone_id) REFERENCES milestone (organization_id, id),
    ADD FOREIGN KEY (organization_id, decision_id) REFERENCES decision (organization_id, id),
    ADD FOREIGN KEY (organization_id, risk_id) REFERENCES risk (organization_id, id),
    ADD FOREIGN KEY (organization_id, requirement_id) REFERENCES requirement (organization_id, id);

CREATE INDEX record_evidence_milestone ON record_evidence (organization_id, milestone_id) WHERE milestone_id IS NOT NULL;
CREATE INDEX record_evidence_decision ON record_evidence (organization_id, decision_id) WHERE decision_id IS NOT NULL;
CREATE INDEX record_evidence_risk ON record_evidence (organization_id, risk_id) WHERE risk_id IS NOT NULL;
CREATE INDEX record_evidence_requirement ON record_evidence (organization_id, requirement_id) WHERE requirement_id IS NOT NULL;

-- A proposal can target the new kinds. They belong to an event, so `proposal_event_of_target` stays.
ALTER TABLE proposal DROP CONSTRAINT proposal_target_kind_check;
ALTER TABLE proposal ADD CONSTRAINT proposal_target_kind_check
    CHECK (target_kind IN (
        'event', 'fact', 'field_definition', 'open_question', 'document',
        'workstream', 'action', 'commitment', 'person', 'institution',
        'milestone', 'decision', 'risk', 'requirement'
    ));
