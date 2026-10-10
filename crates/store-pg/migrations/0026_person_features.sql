-- Event closing, retention periods, legal redaction and dispute notes (ADR 0071, ADR 0072, ADR 0073), expand only.
-- Tables with organization data refer to each other through (organization_id, id) (ADR 0006).

-- An event is active or closed. A closed event is read-only. Retention redacts a closed event only (ADR 0071).
ALTER TABLE event
    ADD COLUMN status text NOT NULL DEFAULT 'active' CONSTRAINT event_status CHECK (status IN ('active', 'closed')),
    ADD COLUMN closed_at timestamptz,
    ADD COLUMN redacted_at timestamptz,
    ADD CONSTRAINT event_closed_at CHECK ((status = 'closed') = (closed_at IS NOT NULL)),
    ADD CONSTRAINT event_redacted_when_closed CHECK (redacted_at IS NULL OR status = 'closed');

-- The retention periods in months. NULL means that retention is off: tada has no default (ADR 0071).
ALTER TABLE organization
    ADD COLUMN retention_closed_event_months smallint
        CONSTRAINT organization_retention_closed_event_months CHECK (retention_closed_event_months BETWEEN 1 AND 120),
    ADD COLUMN retention_source_months smallint
        CONSTRAINT organization_retention_source_months CHECK (retention_source_months BETWEEN 1 AND 120);

-- The end of the last organization membership of an account, and its anonymization 90 days later (ADR 0071).
ALTER TABLE app_user
    ADD COLUMN membership_ended_at timestamptz,
    ADD COLUMN anonymized_at timestamptz;

-- The tombstone of a legal redaction (ADR 0072): `redacted_at` is set and the content columns are NULL.
-- Each content column that had to have a value can be NULL only in a redacted row.
-- In the tables that change (parties, work records, open questions, documents, dispute notes), a CHECK keeps
-- each content column of a redacted row NULL, so that no later change brings content back.
-- The row keeps its IDs, statuses, dates and numbers, so that links and manifests still resolve.

-- A source item has no content of its own. Its redaction redacts all its versions.
ALTER TABLE source_item ADD COLUMN redacted_at timestamptz;

-- Source version: the text, the URL, the hash and the personal parts of the author.
-- The search vector comes from the text, so it becomes empty.
ALTER TABLE source_version
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN sha256 DROP NOT NULL,
    ADD CONSTRAINT source_version_sha256_or_redacted CHECK (sha256 IS NOT NULL OR redacted_at IS NOT NULL),
    DROP CONSTRAINT source_version_check,
    ADD CONSTRAINT source_version_text_of_kind CHECK (
        kind NOT IN ('member-text', 'web-snapshot', 'review') OR text IS NOT NULL OR redacted_at IS NOT NULL
    ),
    DROP CONSTRAINT source_version_check1,
    ADD CONSTRAINT source_version_snapshot_retrieved CHECK ((kind = 'web-snapshot') = (retrieved_at IS NOT NULL)),
    ADD CONSTRAINT source_version_snapshot_url CHECK (
        (kind = 'web-snapshot') = (url IS NOT NULL) OR redacted_at IS NOT NULL
    );

-- Fact version: the value. An unknown never holds a value.
ALTER TABLE fact_version
    ADD COLUMN redacted_at timestamptz,
    DROP CONSTRAINT fact_version_unknown_has_no_value,
    ADD CONSTRAINT fact_version_unknown_has_no_value CHECK (
        CASE WHEN state = 'unknown' THEN value IS NULL ELSE value IS NOT NULL OR redacted_at IS NOT NULL END
    );

-- The quotes that copy the text of a source version.
ALTER TABLE evidence_link
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN quote DROP NOT NULL,
    ADD CONSTRAINT evidence_link_quote_or_redacted CHECK (quote IS NOT NULL OR redacted_at IS NOT NULL);
ALTER TABLE proposal_evidence
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN quote DROP NOT NULL,
    ADD CONSTRAINT proposal_evidence_quote_or_redacted CHECK (quote IS NOT NULL OR redacted_at IS NOT NULL);
ALTER TABLE record_evidence
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN quote DROP NOT NULL,
    ADD CONSTRAINT record_evidence_quote_or_redacted CHECK (quote IS NOT NULL OR redacted_at IS NOT NULL);

