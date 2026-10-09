-- The approval of a document version is a record that never changes (ADR 0051).
-- Migration 0015 guards the content of a version; this trigger guards its status and its approval.
-- The application only approves a draft or a version in review, and supersedes the approved version.
-- This trigger is the second line of defense, as for the content.
-- An upload has no status and no approval, so no update of an upload passes a change of them.
--
-- Allowed status changes (ARCHITECTURE.md: an edit makes a new version, so no version goes back to draft or review):
-- - `draft` to `review`.
-- - `draft` or `review` to `approved`, with `approved_by` and `approved_at` set in the same update.
--   This is the only change that sets the approval.
-- - `approved` to `superseded`.
-- - `draft`, `review`, `approved` or `superseded` to `archived`.
-- Nothing leaves `archived`.
-- Once `approved_at` is set, `approved_by` and `approved_at` never change.
CREATE FUNCTION document_version_approval_record() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    approves boolean := coalesce(OLD.status IN ('draft', 'review') AND NEW.status = 'approved', false);
    allowed boolean := coalesce(
        (OLD.status = 'draft' AND NEW.status = 'review')
        OR (approves AND NEW.approved_at IS NOT NULL)
        OR (OLD.status = 'approved' AND NEW.status = 'superseded')
        OR (OLD.status IN ('draft', 'review', 'approved', 'superseded') AND NEW.status = 'archived'),
        false);
    approval_changed boolean := (NEW.approved_by, NEW.approved_at) IS DISTINCT FROM (OLD.approved_by, OLD.approved_at);
BEGIN
    IF approval_changed AND NOT (approves AND OLD.approved_at IS NULL) THEN
        RAISE EXCEPTION 'the approval of a document version never changes'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_approval_record';
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status AND NOT allowed THEN
        RAISE EXCEPTION 'this status change of a document version is not allowed'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_approval_record';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER document_version_approval_record
    BEFORE UPDATE ON document_version
    FOR EACH ROW EXECUTE FUNCTION document_version_approval_record();
