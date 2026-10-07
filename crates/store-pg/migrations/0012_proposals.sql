-- Changesets, proposals, their evidence and review results, and open questions (ADR 0050).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

-- The proposals of one intake. A changeset without an event belongs to the organization, for example a new event.
-- `author` is the actor of the store-pg `actor` codec (ADR 0039).
-- `source_version_id` is the text of the intake. The passages of the evidence point into it.
CREATE TABLE changeset (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    event_id uuid,
    author jsonb NOT NULL,
    source_version_id uuid NOT NULL,
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX changeset_event ON changeset (organization_id, event_id, created_at);

-- One proposal with its typed operation (ADR 0050). A proposal never changes.
-- `operation` is the operation of the store-pg `proposals` codec, in the format `operation_version`.
-- `target_kind` and `target_id` name the record that the operation creates or changes.
-- A fact has no ID before its first version, so the target of a fact is its field, in the event of the operation.
-- `expected_version` is the version of the target that the operation expects; NULL means that the target does not exist yet.
-- `event_id` is the event that the operation works in; for a new event, it is that event.
-- It has no foreign key, because the event of a proposal in an organization changeset exists only after its `CreateEvent` applies, or never.
CREATE TABLE proposal (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    changeset_id uuid NOT NULL,
    event_id uuid NOT NULL,
    operation jsonb NOT NULL,
    operation_version int NOT NULL CHECK (operation_version >= 1),
    target_kind text NOT NULL CHECK (target_kind IN ('event', 'fact', 'field_definition', 'open_question')),
    target_id uuid NOT NULL,
    expected_version bigint CHECK (expected_version >= 1),
    reason text NOT NULL,
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, changeset_id, id),
    FOREIGN KEY (organization_id, changeset_id) REFERENCES changeset (organization_id, id)
);

CREATE INDEX proposal_target ON proposal (organization_id, event_id, target_kind, target_id);

-- A dependency between two proposals of the same changeset.
CREATE TABLE proposal_dependency (
    organization_id uuid NOT NULL,
    changeset_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    depends_on uuid NOT NULL,
    PRIMARY KEY (proposal_id, depends_on),
    CHECK (proposal_id <> depends_on),
    FOREIGN KEY (organization_id, changeset_id, proposal_id)
        REFERENCES proposal (organization_id, changeset_id, id),
    FOREIGN KEY (organization_id, changeset_id, depends_on)
        REFERENCES proposal (organization_id, changeset_id, id)
);

-- A passage of a source version that supports a proposal (ADR 0040, ADR 0050).
-- The offsets count characters of the normalized text of the source version.
CREATE TABLE proposal_evidence (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    source_version_id uuid NOT NULL,
    start_offset int NOT NULL CHECK (start_offset >= 0),
    end_offset int NOT NULL,
    quote text NOT NULL,
    page int CHECK (page >= 1),
    CHECK (end_offset > start_offset),
    FOREIGN KEY (organization_id, proposal_id) REFERENCES proposal (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX proposal_evidence_proposal ON proposal_evidence (organization_id, proposal_id);

-- Changesets, proposals, their dependencies and their evidence never change, and review results are append-only (ADR 0050).
-- The application never updates or deletes them; these triggers are the second line of defense.
CREATE FUNCTION reject_proposal_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'the rows of % never change', TG_TABLE_NAME
        USING ERRCODE = 'restrict_violation', CONSTRAINT = 'proposal_immutable';
END;
$$;

CREATE TRIGGER changeset_immutable
    BEFORE UPDATE OR DELETE ON changeset
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER changeset_no_truncate
    BEFORE TRUNCATE ON changeset
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_immutable
    BEFORE UPDATE OR DELETE ON proposal
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_no_truncate
    BEFORE TRUNCATE ON proposal
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_dependency_immutable
    BEFORE UPDATE OR DELETE ON proposal_dependency
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_dependency_no_truncate
    BEFORE TRUNCATE ON proposal_dependency
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_evidence_immutable
    BEFORE UPDATE OR DELETE ON proposal_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER proposal_evidence_no_truncate
    BEFORE TRUNCATE ON proposal_evidence
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();

-- The append-only review results of a proposal (ADR 0050). The latest result gives the status of the proposal.
-- `reviewer` is the actor of the store-pg `actor` codec (ADR 0039).
-- `edit_source_version_id` is the source version of kind `review` of an edited value.
CREATE TABLE review_result (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    result text NOT NULL CHECK (result IN ('accepted', 'accepted-with-edit', 'rejected', 'conflict', 'withdrawn')),
    reviewer jsonb NOT NULL,
    edit_source_version_id uuid,
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    CHECK ((result = 'accepted-with-edit') = (edit_source_version_id IS NOT NULL)),
    FOREIGN KEY (organization_id, proposal_id) REFERENCES proposal (organization_id, id),
    FOREIGN KEY (organization_id, edit_source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX review_result_proposal ON review_result (organization_id, proposal_id, created_at);

CREATE TRIGGER review_result_immutable
    BEFORE UPDATE OR DELETE ON review_result
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER review_result_no_truncate
    BEFORE TRUNCATE ON review_result
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();

-- The next event-local number of each kind of record in each scope, for example `QST` in an event (ADR 0038).
CREATE TABLE local_id_counter (
    organization_id uuid NOT NULL REFERENCES organization (id),
    scope_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind ~ '^[A-Z]{3}$'),
    next bigint NOT NULL CHECK (next >= 1),
    PRIMARY KEY (organization_id, scope_id, kind)
);

-- An open question of an event (ADR 0049). It has an owner and the event-local ID `QST-<local_number>`.
CREATE TABLE open_question (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    text text NOT NULL,
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    status text NOT NULL CHECK (status IN ('open', 'closed')),
    version bigint NOT NULL CHECK (version >= 1),
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    UNIQUE (event_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);