-- Proposal: the values of the operation, the reason, the manifest and the lint warnings.
-- The operation keeps its kind only. The kind of the target, its ID, the event and the expected version stay
-- in their own columns.
ALTER TABLE proposal
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN reason DROP NOT NULL,
    ADD CONSTRAINT proposal_reason_or_redacted CHECK (reason IS NOT NULL OR redacted_at IS NOT NULL),
    DROP CONSTRAINT proposal_draft_provenance,
    ADD CONSTRAINT proposal_draft_provenance CHECK (
        ((target_kind = 'document') = (manifest IS NOT NULL) OR redacted_at IS NOT NULL)
        AND (manifest IS NULL) = (lint_warnings IS NULL)
    );

-- Document and document version: the name, the Markdown, the file name and the hash of the content.
-- The object of an upload is deleted after the redaction; the version keeps its key for the orphan sweep (ADR 0074).
ALTER TABLE document
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN name DROP NOT NULL,
    ADD CONSTRAINT document_name_or_redacted CHECK (name IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT document_redacted_without_content CHECK (redacted_at IS NULL OR name IS NULL);
ALTER TABLE document_version
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN sha256 DROP NOT NULL,
    ADD CONSTRAINT document_version_sha256_or_redacted CHECK (sha256 IS NOT NULL OR redacted_at IS NOT NULL),
    DROP CONSTRAINT document_version_markdown_of_drafts,
    ADD CONSTRAINT document_version_markdown_of_drafts CHECK (
        CASE WHEN kind = 'draft' THEN markdown IS NOT NULL OR redacted_at IS NOT NULL ELSE markdown IS NULL END
    ),
    DROP CONSTRAINT document_version_upload_file,
    ADD CONSTRAINT document_version_upload_file CHECK (
        kind <> 'upload' OR (
            blob_key IS NOT NULL AND media_type IS NOT NULL AND size_bytes IS NOT NULL
            AND (file_name IS NOT NULL OR redacted_at IS NOT NULL) AND source_version_id IS NOT NULL
        )
    );

-- Parties: the name, the email address, the phone number and the account link.
ALTER TABLE person
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN name DROP NOT NULL,
    ADD CONSTRAINT person_name_or_redacted CHECK (name IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT person_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(name, email, phone, user_id) = 0
    );
ALTER TABLE institution
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN name DROP NOT NULL,
    ADD CONSTRAINT institution_name_or_redacted CHECK (name IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT institution_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(name, email, phone) = 0
    );

-- Work records and open questions: their texts. An event redaction redacts them (ADR 0072).
ALTER TABLE open_question
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN text DROP NOT NULL,
    ADD CONSTRAINT open_question_text_or_redacted CHECK (text IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT open_question_redacted_without_content CHECK (redacted_at IS NULL OR text IS NULL);
ALTER TABLE workstream
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN name DROP NOT NULL,
    ADD CONSTRAINT workstream_name_or_redacted CHECK (name IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT workstream_redacted_without_content CHECK (redacted_at IS NULL OR name IS NULL);
ALTER TABLE action
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN title DROP NOT NULL,
    ADD CONSTRAINT action_title_or_redacted CHECK (title IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT action_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(title, description) = 0
    );
ALTER TABLE commitment
    ADD COLUMN redacted_at timestamptz,
    ALTER COLUMN text DROP NOT NULL,
    ADD CONSTRAINT commitment_text_or_redacted CHECK (text IS NOT NULL OR redacted_at IS NOT NULL),
    ADD CONSTRAINT commitment_redacted_without_content CHECK (
        redacted_at IS NULL OR num_nonnulls(text, condition, firm_reason) = 0
    ),
    DROP CONSTRAINT commitment_check1,
    ADD CONSTRAINT commitment_condition_of_conditional CHECK (
        status <> 'conditional' OR condition IS NOT NULL OR redacted_at IS NOT NULL
    );

-- The one UPDATE that a legal redaction makes of an immutable row (ADR 0072).
-- It passes only inside a transaction with `SET LOCAL tada.redaction = 'on'`,
-- only on a row that is not redacted yet, and only when it sets `redacted_at`.
-- Each other column stays unchanged, and each content column becomes NULL: a tombstone is complete or refused.
-- Two content columns cannot be NULL: the author of a source version keeps its actor kind and ID only,
-- and the operation of a proposal keeps its kind only.
-- The search vector of a source version is generated from its text, so it is content but not set by the UPDATE.
-- `content` names the content columns of the table. A table without `redacted_at` never passes.
CREATE FUNCTION redaction_tombstone(old_row jsonb, new_row jsonb, content text[]) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT coalesce(current_setting('tada.redaction', true) = 'on', false)
        AND old_row ? 'redacted_at'
        AND old_row -> 'redacted_at' = 'null'::jsonb
        AND new_row -> 'redacted_at' <> 'null'::jsonb
        AND NOT EXISTS (
            SELECT 1 FROM jsonb_each(new_row) AS n (key, value)
            WHERE n.key <> 'redacted_at'
              AND n.value IS DISTINCT FROM old_row -> n.key
              AND (
                  n.key = ANY (content)
                  AND (
                      n.value = 'null'::jsonb
                      OR (n.key = 'author_actor' AND n.value = jsonb_build_object(
                          'kind', old_row -> n.key -> 'kind', 'id', old_row -> n.key -> 'id'))
                      OR (n.key = 'operation' AND n.value = jsonb_build_object(
                          'kind', old_row -> n.key -> 'kind'))
                  )
              ) IS NOT TRUE
        )
        AND NOT EXISTS (
            SELECT 1 FROM unnest(content) AS c (key)
            WHERE c.key <> 'search'
              AND new_row -> c.key IS DISTINCT FROM CASE c.key
                  WHEN 'author_actor' THEN jsonb_build_object(
                      'kind', old_row -> c.key -> 'kind', 'id', old_row -> c.key -> 'id')
                  WHEN 'operation' THEN jsonb_build_object('kind', old_row -> c.key -> 'kind')
                  ELSE 'null'::jsonb
              END
        )
$$;

-- The rows of these tables never change, except by a legal redaction (ADR 0072).
-- The arguments of a trigger name the content columns of its table; the function passes them on.
CREATE OR REPLACE FUNCTION reject_proposal_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF redaction_tombstone(to_jsonb(OLD), to_jsonb(NEW), TG_ARGV) THEN
            RETURN NEW;
        END IF;
    END IF;
    RAISE EXCEPTION 'the rows of % never change', TG_TABLE_NAME
        USING ERRCODE = 'restrict_violation', CONSTRAINT = 'proposal_immutable';
END;
$$;

-- A source item has no content columns, so its triggers of migration 0017 stay: a redaction sets `redacted_at` only.
-- The search vector of a source version is generated from its text, so it counts as content.
DROP TRIGGER source_version_immutable ON source_version;
CREATE TRIGGER source_version_immutable
    BEFORE UPDATE OR DELETE ON source_version
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('text', 'url', 'sha256', 'author_actor', 'search');
DROP TRIGGER fact_version_immutable ON fact_version;
CREATE TRIGGER fact_version_immutable
    BEFORE UPDATE OR DELETE ON fact_version
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('value');
DROP TRIGGER evidence_link_immutable ON evidence_link;
CREATE TRIGGER evidence_link_immutable
    BEFORE UPDATE OR DELETE ON evidence_link
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('quote');
DROP TRIGGER proposal_immutable ON proposal;
CREATE TRIGGER proposal_immutable
    BEFORE UPDATE OR DELETE ON proposal
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('operation', 'reason', 'manifest', 'lint_warnings');
DROP TRIGGER proposal_evidence_immutable ON proposal_evidence;
CREATE TRIGGER proposal_evidence_immutable
    BEFORE UPDATE OR DELETE ON proposal_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('quote');

-- The evidence of work records never changes either (migration 0024 had no trigger for it).
CREATE TRIGGER record_evidence_immutable
    BEFORE UPDATE OR DELETE ON record_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change('quote');
CREATE TRIGGER record_evidence_no_truncate
    BEFORE TRUNCATE ON record_evidence
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();

-- The content of a document version never changes, except by a legal redaction (ADR 0072).
-- The status and the approval of a draft change as before (migration 0022 guards them).
-- `redacted_at` is part of the content: only a redaction sets it.
CREATE OR REPLACE FUNCTION document_version_content_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF redaction_tombstone(to_jsonb(OLD), to_jsonb(NEW), ARRAY['markdown', 'file_name', 'sha256']) THEN
        RETURN NEW;
    END IF;
    IF (NEW.id, NEW.organization_id, NEW.document_id, NEW.number, NEW.kind, NEW.blob_key,
        NEW.media_type, NEW.size_bytes, NEW.sha256, NEW.file_name, NEW.uploaded_by,
        NEW.source_version_id, NEW.created_at, NEW.markdown, NEW.redacted_at)
       IS DISTINCT FROM
       (OLD.id, OLD.organization_id, OLD.document_id, OLD.number, OLD.kind, OLD.blob_key,
        OLD.media_type, OLD.size_bytes, OLD.sha256, OLD.file_name, OLD.uploaded_by,
        OLD.source_version_id, OLD.created_at, OLD.markdown, OLD.redacted_at)
    THEN
        RAISE EXCEPTION 'the content of a document version never changes'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_content_immutable';
    END IF;
    RETURN NEW;
END;
$$;

-- The record of each legal redaction (ADR 0072). It holds no content and no hash.
-- `actor_id` is the owner who asked for it; the retention job has none.
-- A target is redacted once, so a second redaction of the same target finds this row and does nothing.
CREATE TABLE redaction (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    target_kind text NOT NULL CONSTRAINT redaction_target_kind CHECK (target_kind IN (
        'source-item', 'fact-version', 'document-version', 'proposal', 'person', 'institution', 'event'
    )),
    target_id uuid NOT NULL,
    reason text NOT NULL CONSTRAINT redaction_reason CHECK (reason IN ('request', 'retention')),
    actor_id uuid REFERENCES app_user (id),
    redacted_at timestamptz NOT NULL,
    UNIQUE (organization_id, id),
    CONSTRAINT redaction_target_unique UNIQUE (organization_id, target_kind, target_id)
);

-- A note of an owner that disputes immutable evidence (ADR 0073, Art. 32(3)).
-- The target has no foreign key, because it is one of four tables. An event redaction redacts the text.
CREATE TABLE dispute_note (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organization (id),
    target_kind text NOT NULL CONSTRAINT dispute_note_target_kind CHECK (target_kind IN (
        'source-item', 'fact-version', 'document-version', 'proposal'
    )),
    target_id uuid NOT NULL,
    text text CONSTRAINT dispute_note_text_length CHECK (char_length(text) BETWEEN 1 AND 1000),
    author_id uuid NOT NULL REFERENCES app_user (id),
    created_at timestamptz NOT NULL,
    redacted_at timestamptz,
    UNIQUE (organization_id, id),
    CONSTRAINT dispute_note_text_or_redacted CHECK (text IS NOT NULL OR redacted_at IS NOT NULL),
    CONSTRAINT dispute_note_redacted_without_content CHECK (redacted_at IS NULL OR text IS NULL)
);

CREATE INDEX dispute_note_target ON dispute_note (organization_id, target_kind, target_id);

-- The audit detail can name the reason of a redaction (ADR 0072), next to the role names of migration 0011.
ALTER TABLE audit_event
    DROP CONSTRAINT audit_event_detail_roles,
    ADD CONSTRAINT audit_event_detail_roles CHECK (
        detail IS NULL OR (
            jsonb_typeof(detail) = 'object'
            AND detail <> '{}'::jsonb
            AND detail - 'old_role' - 'new_role' - 'reason' = '{}'::jsonb
            AND coalesce(detail ->> 'old_role', 'member') IN
                ('owner', 'admin', 'member', 'event-manager', 'event-contributor', 'event-viewer')
            AND coalesce(detail ->> 'new_role', 'member') IN
                ('owner', 'admin', 'member', 'event-manager', 'event-contributor', 'event-viewer')
            AND coalesce(detail ->> 'reason', 'request') IN ('request', 'retention')
        )
    ),
    ADD CONSTRAINT audit_event_detail_reason_string CHECK (
        detail IS NULL OR coalesce(jsonb_typeof(detail -> 'reason'), 'string') = 'string'
    );

-- An account without membership starts the 90 days of its anonymization at this migration (ADR 0071).
-- The accounts of Slice 1 and 2a could lose their last membership before the column existed.
UPDATE app_user SET membership_ended_at = now()
WHERE anonymized_at IS NULL AND membership_ended_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM organization_membership m WHERE m.user_id = app_user.id);
