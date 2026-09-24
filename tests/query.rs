use std::fs;

use mara::{RelatedFilters, Template, initialize_project, load_corpus, load_schema, related_items};
use tempfile::TempDir;

#[test]
fn item_only_related_items_skips_external_targets() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap()
        + "\n  tracked_by:\n    description: An external ticket reference.\n    source: [requirement]\n    target: []\n    external: true\n";
    fs::write(&schema_path, schema).unwrap();
    let schema = load_schema(&project).unwrap();
    fs::write(
        fixture.path().join("requirements.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:depends_on: REQ-B\n:tracked_by: external:https://example.invalid/ticket/1\n\nA requirement.\n:::\n\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n\nAnother requirement.\n:::\n",
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();

    let related = related_items(&corpus, &schema, "REQ-A", &RelatedFilters::default()).unwrap();
    assert_eq!(related.items.len(), 1);
    assert_eq!(related.items[0].item().id(), "REQ-B");

    let external_only = RelatedFilters::new(None, vec!["tracked_by".into()], vec![]);
    let related = related_items(&corpus, &schema, "REQ-A", &external_only).unwrap();
    assert!(related.items.is_empty());
}
