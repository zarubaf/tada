-- The data of Slice 2a at the schema of migration 0025, for the schema upgrade test (ADR 0006).
-- All data is invented.
-- It adds to the fixture of 0015: load that one first.
-- It holds an open question, parties, a workstream, an action, a commitment and the evidence of the records.
--
-- The IDs end in a number that tells the kind of record, after the numbers of the fixture of 0015:
-- 010x open questions, 011x persons and institutions, 012x workstreams, actions and commitments,
-- 013x record evidence.

INSERT INTO local_id_counter (organization_id, scope_id, kind, next) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000003', 'QST', 2),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000001', 'PER', 2),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000001', 'INS', 2),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000003', 'ACT', 2),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000003', 'COM', 2);

INSERT INTO open_question
    (id, organization_id, event_id, local_number, text, owner_user_id, status, version, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000101', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 1, 'Wer organisiert die Festwirtschaft?',
     '0190a000-0000-7000-8000-000000000002', 'open', 1, '2030-05-06T08:00:00Z');

INSERT INTO person
    (id, organization_id, local_number, name, email, phone, user_id, version, created_at, updated_at) VALUES
    ('0190a000-0000-7000-8000-000000000111', '0190a000-0000-7000-8000-000000000001', 1, 'Max Muster',
     'max@example.org', '+41 00 000 00 00', NULL, 1, '2030-05-06T08:00:00Z', '2030-05-06T08:00:00Z');
INSERT INTO institution
    (id, organization_id, local_number, name, kind, email, phone, version, created_at, updated_at) VALUES
    ('0190a000-0000-7000-8000-000000000112', '0190a000-0000-7000-8000-000000000001', 1, 'Gemeinde Testwil',
     'authority', NULL, NULL, 1, '2030-05-06T08:00:00Z', '2030-05-06T08:00:00Z');

INSERT INTO workstream
    (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at) VALUES
    ('0190a000-0000-7000-8000-000000000121', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 'Festwirtschaft', '0190a000-0000-7000-8000-000000000002',
     'active', 1, '2030-05-06T08:00:00Z', '2030-05-06T08:00:00Z');
INSERT INTO action
    (id, organization_id, event_id, local_number, title, description, owner_user_id, workstream_id, due_date,
     status, version, created_at, updated_at) VALUES
    ('0190a000-0000-7000-8000-000000000122', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 1, 'Zelt reservieren', 'Bei der Gemeinde anfragen.',
     '0190a000-0000-7000-8000-000000000002', '0190a000-0000-7000-8000-000000000121', '2030-06-01',
     'open', 1, '2030-05-06T08:00:00Z', '2030-05-06T08:00:00Z');
INSERT INTO commitment
    (id, organization_id, event_id, local_number, text, condition, person_id, institution_id, owner_user_id,
     workstream_id, due_date, status, firm_reason, version, created_at, updated_at) VALUES
    ('0190a000-0000-7000-8000-000000000123', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 1, 'Bringt die Tische', 'Wenn das Wetter gut ist',
     '0190a000-0000-7000-8000-000000000111', NULL, '0190a000-0000-7000-8000-000000000002',
     '0190a000-0000-7000-8000-000000000121', NULL, 'conditional', NULL, 1,
     '2030-05-06T08:00:00Z', '2030-05-06T08:00:00Z');

-- The evidence of the commitment cites the first member text through the proposal of the first motto.
INSERT INTO record_evidence
    (id, organization_id, action_id, commitment_id, person_id, institution_id, record_version, proposal_id,
     source_version_id, start_offset, end_offset, quote, page) VALUES
    ('0190a000-0000-7000-8000-000000000131', '0190a000-0000-7000-8000-000000000001',
     NULL, '0190a000-0000-7000-8000-000000000123', NULL, NULL, 1, '0190a000-0000-7000-8000-000000000042',
     '0190a000-0000-7000-8000-000000000012', 35, 52, 'Flieg mit Testwil', NULL);
