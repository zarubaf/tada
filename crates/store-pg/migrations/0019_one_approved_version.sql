-- A document has at most one approved version (ADR 0051).
-- The approval locks the document row and supersedes the approved version first; this index is the second line of defense.
CREATE UNIQUE INDEX document_version_one_approved
    ON document_version (organization_id, document_id)
    WHERE status = 'approved';
