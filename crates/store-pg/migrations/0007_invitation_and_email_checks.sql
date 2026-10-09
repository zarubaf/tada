-- States that the code never means are not possible (ADRs 0008 and 0056).

-- An invitation has the timestamp of its status, and no other one.
ALTER TABLE invitation
    ADD CONSTRAINT invitation_accepted_at_check CHECK ((status = 'accepted') = (accepted_at IS NOT NULL)),
    ADD CONSTRAINT invitation_revoked_at_check CHECK ((status = 'revoked') = (revoked_at IS NOT NULL));

-- `Email` trims the address and lowercases it. The join of an invitation to a user compares the
-- addresses as text, so an address that is not normalized would miss its user silently.
ALTER TABLE invitation
    ADD CONSTRAINT invitation_email_check CHECK (email = lower(btrim(email)));
ALTER TABLE email_identity
    ADD CONSTRAINT email_identity_email_check CHECK (email = lower(btrim(email)));
