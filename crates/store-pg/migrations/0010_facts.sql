-- Sources, the field catalog, facts and evidence (ADR 0049, ADR 0050).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

-- An incoming item. An item without an event belongs to the organization, for example an upload.
CREATE TABLE source_item (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    event_id uuid,
    kind text NOT NULL CHECK (kind IN ('member-text', 'upload', 'web-snapshot', 'review')),
    created_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);

-- One immutable version of a source item.
-- `text` is the normalized text: Unicode NFC with LF line ends. Passages count characters in it.
-- `sha256` is the hash of the normalized text, or of the file of an upload (ADR 0009).
-- `author_actor` is the actor of the store-pg `actor` codec (ADR 0039).
CREATE TABLE source_version (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    source_item_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('member-text', 'upload', 'web-snapshot', 'review')),
    channel text CHECK (channel IN ('web', 'telegram', 'job', 'api-token', 'cli')),
    author_actor jsonb NOT NULL,
    text text,
    sha256 bytea NOT NULL CHECK (octet_length(sha256) = 32),
    url text,
    retrieved_at timestamptz,
    captured_at timestamptz NOT NULL,
    -- The texts mix German and English, so the search uses the `simple` configuration without stemming.
    search tsvector GENERATED ALWAYS AS (to_tsvector('simple', coalesce(text, ''))) STORED,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, source_item_id) REFERENCES source_item (organization_id, id),
    CHECK (kind NOT IN ('member-text', 'web-snapshot', 'review') OR text IS NOT NULL),
    CHECK ((kind = 'web-snapshot') = (url IS NOT NULL AND retrieved_at IS NOT NULL))
);

CREATE INDEX source_version_search ON source_version USING gin (search);

-- A field definition of the field catalog (ADR 0049).
-- tada ships some fields: they have no organization and no event, and `tada migrate` writes them from the Rust catalog.
-- The other fields belong to one event.
CREATE TABLE field_definition (
    id uuid PRIMARY KEY,
    organization_id uuid REFERENCES organization (id),
    event_id uuid,
    key text NOT NULL CHECK (key ~ '^[a-z][a-z0-9]*(_[a-z0-9]+)*$' AND char_length(key) BETWEEN 2 AND 64),
    label_text text,
    label_message text,
    value_type jsonb NOT NULL,
    description text NOT NULL,
    module text NOT NULL,
    status text NOT NULL CHECK (status IN ('active', 'deprecated')),
    catalog_version bigint,
    created_at timestamptz NOT NULL,
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id),
    CONSTRAINT field_definition_scope CHECK (
        (organization_id IS NULL AND event_id IS NULL AND catalog_version IS NOT NULL)
        OR (organization_id IS NOT NULL AND event_id IS NOT NULL AND catalog_version IS NULL)
    ),
    CONSTRAINT field_definition_one_label CHECK ((label_text IS NULL) <> (label_message IS NULL)),
    CONSTRAINT field_definition_event_key_unique UNIQUE (event_id, key)
);

CREATE UNIQUE INDEX field_definition_shipped_key_unique ON field_definition (key) WHERE event_id IS NULL;

-- A fact: one field of one event (ADR 0049). `version` is the number of the current fact version.
--
-- The one schema exception to ADR 0006: `field_id` has a single-column foreign key.
-- Shipped field definitions are not organization data, so they have no organization_id and cannot be the target of a composite foreign key.
-- The trigger below checks that a field of an event belongs to the organization and the event of the fact.
CREATE TABLE fact (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    event_id uuid NOT NULL,
    field_id uuid NOT NULL REFERENCES field_definition (id),
    version bigint NOT NULL CHECK (version >= 1),
    UNIQUE (organization_id, id),
    UNIQUE (event_id, field_id),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);

CREATE FUNCTION fact_field_in_scope() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM field_definition
        WHERE id = NEW.field_id
          AND event_id IS NOT NULL
          AND (organization_id <> NEW.organization_id OR event_id <> NEW.event_id)
    ) THEN
        RAISE EXCEPTION 'the field of the fact belongs to another event'
            USING ERRCODE = 'check_violation', CONSTRAINT = 'fact_field_in_scope';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER fact_field_in_scope
    BEFORE INSERT OR UPDATE OF organization_id, event_id, field_id ON fact
    FOR EACH ROW EXECUTE FUNCTION fact_field_in_scope();

-- One immutable state of a fact. An unknown never holds a value (ADR 0049).
-- `accepted_by` is the actor of the store-pg `actor` codec (ADR 0039).
-- `proposal_id` names the proposal that created the version (ADR 0050).
CREATE TABLE fact_version (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    fact_id uuid NOT NULL,
    number bigint NOT NULL CHECK (number >= 1),
    state text NOT NULL CHECK (state IN ('accepted', 'assumption', 'unknown')),
    value jsonb,
    approximate boolean NOT NULL,
    created_at timestamptz NOT NULL,
    accepted_by jsonb NOT NULL,
    proposal_id uuid,
    UNIQUE (organization_id, id),
    UNIQUE (fact_id, number),
    FOREIGN KEY (organization_id, fact_id) REFERENCES fact (organization_id, id),
    CONSTRAINT fact_version_unknown_has_no_value CHECK ((state = 'unknown') = (value IS NULL)),
    CHECK (state <> 'unknown' OR NOT approximate)
);

-- A link from a fact version to a passage of a source version (ADR 0050).
-- The offsets count characters of the normalized text of the source version.
CREATE TABLE evidence_link (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    fact_version_id uuid NOT NULL,
    source_version_id uuid NOT NULL,
    start_offset int NOT NULL CHECK (start_offset >= 0),
    end_offset int NOT NULL,
    quote text NOT NULL,
    page int CHECK (page >= 1),
    CHECK (end_offset > start_offset),
    FOREIGN KEY (organization_id, fact_version_id) REFERENCES fact_version (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX evidence_link_fact_version ON evidence_link (organization_id, fact_version_id);
