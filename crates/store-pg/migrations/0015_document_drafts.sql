-- Document drafts and their provenance manifests (ADR 0050, ADR 0051).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

-- A draft proposal targets a document: a new one, or an existing one at its expected version.
ALTER TABLE proposal DROP CONSTRAINT proposal_target_kind_check;
ALTER TABLE proposal ADD CONSTRAINT proposal_target_kind_check
    CHECK (target_kind IN ('event', 'fact', 'field_definition', 'open_question', 'document'));

-- The provenance manifest and the lint warnings of a draft proposal, fixed when tada stores the proposal (ADR 0051).
-- `manifest` is `{"facts": [{"fact_id", "version"}], "sources": [{"source_version_id", "start", "end", "quote"}]}`
-- and `lint_warnings` is `[{"line", "kind"}]`, in the format of the store-pg `proposals` codec.
-- Only a draft proposal has them. The UPDATE trigger of the proposal keeps them unchanged after the insert.
ALTER TABLE proposal
    ADD COLUMN manifest jsonb,
    ADD COLUMN lint_warnings jsonb,
    ADD CONSTRAINT proposal_draft_provenance CHECK (
        (target_kind = 'document') = (manifest IS NOT NULL)
        AND (manifest IS NULL) = (lint_warnings IS NULL)
    );

-- The Markdown text of a draft version, with LF line ends (ADR 0058). `sha256` is its hash.
-- A draft has no file. For a draft, `uploaded_by` is the member who accepted its proposal.
ALTER TABLE document_version
    ADD COLUMN markdown text,
    ADD CONSTRAINT document_version_markdown_of_drafts CHECK ((kind = 'draft') = (markdown IS NOT NULL)),
    ADD CONSTRAINT document_version_draft_without_file CHECK (
        kind <> 'draft' OR (
            blob_key IS NULL AND media_type IS NULL AND size_bytes IS NULL
            AND file_name IS NULL AND source_version_id IS NULL
        )
    );

-- The Markdown is content too: it never changes.
CREATE OR REPLACE FUNCTION document_version_content_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.id, NEW.organization_id, NEW.document_id, NEW.number, NEW.kind, NEW.blob_key,
        NEW.media_type, NEW.size_bytes, NEW.sha256, NEW.file_name, NEW.uploaded_by,
        NEW.source_version_id, NEW.created_at, NEW.markdown)
       IS DISTINCT FROM
       (OLD.id, OLD.organization_id, OLD.document_id, OLD.number, OLD.kind, OLD.blob_key,
        OLD.media_type, OLD.size_bytes, OLD.sha256, OLD.file_name, OLD.uploaded_by,
        OLD.source_version_id, OLD.created_at, OLD.markdown)
    THEN
        RAISE EXCEPTION 'the content of a document version never changes'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_content_immutable';
    END IF;
    RETURN NEW;
END;
$$;

-- A manifest row names an exact fact version, so the fact versions get a key with the organization.
ALTER TABLE fact_version
    ADD CONSTRAINT fact_version_organization_fact_number_key UNIQUE (organization_id, fact_id, number);

-- A fact version that a draft version cites (ADR 0051).
CREATE TABLE document_manifest_fact (
    organization_id uuid NOT NULL,
    document_version_id uuid NOT NULL,
    fact_id uuid NOT NULL,
    fact_version_number bigint NOT NULL CHECK (fact_version_number >= 1),
    PRIMARY KEY (document_version_id, fact_id, fact_version_number),
    FOREIGN KEY (organization_id, document_version_id) REFERENCES document_version (organization_id, id),
    FOREIGN KEY (organization_id, fact_id, fact_version_number)
        REFERENCES fact_version (organization_id, fact_id, number)
);

-- A source passage that a draft version cites (ADR 0051).
-- The offsets count characters of the normalized text of the source version.
CREATE TABLE document_manifest_source (
    organization_id uuid NOT NULL,
    document_version_id uuid NOT NULL,
    source_version_id uuid NOT NULL,
    start_offset int NOT NULL CHECK (start_offset >= 0),
    end_offset int NOT NULL,
    PRIMARY KEY (document_version_id, source_version_id, start_offset, end_offset),
    CHECK (end_offset > start_offset),
    FOREIGN KEY (organization_id, document_version_id) REFERENCES document_version (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id)
);

CREATE INDEX document_manifest_fact_fact ON document_manifest_fact (organization_id, fact_id);
CREATE INDEX document_manifest_source_source ON document_manifest_source (organization_id, source_version_id);

-- The manifest is part of its document version and never changes (ADR 0051).
-- The application never updates or deletes it; these triggers are the second line of defense.
CREATE FUNCTION reject_manifest_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'the rows of % never change', TG_TABLE_NAME
        USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_manifest_immutable';
END;
$$;

CREATE TRIGGER document_manifest_fact_immutable
    BEFORE UPDATE OR DELETE ON document_manifest_fact
    FOR EACH ROW EXECUTE FUNCTION reject_manifest_change();
CREATE TRIGGER document_manifest_fact_no_truncate
    BEFORE TRUNCATE ON document_manifest_fact
    FOR EACH STATEMENT EXECUTE FUNCTION reject_manifest_change();
CREATE TRIGGER document_manifest_source_immutable
    BEFORE UPDATE OR DELETE ON document_manifest_source
    FOR EACH ROW EXECUTE FUNCTION reject_manifest_change();
CREATE TRIGGER document_manifest_source_no_truncate
    BEFORE TRUNCATE ON document_manifest_source
    FOR EACH STATEMENT EXECUTE FUNCTION reject_manifest_change();
