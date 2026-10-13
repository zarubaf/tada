use super::*;

const READ_TOOLS: [&str; 14] = [
    "list_events",
    "list_members",
    "get_event_schema",
    "get_event_profile",
    "search_sources",
    "get_source_passage",
    "list_documents",
    "get_document_version",
    "list_workstreams",
    "list_actions",
    "list_commitments",
    "list_milestones",
    "list_decisions",
    "search_parties",
];

fn names<'a>(tools: impl Iterator<Item = &'a Arc<dyn AiTool<AiToolPorts>>>) -> Vec<&'static str> {
    tools.map(|tool| tool.name()).collect()
}

#[test]
fn only_the_proposal_tool_writes() {
    let catalog = standard_catalog();
    assert_eq!(names(catalog.read_only()), READ_TOOLS);
    let writes = catalog.all().filter(|tool| !tool.read_only());
    assert_eq!(names(writes), ["propose_changeset"]);
}

#[test]
fn each_input_schema_is_an_object_without_the_name_of_its_rust_type() {
    for tool in standard_catalog().all() {
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object", "{}", tool.name());
        assert!(schema.get("title").is_none(), "{}", tool.name());
        assert!(
            tool.output_schema().get("title").is_none(),
            "{}",
            tool.name()
        );
    }
}
