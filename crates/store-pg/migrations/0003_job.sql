-- The job queue (ADRs 0007 and 0054). A command adds its jobs in its own transaction.
-- A completed job is removed. A job that failed its last attempt stays, with failed_at, for the operator.
CREATE TABLE job (
    id uuid PRIMARY KEY,
    organization_id uuid REFERENCES organization (id),
    kind text NOT NULL,
    version integer NOT NULL,
    payload jsonb NOT NULL,
    request_id uuid,
    run_at timestamptz NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    max_attempts integer NOT NULL DEFAULT 10 CHECK (max_attempts >= 1),
    locked_by uuid,
    locked_until timestamptz,
    last_error text,
    failed_at timestamptz,
    created_at timestamptz NOT NULL
);

CREATE INDEX job_due ON job (run_at, id) WHERE failed_at IS NULL;

-- The heartbeat also shows the queue (ADR 0035).
ALTER TABLE worker_heartbeat
    ADD COLUMN queue_depth bigint NOT NULL DEFAULT 0,
    ADD COLUMN oldest_due_seconds bigint;
