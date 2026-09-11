use std::{fs, path::Path, process::Command};

use mara::{ItemCreationRequest, ItemUpdateParams, Template};
use tempfile::TempDir;

const MID: &str = "01M1PXP2KG381MM1VNN6XC7S4M";

fn item(body: &str) -> String {
    format!(":::mara requirement REQ-ONE\n:mid: {MID}\n:title: One\n\n{body}\n:::\n")
}

fn fixture(source: &str) -> (TempDir, mara::Project, mara::Schema) {
    let dir = TempDir::new().unwrap();
    let project = mara::initialize_project(dir.path(), Template::Minimal).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    fs::write(dir.path().join("a.mara.md"), source).unwrap();
    (dir, project, schema)
}

fn update(body: &str) -> ItemUpdateParams {
    ItemUpdateParams {
        reference: "REQ-ONE".into(),
        title: None,
        fields: vec![],
        clear_fields: vec![],
        body: Some(body.into()),
    }
}

fn creation(body: &str) -> ItemCreationRequest {
    ItemCreationRequest {
        flavour: "requirement".into(),
        id: "REQ-NEW".into(),
        file: "a.mara.md".into(),
        title: "New".into(),
        fields: vec![],
        relations: vec![],
        body: Some(body.into()),
        line: Some(1),
    }
}

