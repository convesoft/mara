use std::fs;

use mara::{RelatedFilters, Template, initialize_project, load_corpus, load_schema, related_items};
use tempfile::TempDir;

#[test]
fn item_only_related_items_skips_external_and_code_targets() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap()
        + "\n  tracked_by:\n    description: An external ticket reference.\n    source: [requirement]\n    target: []\n    external: true\n  code_implements:\n    description: Code implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n";
    fs::write(&schema_path, schema).unwrap();
    let schema = load_schema(&project).unwrap();
    fs::write(
        fixture.path().join("requirements.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:depends_on: REQ-B\n:tracked_by: external:https://example.invalid/ticket/1\n:implemented_by_code: code:implementation.rs\n\nA requirement.\n:::\n\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n\nAnother requirement.\n:::\n",
    )
    .unwrap();
    fs::write(fixture.path().join("implementation.rs"), "fn run() {}\n").unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();

    let related = related_items(&corpus, &schema, "REQ-A", &RelatedFilters::default()).unwrap();
    assert_eq!(related.items.len(), 1);
    assert_eq!(related.items[0].item().id(), "REQ-B");

    let external_only = RelatedFilters::new(None, vec!["tracked_by".into()], vec![]);
    let related = related_items(&corpus, &schema, "REQ-A", &external_only).unwrap();
    assert!(related.items.is_empty());
}

#[test]
fn public_relation_helpers_author_and_remove_item_side_code_links() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap()
        + "\n  code_implements:\n    description: Code implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n";
    fs::write(&schema_path, schema).unwrap();
    let schema = load_schema(&project).unwrap();
    let item_path = fixture.path().join("requirements.mara.md");
    fs::write(
        &item_path,
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nA requirement.\n:::\n",
    )
    .unwrap();
    fs::write(fixture.path().join("implementation.rs"), "fn run() {}\n").unwrap();

    let added = mara::add_relation(
        &project,
        &schema,
        "REQ-A",
        "implemented_by_code",
        "code:implementation.rs",
    )
    .unwrap();
    assert_eq!(added.target(), "code:implementation.rs");
    assert!(
        fs::read_to_string(&item_path)
            .unwrap()
            .contains(":implemented_by_code: code:implementation.rs")
    );

    mara::remove_relation(
        &project,
        &schema,
        "REQ-A",
        "implemented_by_code",
        "code:implementation.rs",
    )
    .unwrap();
    assert!(
        !fs::read_to_string(&item_path)
            .unwrap()
            .contains(":implemented_by_code: code:implementation.rs")
    );

    let error = mara::add_relation(
        &project,
        &schema,
        "code:implementation.rs",
        "code_implements",
        "REQ-A",
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot modify code source files")
    );
}
