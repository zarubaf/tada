-- Documents and their versions are never deleted (ADR 0009, ADR 0051), expand only.

-- A deleted document version removes the record of its evidence, while its object and its source version stay.
-- The application never deletes a document or a version; these triggers are the second line of defense.
-- Updates stay possible: the record version of a document counts up, and a draft changes its status
-- (migration 0014 guards the content of a version).
-- They reuse the function of the proposal tables. A legal redaction (ADR 0045) must name how it passes them,
-- as for the proposal tables.
CREATE TRIGGER document_no_delete
    BEFORE DELETE ON document
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER document_no_truncate
    BEFORE TRUNCATE ON document
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER document_version_no_delete
    BEFORE DELETE ON document_version
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER document_version_no_truncate
    BEFORE TRUNCATE ON document_version
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
