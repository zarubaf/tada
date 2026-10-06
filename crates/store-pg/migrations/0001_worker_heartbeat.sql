-- One row for each running worker process (ADR 0025, ADR 0035).
-- An operator check reads last_loop_at. The worker removes its row when it stops cleanly.
CREATE TABLE worker_heartbeat (
    worker_id uuid PRIMARY KEY,
    started_at timestamptz NOT NULL,
    last_loop_at timestamptz NOT NULL
);
