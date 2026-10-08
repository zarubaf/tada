-- The privacy notice of an organization (ADR 0045).
-- A NULL text means that the template of the web client applies.
-- The version starts at 1 and grows with each change of the text (ADR 0006).
ALTER TABLE organization
    ADD COLUMN privacy_notice text CHECK (char_length(privacy_notice) BETWEEN 1 AND 20000),
    ADD COLUMN privacy_notice_version bigint NOT NULL DEFAULT 1 CHECK (privacy_notice_version >= 1);