fn rejected(dir: &TempDir, before: &str, result: Result<impl Sized, mara::Error>) {
    let error = match result {
        Ok(_) => panic!("unsafe mutation succeeded"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("a.mara.md:") && error.contains("bytes") && error.contains("link"),
        "{error}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("a.mara.md")).unwrap(),
        before
    );
    assert!(!dir.path().join("b.mara.md").exists());
    assert!(!dir.path().join(".mara/transaction.json").exists());
}

#[test]
fn creation_deletion_update_and_same_document_move_reject_heading_retargeting() {
    for newline in ["\n", "\r\n"] {
        let source = format!(
            "{}\n# Same\n\nLast.\n\n[żółć](#same)\n",
            item("# Same\n\nFirst.")
        )
        .replace('\n', newline);
        let (dir, project, schema) = fixture(&source);
        rejected(
            &dir,
            &source,
            mara::create_item(&project, &schema, creation("# Same\n\nInserted.")),
        );
        rejected(
            &dir,
            &source,
            mara::delete_item(&project, &schema, "REQ-ONE"),
        );
        rejected(
            &dir,
            &source,
            mara::update_item(&project, &schema, update("# Renamed\n\nFirst.")),
        );
        rejected(
            &dir,
            &source,
            mara::move_item(&project, &schema, "REQ-ONE", Path::new("a.mara.md"), None),
        );
    }
}

#[test]
fn cross_document_move_checks_incoming_and_carried_relative_links() {
    for source in [
        format!("[inside](#inner)\n\n{}", item("# Inner\n\nContent.")),
        format!("# Outer\n\nContent.\n\n{}", item("[carried](#outer)")),
        format!(
            "<a name=\"outside\"></a>\n\nParagraph.\n\n{}",
            item("[carried](a.mara.md#outside)")
        ),
    ] {
        let (dir, project, schema) = fixture(&source);
        let destination = if source.contains("a.mara.md#outside") {
            "nested/b.mara.md"
        } else {
            "b.mara.md"
        };
        fs::create_dir(dir.path().join("nested")).unwrap();
        let result = mara::move_item(&project, &schema, "REQ-ONE", Path::new(destination), None);
        rejected(&dir, &source, result);
        assert!(!dir.path().join(destination).exists());
    }
}

#[test]
fn cross_document_move_rejects_valid_but_retargeted_carried_links() {
    let source = format!("# Local\n\nOriginal.\n\n{}", item("[carried](#local)"));
    let (dir, project, schema) = fixture(&source);
    let destination = "# Local\n\nDifferent.\n";
    fs::write(dir.path().join("b.mara.md"), destination).unwrap();
    let error =
        mara::move_item(&project, &schema, "REQ-ONE", Path::new("b.mara.md"), None).unwrap_err();
    assert!(error.to_string().contains("change destination"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.mara.md")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("b.mara.md")).unwrap(),
        destination
    );
}

#[test]
fn moving_reference_usage_does_not_count_as_editing_its_definition() {
    let source = format!("# One\n\nFirst.\n\n[dest]: #one\n\n{}", item("[ref][dest]"));
    let (dir, project, schema) = fixture(&source);
    let destination = "# Two\n\nSecond.\n\n[dest]: #two\n";
    fs::write(dir.path().join("b.mara.md"), destination).unwrap();
    let error =
        mara::move_item(&project, &schema, "REQ-ONE", Path::new("b.mara.md"), None).unwrap_err();
    assert!(error.to_string().contains("change destination"), "{error}");
    assert_eq!(
        fs::read_to_string(dir.path().join("a.mara.md")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("b.mara.md")).unwrap(),
        destination
    );
}

#[test]
fn explicit_anchor_destination_cannot_silently_change_to_another_block() {
    let source = format!(
        "[inside](#stable)\n\n{}",
        item("<a name=\"stable\"></a>\n\nFirst.")
    );
    // Move the anchor between blocks while its old paragraph remains in place.
    let (dir, project, schema) = fixture(&source);
    rejected(
        &dir,
        &source,
        mara::update_item(
            &project,
            &schema,
            update("First.\n\n<a name=\"stable\"></a>\n\nSecond."),
        ),
    );
}

#[test]
fn unchanged_links_in_replaced_bodies_are_protected_but_edited_links_may_change() {
    let source = item("# Same\n\nFirst.\n\n# Same\n\nSecond.\n\n[link](#same)");
    let (dir, project, schema) = fixture(&source);
    rejected(
        &dir,
        &source,
        mara::update_item(
            &project,
            &schema,
            update("# Same\n\nSecond.\n\n[link](#same)"),
        ),
    );
    rejected(
        &dir,
        &source,
        mara::update_item(
            &project,
            &schema,
            update("# Same\n\nInserted.\n\n# Same\n\nFirst.\n\n# Same\n\nSecond.\n\n[link](#same)"),
        ),
    );
    rejected(
        &dir,
        &source,
        mara::update_item(
            &project,
            &schema,
            update("# Other\n\nFirst.\n\n# Same\n\nSecond.\n\n[link](#same)"),
        ),
    );
    mara::update_item(
        &project,
        &schema,
        update("# Other\n\nFirst.\n\n# Same\n\nSecond.\n\n[link](#other)"),
    )
    .unwrap();
    assert!(
        mara::validate_corpus(&mara::load_corpus(&project, &schema).unwrap(), &schema).is_empty()
    );
}

#[test]
fn removed_self_references_do_not_block_deletion_and_identity_moves_work() {
    let source = item("# Inner\n\n[[REQ-ONE]] [self](#inner)");
    let (dir, project, schema) = fixture(&source);
    mara::delete_item(&project, &schema, "REQ-ONE").unwrap();
    assert!(
        fs::read_to_string(dir.path().join("a.mara.md"))
            .unwrap()
            .is_empty()
    );

    let source = format!(
        "[[REQ-ONE]] [[{MID}]]\n\n{}",
        item("# Inner\n\n[self](#inner)")
    );
    fs::write(dir.path().join("a.mara.md"), source).unwrap();
    let moved =
        mara::move_item(&project, &schema, "REQ-ONE", Path::new("b.mara.md"), None).unwrap();
    assert_eq!(moved.mid, MID);
    assert!(
        mara::validate_corpus(&mara::load_corpus(&project, &schema).unwrap(), &schema).is_empty()
    );
}

#[test]
fn rename_rewrites_narrative_ids_preserving_mids_and_literal_contexts() {
    let source = format!(
        "[[REQ-ONE]] [[{MID}]] `[[REQ-ONE]]` \\[[REQ-ONE]]\n\n<!-- [[REQ-ONE]] -->\n\n{}",
        item("Self [[REQ-ONE]].")
    );
    let (dir, project, schema) = fixture(&source);
    mara::rename_item(&project, &schema, "REQ-ONE", "REQ-RENAMED").unwrap();
    let after = fs::read_to_string(dir.path().join("a.mara.md")).unwrap();
    assert!(after.starts_with(&format!(
        "[[REQ-RENAMED]] [[{MID}]] `[[REQ-ONE]]` \\[[REQ-ONE]]"
    )));
    assert!(after.contains("<!-- [[REQ-ONE]] -->"));
    assert!(after.contains("Self [[REQ-RENAMED]]."));
}

#[test]
fn creation_rejects_new_broken_markdown_links_without_initial_relations() {
    let source = "# Existing\n\nContent.\n";
    let (dir, project, schema) = fixture(source);
    rejected(
        &dir,
        source,
        mara::create_item(&project, &schema, creation("[broken](#absent)")),
    );
}

#[test]
fn cli_preflight_reports_every_affected_source_location_before_writes() {
    let source = format!(
        "[one](#same) [two](#same)\n\n{}\n# Same\n\nSurvivor.\n",
        item("# Same\n\nDeleted.")
    );
    let (dir, _, _) = fixture(&source);
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .current_dir(dir.path())
        .args(["item", "delete", "REQ-ONE"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert_eq!(error.matches("untouched link").count(), 2, "{error}");
    assert_eq!(
        fs::read_to_string(dir.path().join("a.mara.md")).unwrap(),
        source
    );
}
