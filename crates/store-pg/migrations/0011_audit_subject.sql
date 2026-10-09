-- The member whom an audit event is about, and the role change, if any (ADR 0061).
-- `detail` holds role names only: no personal data and no free text (ADR 0039, ADR 0045).
-- `subject_user_id` has no foreign key, like `actor_id`: the log outlives users (ADR 0045).
-- This migration touches only `audit_event`.
ALTER TABLE audit_event
    ADD COLUMN subject_user_id uuid,
    ADD COLUMN detail jsonb,
    ADD CONSTRAINT audit_event_detail_roles CHECK (
        detail IS NULL OR (
            jsonb_typeof(detail) = 'object'
            AND detail <> '{}'::jsonb
            AND detail - 'old_role' - 'new_role' = '{}'::jsonb
            AND coalesce(detail ->> 'old_role', 'member') IN
                ('owner', 'admin', 'member', 'event-manager', 'event-contributor', 'event-viewer')
            AND coalesce(detail ->> 'new_role', 'member') IN
                ('owner', 'admin', 'member', 'event-manager', 'event-contributor', 'event-viewer')
        )
    ),
    ADD CONSTRAINT audit_event_actor_kind CHECK (actor_kind IN ('member', 'service', 'ai')),
    ADD CONSTRAINT audit_event_channel CHECK (channel IN ('web', 'telegram', 'job', 'api-token', 'cli'));

-- Audit queries read the log of one organization in time order, or the events about one member.
CREATE INDEX audit_event_organization_time ON audit_event (organization_id, occurred_at);
CREATE INDEX audit_event_subject ON audit_event (organization_id, subject_user_id)
    WHERE subject_user_id IS NOT NULL;
