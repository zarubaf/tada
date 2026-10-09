-- Documents, their versions and the storage quota (ADR 0009, ADR 0043, ADR 0051).
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

-- The storage quota of each organization, in bytes (ADR 0043).
-- It is a value in the database, not an environment setting: an operator changes it for one organization with SQL.
-- The default is 5 GiB.
ALTER TABLE organization
    ADD COLUMN storage_quota_bytes bigint NOT NULL DEFAULT 5368709120 CHECK (storage_quota_bytes >= 0);

-- A document: a file with a stable ID and the organization-local ID `DOC-<local_number>` (ADR 0038).
-- In Slice 1, each document belongs to one event, and access follows the event role (ADR 0052, ARCHITECTURE.md).
-- `version` is the record version: it increases with each new document version.
CREATE TABLE document (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    event_id uuid NOT NULL,
    local_number bigint NOT NULL CHECK (local_number >= 1),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    owner_user_id uuid NOT NULL REFERENCES app_user (id),
    created_at timestamptz NOT NULL,
    version bigint NOT NULL CHECK (version >= 1),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, local_number),
    FOREIGN KEY (organization_id, event_id) REFERENCES event (organization_id, id)
);

CREATE INDEX document_event ON document (organization_id, event_id, local_number);

-- One immutable version of a document: an upload or a draft (ADR 0051).
-- An upload has a file in the object storage under `blob_key` and a source version of the kind `upload` (ADR 0050).
-- `sha256` is the hash of the file of an upload.
-- Only a draft has a status, and approval applies to drafts only (ADR 0051).
CREATE TABLE document_version (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    document_id uuid NOT NULL,
    number int NOT NULL CHECK (number >= 1),
    kind text NOT NULL CHECK (kind IN ('upload', 'draft')),
    blob_key text UNIQUE,
    media_type text,
    size_bytes bigint CHECK (size_bytes >= 0),
    sha256 bytea NOT NULL CHECK (octet_length(sha256) = 32),
    file_name text CHECK (char_length(file_name) BETWEEN 1 AND 200),
    uploaded_by uuid NOT NULL REFERENCES app_user (id),
    source_version_id uuid,
    status text CHECK (status IN ('draft', 'review', 'approved', 'superseded', 'archived')),
    created_at timestamptz NOT NULL,
    approved_by uuid REFERENCES app_user (id),
    approved_at timestamptz,
    UNIQUE (organization_id, id),
    UNIQUE (document_id, number),
    FOREIGN KEY (organization_id, document_id) REFERENCES document (organization_id, id),
    FOREIGN KEY (organization_id, source_version_id) REFERENCES source_version (organization_id, id),
    -- The key of an object starts with the organization ID (ADR 0009), so a row cannot name an object of another organization.
    CONSTRAINT document_version_blob_key_in_organization CHECK (
        blob_key IS NULL OR starts_with(blob_key, organization_id::text || '/')
    ),
    CONSTRAINT document_version_status_of_drafts CHECK ((kind = 'draft') = (status IS NOT NULL)),
    CONSTRAINT document_version_upload_file CHECK (
        kind <> 'upload' OR (
            blob_key IS NOT NULL AND media_type IS NOT NULL AND size_bytes IS NOT NULL
            AND file_name IS NOT NULL AND source_version_id IS NOT NULL
        )
    ),
    CONSTRAINT document_version_approval CHECK (
        (approved_by IS NULL) = (approved_at IS NULL) AND (approved_at IS NULL OR kind = 'draft')
    )
);

-- The storage quota adds the sizes of the versions of each organization.
CREATE INDEX document_version_organization ON document_version (organization_id);

-- The content of a document version never changes (ADR 0009, ADR 0051).
-- Only the status and the approval of a draft can change.
-- The application never updates the content; this trigger is the second line of defense.
CREATE FUNCTION document_version_content_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.id, NEW.organization_id, NEW.document_id, NEW.number, NEW.kind, NEW.blob_key,
        NEW.media_type, NEW.size_bytes, NEW.sha256, NEW.file_name, NEW.uploaded_by,
        NEW.source_version_id, NEW.created_at)
       IS DISTINCT FROM
       (OLD.id, OLD.organization_id, OLD.document_id, OLD.number, OLD.kind, OLD.blob_key,
        OLD.media_type, OLD.size_bytes, OLD.sha256, OLD.file_name, OLD.uploaded_by,
        OLD.source_version_id, OLD.created_at)
    THEN
        RAISE EXCEPTION 'the content of a document version never changes'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_content_immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER document_version_content_immutable
    BEFORE UPDATE ON document_version
    FOR EACH ROW EXECUTE FUNCTION document_version_content_immutable();
