-- Guards for sources, fact versions and evidence, and a stricter audit detail (expand only).

-- Source items, source versions, fact versions and evidence links never change (ADR 0009, ADR 0050).
-- The application never updates or deletes them; these triggers are the second line of defense.
-- They reuse the function of the proposal tables. A legal redaction (ADR 0045) must name how it passes them,
-- as for the proposal tables.
CREATE TRIGGER source_item_immutable
    BEFORE UPDATE OR DELETE ON source_item
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER source_item_no_truncate
    BEFORE TRUNCATE ON source_item
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER source_version_immutable
    BEFORE UPDATE OR DELETE ON source_version
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER source_version_no_truncate
    BEFORE TRUNCATE ON source_version
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER fact_version_immutable
    BEFORE UPDATE OR DELETE ON fact_version
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER fact_version_no_truncate
    BEFORE TRUNCATE ON fact_version
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER evidence_link_immutable
    BEFORE UPDATE OR DELETE ON evidence_link
    FOR EACH ROW EXECUTE FUNCTION reject_proposal_change();
CREATE TRIGGER evidence_link_no_truncate
    BEFORE TRUNCATE ON evidence_link
    FOR EACH STATEMENT EXECUTE FUNCTION reject_proposal_change();

-- The proposal that created a fact version is a proposal of the same organization.
ALTER TABLE fact_version
    ADD CONSTRAINT fact_version_proposal
    FOREIGN KEY (organization_id, proposal_id) REFERENCES proposal (organization_id, id);

-- The current version of a fact exists. The apply writes the fact before its version,
-- so the check waits for the commit.
ALTER TABLE fact
    ADD CONSTRAINT fact_current_version
    FOREIGN KEY (id, version) REFERENCES fact_version (fact_id, number) DEFERRABLE INITIALLY DEFERRED;

-- The access rule of sources and the evidence reads look up the evidence of a source version.
CREATE INDEX evidence_link_source_version ON evidence_link (organization_id, source_version_id);
CREATE INDEX proposal_evidence_source_version ON proposal_evidence (organization_id, source_version_id);

-- A role in the audit detail is a string. `->>` reads a JSON null as SQL NULL, which the role check of
-- migration 0011 lets pass (ADR 0061).
ALTER TABLE audit_event
    ADD CONSTRAINT audit_event_detail_role_strings CHECK (
        detail IS NULL OR (
            coalesce(jsonb_typeof(detail -> 'old_role'), 'string') = 'string'
            AND coalesce(jsonb_typeof(detail -> 'new_role'), 'string') = 'string'
        )
    );
