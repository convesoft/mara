//! VER-RENAME-ID, and the `item rename` row of VER-MID-RESOLUTION.

use serde_json::json;

use crate::{Project, X, documents, unchanged};

#[test]
fn renames_an_id_wherever_it_is_a_handle() {
    let project = Project::three_directories();
    let incoming = |project: &Project| {
        let result = project.json(&["relation", "list", X], 0);
        let connections = result["connections"].as_array().unwrap().clone();
        let incoming = connections
            .into_iter()
            .filter(|connection| connection["direction"] == "incoming");
        incoming
            .map(|connection| connection["source"].clone())
            .collect::<Vec<_>>()
    };
    let sources = incoming(&project);

    let result = project.json(&["item", "rename", "REQ-MOVED", "REQ-RENAMED"], 0);
    assert_eq!(result["old_id"], "REQ-MOVED");
    assert_eq!(result["item"]["id"], "REQ-RENAMED");
    assert_eq!(result["item"]["mid"], X);
    // Mention text first, then the `ID` row, then relation link text only
    // (ADR-MENTIONS-BEFORE-ID-ROW).
    assert_eq!(
        result["files"],
        json!(["three/c.mara.md", "one/a.mara.md", "two/b.mara.md"])
    );
    assert_eq!(project.problems(), []);
    // The anchor keeps the MID; the `ID` row, every inbound relation link's text and every
    // mention text that was the old ID carry the new ID; other mention text and the plain text
    // stay. Nothing else changed.
    for (path, before) in documents() {
        let after = project.read(path);
        let expected = before
            .replace("| ID | REQ-MOVED |", "| ID | REQ-RENAMED |")
            .replace("[REQ-MOVED](", "[REQ-RENAMED](");
        assert_eq!(after, expected, "{path}");
    }
    assert!(
        project
            .read("three/c.mara.md")
            .contains("REQ-MOVED is plain text here.")
    );
    assert!(
        project
            .read("one/a.mara.md")
            .contains(&format!("<a id=\"{X}\"></a>"))
    );

    // The old ID no longer resolves.
    for args in [
        &["get", "REQ-MOVED"][..],
        &["item", "validate", "REQ-MOVED"],
    ] {
        let result = project.json(args, 1);
        assert_eq!(result["error"]["code"], "unknown-item", "{args:?}");
    }
    // `get` and `relation list` of the MID (VER-MID-RESOLUTION).
    let got = project.json(&["get", X], 0);
    assert_eq!(got["node"]["id"], "REQ-RENAMED");
    assert_eq!(incoming(&project), sources);
}

#[test]
fn refuses_a_new_id_that_would_not_be_valid_and_writes_nothing() {
    let project = Project::three_directories();
    let tree = project.tree();
    for (new_id, code, status) in [
        ("req-lower", "invalid-argument", 2),
        ("REQ", "invalid-argument", 2),
        ("NOT-REQ", "wrong-id-prefix", 1),
        ("REQ-DEST", "duplicate-id", 1),
    ] {
        let result = project.json(&["item", "rename", "REQ-MOVED", new_id], status);
        assert_eq!(result["error"]["code"], code, "{new_id}: {result}");
        unchanged(&tree, &project.tree(), new_id);
    }
    // The current ID: nothing to write.
    let result = project.json(&["item", "rename", "REQ-MOVED", "REQ-MOVED"], 0);
    assert_eq!(result["files"], json!([]));
    unchanged(&tree, &project.tree(), "the same ID");
}
