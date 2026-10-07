-- The counters of the rate limits (ADR 0008, ADR 0056). All `serve` processes share them (ADR 0025).
-- The key is an HMAC-SHA-256 of a normalized email address or of an IP address, never the address.
-- A counter counts the requests of one key in one fixed window. Each sign-in request deletes the
-- counters of the windows that ended.

CREATE TABLE rate_limit_counter (
    key bytea NOT NULL CHECK (octet_length(key) = 32),
    window_start timestamptz NOT NULL,
    count integer NOT NULL CHECK (count > 0),
    PRIMARY KEY (key, window_start)
);

-- The delete of the ended windows.
CREATE INDEX rate_limit_counter_window_start ON rate_limit_counter (window_start);
