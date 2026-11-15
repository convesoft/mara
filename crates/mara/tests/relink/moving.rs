//! VER-MOVE-ITEM, and the `item move` rows of VER-MID-RESOLUTION, VER-GITHUB-RENDERING and
//! VER-NESTED-PROJECTS.

use serde_json::{Value, json};

use crate::{Project, W, X, Y, Z, item, unchanged};

/// The connections of `relation list`: kind, relation, direction and neighbour.
fn connections(project: &Project, node: &str) -> Vec<Value> {
    let result = project.json(&["relation", "list", node], 0);
    let mut connections: Vec<Value> = result["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|connection| {
            json!({
                "kind": connection["kind"],
                "relation": connection["relation"],
                "direction": connection["direction"],
                "neighbour": connection["neighbour"]["mid"],
            })
        })
        .collect();
    connections.sort_by_key(|connection| connection.to_string());
    connections
}

#[test]
fn moves_an_item_to_a_document_in_another_directory_with_every_link() {
    let project = Project::three_directories();
    assert_eq!(project.problems(), []);
    let before = connections(&project, X);
    let rows = project.json(&["get", X], 0)["rows"].clone();

    let result = project.json(&["item", "move", "REQ-MOVED", "two/b.mara.md"], 0);
    assert_eq!(result["item"]["path"], "two/b.mara.md");
    assert_eq!(result["item"]["mid"], X);
    assert_eq!(result["from"], json!({"path": "one/a.mara.md", "line": 5}));
    // Destination, source, then the other document whose links change (DES-WRITE-PROCEDURE).
    assert_eq!(
        result["files"],
        json!(["two/b.mara.md", "one/a.mara.md", "three/c.mara.md"])
    );
    assert_eq!(project.problems(), []);

    // The anchor, heading, rows and body, with the item's own links resolving from the new file.
    let moved = item(
        X,
        "REQ-MOVED",
        &format!(
            "| Refines | [REQ-NEIGHBOUR](../one/a.mara.md#{Y}) |\n\
             | Refines | [REQ-THIRD](../three/c.mara.md#{Z}) |\n"
        ),
        &format!(
            "It mentions [itself](#{X}), [REQ-NEIGHBOUR](../one/a.mara.md#{Y}) and [the third](../three/c.mara.md#{Z}).\n\n"
        ),
    );
    let destination = project.read("two/b.mara.md");
    assert!(
        destination.ends_with(&format!("\n---\n\n{moved}")),
        "{destination}"
    );
    // Every inbound relation link and mention carries the new path.
    assert!(destination.contains(&format!("| Refines | [REQ-MOVED](#{X}) |")));
    assert!(destination.contains(&format!("[the moved item](#{X})")));
    let source = project.read("one/a.mara.md");
    assert!(!source.contains(&format!("<a id=\"{X}\">")));
    assert!(source.contains(&format!("| Refines | [REQ-MOVED](../two/b.mara.md#{X}) |")));
    assert!(source.contains(&format!("It mentions [REQ-MOVED](../two/b.mara.md#{X}).")));
    let third = project.read("three/c.mara.md");
    assert_eq!(
        third.matches(&format!("](../two/b.mara.md#{X})")).count(),
        3
    );

    // `get` and `relation list` of the MID: the item at its new location, every connection.
    let got = project.json(&["get", X], 0);
    assert_eq!(got["node"]["path"], "two/b.mara.md");
    assert_eq!(
        got["rows"].as_array().unwrap().len(),
        rows.as_array().unwrap().len()
    );
    assert_eq!(connections(&project, X), before);
    // No document that the move wrote starts with `---` (VER-GITHUB-RENDERING).
    for path in ["one/a.mara.md", "two/b.mara.md", "three/c.mara.md"] {
        assert!(!project.read(path).starts_with("---"), "{path}");
    }
}

