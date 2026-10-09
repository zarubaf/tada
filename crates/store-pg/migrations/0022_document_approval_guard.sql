-- The approval of a document version is a record that never changes (ADR 0051).
-- Migration 0015 guards the content of a version; this trigger guards its status and its approval.
-- The application only approves a draft or a version in review, and supersedes the approved version.
-- This trigger is the second line of defense, as for the content.
-- An upload has no status and no approval, so no update of an upload passes a change of them.
--
-- Allowed changes:
-- - `draft` or `review` to `approved`, with `approved_by` and `approved_at` set in the same update.
-- - `approved` to `superseded`, with the approval unchanged.
-- Once `approved_at` is set, `approved_by` and `approved_at` never change.
-- A move into `review` or `archived` needs a new migration when the application gets one.
CREATE FUNCTION document_version_approval_record() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    approves boolean := coalesce(OLD.status IN ('draft', 'review') AND NEW.status = 'approved', false);
    supersedes boolean := coalesce(OLD.status = 'approved' AND NEW.status = 'superseded', false);
    approval_changed boolean := (NEW.approved_by, NEW.approved_at) IS DISTINCT FROM (OLD.approved_by, OLD.approved_at);
BEGIN
    IF approval_changed AND NOT (approves AND OLD.approved_at IS NULL) THEN
        RAISE EXCEPTION 'the approval of a document version never changes'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_approval_record';
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status
       AND NOT ((approves AND NEW.approved_at IS NOT NULL) OR supersedes) THEN
        RAISE EXCEPTION 'this status change of a document version is not allowed'
            USING ERRCODE = 'restrict_violation', CONSTRAINT = 'document_version_approval_record';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER document_version_approval_record
    BEFORE UPDATE ON document_version
    FOR EACH ROW EXECUTE FUNCTION document_version_approval_record();
