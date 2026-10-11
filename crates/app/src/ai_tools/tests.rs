use std::collections::BTreeSet;

use super::*;

const READ_TOOLS: [&str; 11] = [
    "list_events",
    "get_event_schema",
    "get_event_profile",
    "search_sources",
    "get_source_passage",
    "list_documents",
    "get_document_version",
    "list_workstreams",
    "list_actions",
    "list_commitments",
    "search_parties",
];

fn names<'a>(tools: impl Iterator<Item = &'a dyn AiTool>) -> Vec<&'static str> {
    tools.map(AiTool::name).collect()
}

#[test]
fn the_standard_catalog_has_each_tool_once_and_only_the_proposal_tool_writes() {
    let catalog = ToolCatalog::standard();
    let all = names(catalog.all());
    assert_eq!(all.iter().collect::<BTreeSet<_>>().len(), all.len());
    assert_eq!(names(catalog.read_only()), READ_TOOLS);
    let writes: Vec<_> = catalog.all().filter(|tool| !tool.read_only()).collect();
    assert_eq!(names(writes.into_iter()), ["propose_changeset"]);
}

#[test]
fn each_input_schema_is_an_object_without_the_name_of_its_rust_type() {
    for tool in ToolCatalog::standard().all() {
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

#[test]
fn only_keeps_the_named_tools() {
    let catalog = ToolCatalog::standard().only(&["list_actions", "get_event_profile"]);
    assert_eq!(names(catalog.all()), ["get_event_profile", "list_actions"]);
    assert!(catalog.get("propose_changeset").is_none());
    assert_eq!(
        catalog.get("list_actions").map(AiTool::name),
        Some("list_actions")
    );
}

#[test]
#[should_panic(expected = "the catalog has no tool list_everything")]
fn only_rejects_an_unknown_name() {
    let _ = ToolCatalog::standard().only(&["list_everything"]);
}

#[test]
fn an_error_of_a_query_keeps_its_code_its_invalid_values_and_its_store_failure() {
    let error = AiToolError::invalid("event_key", "pattern");
    assert_eq!(error.code(), ProblemCode::ValidationFailed);
    assert_eq!(
        error.field_errors(),
        [FieldError::new("event_key", "pattern")]
    );
    assert!(error.store_error().is_none());
    let error = AiToolError::from(StoreError::Unavailable("down".into()));
    assert_eq!(error.code(), ProblemCode::Unavailable);
    assert!(error.store_error().is_some());
}
