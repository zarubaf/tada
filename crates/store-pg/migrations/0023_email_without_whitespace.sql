-- An email address has no whitespace inside it. `Email` refuses such an address,
-- and the CHECKs of migration 0007 trim only the ends.
--
-- The constraints are `NOT VALID`: PostgreSQL checks each new and each changed row, but not the existing rows.
-- Before this change, `Email` accepted an address with a space inside, so an existing row can break the rule.
-- A validation of such a row would stop this migration and the upgrade.
ALTER TABLE invitation
    ADD CONSTRAINT invitation_email_whitespace_check CHECK (email !~ '[[:space:]]') NOT VALID;
ALTER TABLE email_identity
    ADD CONSTRAINT email_identity_email_whitespace_check CHECK (email !~ '[[:space:]]') NOT VALID;
