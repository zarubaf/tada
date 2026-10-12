-- A changeset whose proposals cite only stored sources contains no words of the member (ADR 0079).
-- tada then stores no source version for it, and `source_version_id` is NULL.
-- A release before this change cannot read a changeset without a source version. The gap lasts only
-- until all roles run the new release, and ADR 0079 accepts it.

ALTER TABLE changeset ALTER COLUMN source_version_id DROP NOT NULL;
