-- The data of one organization at the schema of migration 0015, for the schema upgrade test (ADR 0006).
-- All data is invented.
-- It holds a concept draft with its provenance manifest, the sources, the relationships between the records,
-- and old versions of a fact, of an upload and of the concept.
-- It follows the stored formats of the store-pg codecs at that schema: the actor, the operation and the manifest.
--
-- The IDs end in a number that tells the kind of record:
-- 0001 organization, 0002 user, 0003 event, 001x sources, 0021 field, 003x changesets, 004x proposals,
-- 005x fact and fact versions, 006x evidence links, 007x proposal evidence, 008x review results, 009x documents.

INSERT INTO organization (id, slug, name, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000001', 'testwil', 'Flugplatzverein Testwil', '2030-05-01T08:00:00Z');

INSERT INTO app_user (id, display_name, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000002', 'Anna Muster', '2030-05-01T08:00:00Z');
INSERT INTO email_identity (user_id, email, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000002', 'anna@example.org', '2030-05-01T08:00:00Z');
INSERT INTO organization_membership (organization_id, user_id, role, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000002', 'owner', '2030-05-01T08:00:00Z');

INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000003', '0190a000-0000-7000-8000-000000000001', 'OPENDAY',
     'Open Day Testwil', 'Europe/Zurich', 1, '2030-05-01T08:00:00Z');
INSERT INTO event_membership (organization_id, event_id, user_id, event_role, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000003',
     '0190a000-0000-7000-8000-000000000002', 'event-manager', '2030-05-01T08:00:00Z');

-- A member text in two versions, and an upload in two versions.
INSERT INTO source_item (id, organization_id, event_id, kind, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000011', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 'member-text', '2030-05-02T08:00:00Z'),
    ('0190a000-0000-7000-8000-000000000014', '0190a000-0000-7000-8000-000000000001',
     NULL, 'upload', '2030-05-02T09:00:00Z');
INSERT INTO source_version (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at) VALUES
    ('0190a000-0000-7000-8000-000000000012', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000011', 'member-text', 'web',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     'Das Motto des Open Day Testwil ist Flieg mit Testwil.',
     sha256(convert_to('Das Motto des Open Day Testwil ist Flieg mit Testwil.', 'UTF8')), '2030-05-02T08:00:00Z'),
    ('0190a000-0000-7000-8000-000000000013', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000011', 'member-text', 'web',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     'Neu: Das Motto ist Flieg mit uns.',
     sha256(convert_to('Neu: Das Motto ist Flieg mit uns.', 'UTF8')), '2030-05-03T08:00:00Z'),
    ('0190a000-0000-7000-8000-000000000015', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000014', 'upload', 'web',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     NULL, sha256(convert_to('flyer version 1', 'UTF8')), '2030-05-02T09:00:00Z'),
    ('0190a000-0000-7000-8000-000000000016', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000014', 'upload', 'web',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     NULL, sha256(convert_to('flyer version 2', 'UTF8')), '2030-05-03T09:00:00Z');

-- A field of the event.
INSERT INTO field_definition
    (id, organization_id, event_id, key, label_text, value_type, description, module, status, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000021', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 'motto', 'Motto', '{"type": "text"}',
     'Das Motto des Anlasses.', 'core', 'active', '2030-05-02T10:00:00Z');

-- Four changesets: the field and the first motto, the new motto, the concept, and the new concept.
INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at) VALUES
    ('0190a000-0000-7000-8000-000000000031', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000012', '2030-05-02T08:00:00Z'),
    ('0190a000-0000-7000-8000-000000000032', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000013', '2030-05-03T08:00:00Z'),
    ('0190a000-0000-7000-8000-000000000033', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000012', '2030-05-02T11:00:00Z'),
    ('0190a000-0000-7000-8000-000000000034', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000013', '2030-05-03T11:00:00Z');

INSERT INTO proposal
    (id, organization_id, changeset_id, event_id, operation, operation_version, target_kind, target_id,
     expected_version, reason, created_at, manifest, lint_warnings) VALUES
    ('0190a000-0000-7000-8000-000000000041', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000031', '0190a000-0000-7000-8000-000000000003',
     '{"kind": "add_field_definition", "id": "0190a000-0000-7000-8000-000000000021",
       "event_id": "0190a000-0000-7000-8000-000000000003", "key": "motto", "label": "Motto",
       "value_type": {"type": "text"}, "description": "Das Motto des Anlasses.", "module": "core"}',
     1, 'field_definition', '0190a000-0000-7000-8000-000000000021', NULL,
     'Die Quelle nennt ein Motto.', '2030-05-02T08:00:00Z', NULL, NULL),
    ('0190a000-0000-7000-8000-000000000042', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000031', '0190a000-0000-7000-8000-000000000003',
     '{"kind": "set_fact", "event_id": "0190a000-0000-7000-8000-000000000003",
       "field_id": "0190a000-0000-7000-8000-000000000021", "state": "accepted",
       "value": {"type": "text", "text": "Flieg mit Testwil"}, "approximate": false, "expected_version": null}',
     1, 'fact', '0190a000-0000-7000-8000-000000000021', NULL,
     'Die Quelle nennt das Motto.', '2030-05-02T08:00:00Z', NULL, NULL),
    ('0190a000-0000-7000-8000-000000000043', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000032', '0190a000-0000-7000-8000-000000000003',
     '{"kind": "set_fact", "event_id": "0190a000-0000-7000-8000-000000000003",
       "field_id": "0190a000-0000-7000-8000-000000000021", "state": "accepted",
       "value": {"type": "text", "text": "Flieg mit uns"}, "approximate": false, "expected_version": 1}',
     1, 'fact', '0190a000-0000-7000-8000-000000000021', 1,
     'Die neue Quelle nennt ein neues Motto.', '2030-05-03T08:00:00Z', NULL, NULL),
    ('0190a000-0000-7000-8000-000000000044', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000033', '0190a000-0000-7000-8000-000000000003',
     '{"kind": "create_document_draft", "event_id": "0190a000-0000-7000-8000-000000000003",
       "document": {"kind": "new", "id": "0190a000-0000-7000-8000-000000000094", "name": "Vorläufiges Konzept"},
       "markdown": "# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=1).\nDer Anlass heisst [Open Day Testwil](tada:source/0190a000-0000-7000-8000-000000000012#14-30).\n"}',
     1, 'document', '0190a000-0000-7000-8000-000000000094', NULL,
     'Ein erster Entwurf des Konzepts.', '2030-05-02T11:00:00Z',
     '{"facts": [{"fact_id": "0190a000-0000-7000-8000-000000000051", "version": 1}],
       "sources": [{"source_version_id": "0190a000-0000-7000-8000-000000000012", "start": 14, "end": 30, "quote": "Open Day Testwil"}]}',
     '[]'),
    ('0190a000-0000-7000-8000-000000000045', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000034', '0190a000-0000-7000-8000-000000000003',
     '{"kind": "create_document_draft", "event_id": "0190a000-0000-7000-8000-000000000003",
       "document": {"kind": "existing", "document_id": "0190a000-0000-7000-8000-000000000094", "expected_version": 1},
       "markdown": "# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=2).\nDie Quelle sagt [Flieg mit uns](tada:source/0190a000-0000-7000-8000-000000000013#19-32).\n"}',
     1, 'document', '0190a000-0000-7000-8000-000000000094', 1,
     'Das Konzept nennt das neue Motto.', '2030-05-03T11:00:00Z',
     '{"facts": [{"fact_id": "0190a000-0000-7000-8000-000000000051", "version": 2}],
       "sources": [{"source_version_id": "0190a000-0000-7000-8000-000000000013", "start": 19, "end": 32, "quote": "Flieg mit uns"}]}',
     '[]');

-- The motto needs its field.
INSERT INTO proposal_dependency (organization_id, changeset_id, proposal_id, depends_on) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000031',
     '0190a000-0000-7000-8000-000000000042', '0190a000-0000-7000-8000-000000000041');

INSERT INTO proposal_evidence
    (id, organization_id, proposal_id, source_version_id, start_offset, end_offset, quote) VALUES
    ('0190a000-0000-7000-8000-000000000071', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000041', '0190a000-0000-7000-8000-000000000012', 35, 52, 'Flieg mit Testwil'),
    ('0190a000-0000-7000-8000-000000000072', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000042', '0190a000-0000-7000-8000-000000000012', 35, 52, 'Flieg mit Testwil'),
    ('0190a000-0000-7000-8000-000000000073', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000043', '0190a000-0000-7000-8000-000000000013', 19, 32, 'Flieg mit uns'),
    ('0190a000-0000-7000-8000-000000000074', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000044', '0190a000-0000-7000-8000-000000000012', 14, 30, 'Open Day Testwil'),
    ('0190a000-0000-7000-8000-000000000075', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000045', '0190a000-0000-7000-8000-000000000013', 19, 32, 'Flieg mit uns');

INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
SELECT
    ('0190a000-0000-7000-8000-00000000008' || n)::uuid,
    '0190a000-0000-7000-8000-000000000001',
    ('0190a000-0000-7000-8000-00000000004' || n)::uuid,
    'accepted',
    '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
    '2030-05-04T08:00:00Z'::timestamptz + n * interval '1 minute'
FROM generate_series(1, 5) AS n;

-- The motto: version 1 is old, version 2 is current.
INSERT INTO fact (id, organization_id, event_id, field_id, version) VALUES
    ('0190a000-0000-7000-8000-000000000051', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', '0190a000-0000-7000-8000-000000000021', 2);
INSERT INTO fact_version
    (id, organization_id, fact_id, number, state, value, approximate, created_at, accepted_by, proposal_id) VALUES
    ('0190a000-0000-7000-8000-000000000052', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000051', 1, 'accepted', '{"type": "text", "text": "Flieg mit Testwil"}', false,
     '2030-05-04T08:02:00Z',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000042'),
    ('0190a000-0000-7000-8000-000000000053', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000051', 2, 'accepted', '{"type": "text", "text": "Flieg mit uns"}', false,
     '2030-05-04T08:03:00Z',
     '{"kind": "member", "id": "0190a000-0000-7000-8000-000000000002", "principal": null, "channel": "web", "request_id": null}',
     '0190a000-0000-7000-8000-000000000043');
INSERT INTO evidence_link
    (id, organization_id, fact_version_id, source_version_id, start_offset, end_offset, quote) VALUES
    ('0190a000-0000-7000-8000-000000000061', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000052', '0190a000-0000-7000-8000-000000000012', 35, 52, 'Flieg mit Testwil'),
    ('0190a000-0000-7000-8000-000000000062', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000053', '0190a000-0000-7000-8000-000000000013', 19, 32, 'Flieg mit uns');

-- DOC-001 is an upload in two versions. DOC-002 is the concept: version 1 is superseded, version 2 is approved.
INSERT INTO local_id_counter (organization_id, scope_id, kind, next) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000001', 'DOC', 3);
INSERT INTO document (id, organization_id, event_id, local_number, name, owner_user_id, created_at, version) VALUES
    ('0190a000-0000-7000-8000-000000000091', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 1, 'Flyer', '0190a000-0000-7000-8000-000000000002',
     '2030-05-02T09:00:00Z', 2),
    ('0190a000-0000-7000-8000-000000000094', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000003', 2, 'Vorläufiges Konzept', '0190a000-0000-7000-8000-000000000002',
     '2030-05-04T08:04:00Z', 2);
INSERT INTO document_version
    (id, organization_id, document_id, number, kind, blob_key, media_type, size_bytes, sha256, file_name,
     uploaded_by, source_version_id, status, created_at, approved_by, approved_at, markdown) VALUES
    ('0190a000-0000-7000-8000-000000000092', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000091', 1, 'upload',
     '0190a000-0000-7000-8000-000000000001/0190a000-0000-7000-8000-000000000192', 'application/pdf', 15,
     sha256(convert_to('flyer version 1', 'UTF8')), 'flyer.pdf', '0190a000-0000-7000-8000-000000000002',
     '0190a000-0000-7000-8000-000000000015', NULL, '2030-05-02T09:00:00Z', NULL, NULL, NULL),
    ('0190a000-0000-7000-8000-000000000093', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000091', 2, 'upload',
     '0190a000-0000-7000-8000-000000000001/0190a000-0000-7000-8000-000000000193', 'application/pdf', 15,
     sha256(convert_to('flyer version 2', 'UTF8')), 'flyer.pdf', '0190a000-0000-7000-8000-000000000002',
     '0190a000-0000-7000-8000-000000000016', NULL, '2030-05-03T09:00:00Z', NULL, NULL, NULL),
    ('0190a000-0000-7000-8000-000000000095', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000094', 1, 'draft', NULL, NULL, NULL,
     sha256(convert_to(E'# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=1).\nDer Anlass heisst [Open Day Testwil](tada:source/0190a000-0000-7000-8000-000000000012#14-30).\n', 'UTF8')),
     NULL, '0190a000-0000-7000-8000-000000000002', NULL, 'superseded', '2030-05-04T08:04:00Z',
     '0190a000-0000-7000-8000-000000000002', '2030-05-04T09:00:00Z',
     E'# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=1).\nDer Anlass heisst [Open Day Testwil](tada:source/0190a000-0000-7000-8000-000000000012#14-30).\n'),
    ('0190a000-0000-7000-8000-000000000096', '0190a000-0000-7000-8000-000000000001',
     '0190a000-0000-7000-8000-000000000094', 2, 'draft', NULL, NULL, NULL,
     sha256(convert_to(E'# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=2).\nDie Quelle sagt [Flieg mit uns](tada:source/0190a000-0000-7000-8000-000000000013#19-32).\n', 'UTF8')),
     NULL, '0190a000-0000-7000-8000-000000000002', NULL, 'approved', '2030-05-04T08:05:00Z',
     '0190a000-0000-7000-8000-000000000002', '2030-05-05T08:00:00Z',
     E'# Vorläufiges Konzept\n\nDas Motto ist [](tada:fact/0190a000-0000-7000-8000-000000000051?v=2).\nDie Quelle sagt [Flieg mit uns](tada:source/0190a000-0000-7000-8000-000000000013#19-32).\n');

-- The manifest of each concept version: the old version cites the old motto and the old source version.
INSERT INTO document_manifest_fact (organization_id, document_version_id, fact_id, fact_version_number) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000095',
     '0190a000-0000-7000-8000-000000000051', 1),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000096',
     '0190a000-0000-7000-8000-000000000051', 2);
INSERT INTO document_manifest_source
    (organization_id, document_version_id, source_version_id, start_offset, end_offset) VALUES
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000095',
     '0190a000-0000-7000-8000-000000000012', 14, 30),
    ('0190a000-0000-7000-8000-000000000001', '0190a000-0000-7000-8000-000000000096',
     '0190a000-0000-7000-8000-000000000013', 19, 32);
