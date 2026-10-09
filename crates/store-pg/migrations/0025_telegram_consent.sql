-- The Telegram account accepts a claimed link code in the chat, after the bot named the tada account
-- of the code. Only then can the member confirm the link in the web client. Without this step, a
-- member could send a victim the own code and link the Telegram account of the victim.

ALTER TABLE telegram_link_code ADD COLUMN accepted_at timestamptz;

-- A link that the member confirmed before this migration had the consent of the old flow.
UPDATE telegram_link_code SET accepted_at = claimed_at WHERE confirmed_at IS NOT NULL;

ALTER TABLE telegram_link_code
    ADD CONSTRAINT telegram_link_code_accepted_check CHECK (accepted_at IS NULL OR claimed_at IS NOT NULL),
    ADD CONSTRAINT telegram_link_code_confirmed_check CHECK (confirmed_at IS NULL OR accepted_at IS NOT NULL);