#[test]
fn moves_an_item_into_a_new_document_and_within_its_own() {
    let project = Project::three_directories();
    let result = project.json(&["item", "move", X, "four/five/new.mara.md"], 0);
    assert_eq!(result["files"][0], "four/five/new.mara.md");
    let created = project.read("four/five/new.mara.md");
    assert!(
        created.starts_with(&format!("<a id=\"{X}\"></a>\n")),
        "{created}"
    );
    assert_eq!(project.problems(), []);
    assert_eq!(
        project.json(&["get", X], 0)["node"]["path"],
        "four/five/new.mara.md"
    );

    // Within one document: the item at that position, byte-identical, the other regions as
    // they were.
    let project = Project::three_directories();
    let before = project.read("one/a.mara.md");
    let x_text =
        &before[before.find("<a id").unwrap()..before.find("\n---\n\n<a id=\"01M7D8DZ2R").unwrap()];
    let y_text = &before[before.rfind("<a id").unwrap()..];
    let result = project.json(
        &["item", "move", "REQ-MOVED", "one/a.mara.md", "--line", "1"],
        0,
    );
    assert_eq!(result["files"], json!(["one/a.mara.md"]));
    assert_eq!(result["item"]["line"], 1);
    let after = project.read("one/a.mara.md");
    assert_eq!(after, format!("{x_text}\n---\n\n# One\n\n---\n\n{y_text}"));
    assert_eq!(project.problems(), []);

    // To where it is: nothing is written.
    let tree = project.tree();
    for line in ["1", "15", "16", "17"] {
        let result = project.json(&["item", "move", X, "one/a.mara.md", "--line", line], 0);
        assert_eq!(result["files"], json!([]), "line {line}");
    }
    unchanged(&tree, &project.tree(), "a move to where the item is");
}

#[test]
fn refuses_a_destination_that_would_be_no_input_and_writes_nothing() {
    let project = Project::three_directories();
    project.write(".gitignore", "ignored/\n");
    project.write("inner/.mara/config.toml", crate::CONFIG);
    project.write("inner/.mara/schema.toml", crate::SCHEMA);
    project.write("inner/doc.mara.md", "# Inner\n");
    project.write("notes.txt", "Notes.\n");
    std::os::unix::fs::symlink(project.path("two"), project.path("linked")).unwrap();
    let tree = project.tree();
    for (path, line, code, status) in [
        ("notes.md", None, "not-an-input", 1),
        ("ignored/x.mara.md", None, "not-an-input", 1),
        // VER-NESTED-PROJECTS: a path below the inner project's root.
        ("inner/x.mara.md", None, "not-an-input", 1),
        ("inner/doc.mara.md", None, "not-an-input", 1),
        ("linked/x.mara.md", None, "symbolic-link", 1),
        (".git/x.mara.md", None, "not-an-input", 1),
        ("../x.mara.md", None, "invalid-argument", 2),
        ("two/b.mara.md", Some("7"), "invalid-position", 1),
        ("two/b.mara.md", Some("99"), "invalid-position", 1),
        ("two/b.mara.md", Some("0"), "invalid-argument", 2),
    ] {
        let mut args = vec!["item", "move", "REQ-MOVED", path];
        args.extend(line.iter().flat_map(|line| ["--line", line]));
        let result = project.json(&args, status);
        assert_eq!(result["error"]["code"], code, "{path} {line:?}: {result}");
        unchanged(&tree, &project.tree(), path);
    }
}

/// mara-jnvzxr: a relation row and a mention of the moved item that use a definition at the end
/// of the page.
#[test]
fn refuses_a_move_that_would_leave_a_definition_behind() {
    let project = Project::three_directories();
    let text = project.read("one/a.mara.md");
    let text = text.replace(
        &format!("| Refines | [REQ-THIRD](../three/c.mara.md#{Z}) |"),
        "| Refines | [REQ-THIRD][third] |",
    );
    let text = text.replace(
        &format!("[the third](../three/c.mara.md#{Z})"),
        "[the third][third]",
    );
    project.write(
        "one/a.mara.md",
        &format!("{text}\n[third]: ../three/c.mara.md#{Z}\n"),
    );
    assert_eq!(project.problems(), []);
    let tree = project.tree();
    for destination in ["two/b.mara.md", "one/a.mara.md"] {
        let result = project.json(&["item", "move", "REQ-MOVED", destination], 1);
        let error = &result["error"];
        assert_eq!(error["code"], "unreadable-edit", "{result}");
        let message = error["message"].as_str().unwrap();
        assert!(
            message.contains("`[REQ-THIRD][third]`") && message.contains("line 36"),
            "{message}"
        );
        assert_eq!(error["diagnostics"][0]["line"], 14);
        unchanged(&tree, &project.tree(), destination);
    }
    // Moving the item that the definition does not concern works.
    project.json(&["item", "move", "REQ-DEST", "one/a.mara.md"], 0);
    assert_eq!(project.problems(), []);
    let _ = W;
}
