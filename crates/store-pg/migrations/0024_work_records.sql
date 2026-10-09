-- The work records of an event and the parties that they name (Slice 2a).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).
-- The readable IDs `PER-<n>` and `INS-<n>` count in the organization, `ACT-<n>` and `COM-<n>` in the event (ADR 0038).
-- They use the `local_id_counter` of migration 0012: its `scope_id` is the organization or the event.

-- A person that a commitment or an action names. It can be a member, then `user_id` points to the account.
CREATE TABLE person (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    local_number bigint NOT NULL CHECK (local_number >= 1),
    name text NOT NULL,
    email text,
    phone text,
    user_id uuid REFERENCES app_user (id),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, local_number)
);

-- An institution that a commitment names: an authority, a company, a club or another body.
CREATE TABLE institution (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    local_number bigint NOT NULL CHECK (local_number >= 1),
    name text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('authority', 'company', 'club', 'other')),
    email text,
    phone text,
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, local_number)
);

-- A workstream of an event. It has a lead. A name is unique in the event, whatever its case.
CREATE TABLE workstream (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    name text NOT NULL,
    lead_user_id uuid NOT NULL REFERENCES app_user (id),
    status text NOT NULL CHECK (status IN ('active', 'closed')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, event_id, id),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);

CREATE UNIQUE INDEX workstream_name ON workstream (event_id, lower(name));

-- An action of an event: a task with one owner. The composite key keeps its workstream in the same event.
CREATE TABLE action (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    title text NOT NULL,
    description text,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    due_date date,
    status text NOT NULL CHECK (status IN ('open', 'in-progress', 'blocked', 'done', 'canceled')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id)
);

CREATE INDEX action_open_of_owner ON action (organization_id, owner_user_id)
    WHERE status IN ('open', 'in-progress', 'blocked');

-- A commitment of an event: a promise of one person or one institution. A conditional one names its condition.
CREATE TABLE commitment (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    text text NOT NULL,
    condition text,
    person_id uuid,
    institution_id uuid,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    workstream_id uuid,
    due_date date,
    status text NOT NULL CHECK (status IN ('conditional', 'firm', 'fulfilled', 'broken', 'withdrawn')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    CHECK (num_nonnulls(person_id, institution_id) = 1),
    CHECK (status <> 'conditional' OR condition IS NOT NULL),
    UNIQUE (organization_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, event_id, workstream_id)
        REFERENCES workstream (organization_id, event_id, id),
    FOREIGN KEY (organization_id, person_id) REFERENCES person (organization_id, id),
    FOREIGN KEY (organization_id, institution_id) REFERENCES institution (organization_id, id)
);

CREATE INDEX commitment_open_of_owner ON commitment (organization_id, owner_user_id)
    WHERE status IN ('conditional', 'firm');

-- A passage of a source version that supports a work record, with the version of the record that it supports.
-- It copies a passage of the proposal that created or changed the record. Exactly one record column is set.
-- The offsets count characters of the normalized text of the source version, as in `evidence_link`.
CREATE TABLE record_evidence (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    action_id uuid,
    commitment_id uuid,
    person_id uuid,
    institution_id uuid,
    record_version bigint NOT NULL CHECK (record_version >= 1),
    proposal_id uuid NOT NULL,
    source_version_id uuid NOT NULL,
    start_offset int NOT NULL CHECK (start_offset >= 0),
    end_offset int NOT NULL,
    quote text NOT NULL,
    page int CHECK (page >= 1),
    CHECK (num_nonnulls(action_id, commitment_id, person_id, institution_id) = 1),
    CHECK (end_offset > start_offset),
    FOREIGN KEY (organization_id, action_id) REFERENCES action (organization_id, id),
    FOREIGN KEY (organization_id, commitment_id) REFERENCES commitment (organization_id, id),
    FOREIGN KEY (organization_id, person_id) REFERENCES person (organization_id, id),
    FOREIGN KEY (organization_id, institution_id) REFERENCES institution (organization_id, id),
    FOREIGN KEY (organization_id, proposal_id) REFERENCES proposal (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX record_evidence_action ON record_evidence (organization_id, action_id) WHERE action_id IS NOT NULL;
CREATE INDEX record_evidence_commitment ON record_evidence (organization_id, commitment_id) WHERE commitment_id IS NOT NULL;
CREATE INDEX record_evidence_person ON record_evidence (organization_id, person_id) WHERE person_id IS NOT NULL;
CREATE INDEX record_evidence_institution ON record_evidence (organization_id, institution_id) WHERE institution_id IS NOT NULL;

-- A proposal can target the new kinds of record.
ALTER TABLE proposal DROP CONSTRAINT proposal_target_kind_check;
ALTER TABLE proposal ADD CONSTRAINT proposal_target_kind_check
    CHECK (target_kind IN (
        'event', 'fact', 'field_definition', 'open_question', 'document',
        'workstream', 'action', 'commitment', 'person', 'institution'
    ));
