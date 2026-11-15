//! VER-PRESERVED-BYTES, VER-REFUSED-EDITS, the lock case of VER-CONCURRENT-WRITERS and the last
//! row of VER-FIX-CASES, for move and rename.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use mara::write::WriteLock;

use crate::{Project, Tree, W, X, Y, Z, changed, paused, temporary_left, unchanged};

/// CRLF with a byte order mark, front matter, irregular table padding, emphasis, trailing spaces
/// and no final newline; mode 0600.
fn first() -> String {
    format!(
        "\u{feff}---\ntitle: One\n---\n# One  \n\n---\n\n<a id=\"{X}\"></a>\n\n### *Moved* item\n\n\
         |  Type |   Requirement |\n|---|:---|\n| ID    |  REQ-MOVED |\n|Status|Draft|\n\
         |  Refines  |  [REQ-NEIGHBOUR](#{Y})  |\n\n\
         It _mentions_ [REQ-NEIGHBOUR](#{Y}) and [the third](../three/c.mara.md#{Z}).  \n\n\
         Body of REQ-MOVED.\n\n---\n\n<a id=\"{Y}\"></a>\n\n### Neighbour\n\n\
         | Type | Requirement |\n| --- | --- |\n|   ID |REQ-NEIGHBOUR|\n| Status | Draft |\n\
         | Refines |   [REQ-MOVED](#{X})   |\n\nIt mentions [REQ-MOVED](#{X}).  \n\n\
         Body of REQ-NEIGHBOUR."
    )
    .replace('\n', "\r\n")
}

/// LF, mode 0644.
fn second() -> String {
    format!(
        "# Two\n\n---\n\n<a id=\"{W}\"></a>\n\n### Dest\n\n|Type|Requirement|\n|-|-|\n|ID|REQ-DEST|\n\
         |Status|Draft|\n|Refines|[REQ-MOVED](../one/a.mara.md#{X})|\n\n\
         It mentions [the *moved* item](../one/a.mara.md#{X}).\n\nBody of REQ-DEST.\n"
    )
}

/// LF without a final newline.
fn third() -> String {
    format!(
        "# Three\n\nThe narrative mentions [REQ-MOVED](../one/a.mara.md#{X}) with __emphasis__.\n\n\
         ---\n\n<a id=\"{Z}\"></a>\n\n### Third\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-THIRD |\n| Status | Draft |\n| Refines | [REQ-MOVED](../one/a.mara.md#{X})    |\n\n\
         Body of REQ-THIRD."
    )
}

fn formatted() -> Project {
    let project = Project::new();
    for (path, text, mode) in [
        ("one/a.mara.md", first(), 0o600),
        ("two/b.mara.md", second(), 0o644),
        ("three/c.mara.md", third(), 0o644),
        (
            "four/d.mara.md",
            "# Four\n\nNarrative only.\n".to_owned(),
            0o644,
        ),
    ] {
        project.write(path, &text);
        fs::set_permissions(project.path(path), fs::Permissions::from_mode(mode)).unwrap();
    }
    assert_eq!(project.problems(), []);
    project
}

fn mode(project: &Project, path: &str) -> u32 {
    fs::metadata(project.path(path))
        .unwrap()
        .permissions()
        .mode()
        & 0o7777
}

/// Each file that the edit changed has the expected bytes and its mode; every other file keeps
/// its bytes and mtime.
fn only(project: &Project, before: &Tree, expected: &[(&str, String)]) {
    let after = project.tree();
    for (path, text) in expected {
        assert_eq!(project.read(path), *text, "{path}");
    }
    let mut written: Vec<&str> = expected.iter().map(|(path, _)| *path).collect();
    written.sort();
    assert_eq!(changed(before, &after), written);
    for (path, (bytes, mtime)) in before {
        if !written.contains(&path.as_str()) {
            assert_eq!(after[path], (bytes.clone(), *mtime), "{path}");
        }
    }
    for (path, mode_before) in [("one/a.mara.md", 0o600), ("two/b.mara.md", 0o644)] {
        assert_eq!(mode(project, path), mode_before, "{path}");
    }
}

#[test]
fn a_move_changes_only_the_bytes_it_must() {
    let project = formatted();
    let before = project.tree();
    project.json(&["item", "move", "REQ-MOVED", "two/b.mara.md"], 0);
    let old = first();
    let start = old.find("<a id").unwrap();
    let end = old.find("Body of REQ-MOVED.").unwrap() + "Body of REQ-MOVED.".len();
    let moved = old[start..end].replace(&format!("(#{Y})"), &format!("(../one/a.mara.md#{Y})"));
    let rest = &old[end + 2..];
    let heading = old.find("# One  \r\n").unwrap() + "# One  \r\n".len();
    let source = format!("{}{rest}", &old[..heading])
        .replace(&format!("(#{X})"), &format!("(../two/b.mara.md#{X})"));
    let destination = second().replace(&format!("../one/a.mara.md#{X}"), &format!("#{X}"))
        + &format!("\n---\n\n{moved}\n");
    let narrative = third().replace("../one/a.mara.md", "../two/b.mara.md");
    only(
        &project,
        &before,
        &[
            ("two/b.mara.md", destination),
            ("one/a.mara.md", source),
            ("three/c.mara.md", narrative),
        ],
    );
    assert_eq!(project.problems(), []);
}

#[test]
fn a_rename_changes_only_the_bytes_it_must() {
    let project = formatted();
    let before = project.tree();
    project.json(&["item", "rename", X, "REQ-RENAMED"], 0);
    let renamed = |text: String| {
        text.replace("[REQ-MOVED](", "[REQ-RENAMED](")
            .replace("|  REQ-MOVED |", "|  REQ-RENAMED |")
    };
    only(
        &project,
        &before,
        &[
            ("one/a.mara.md", renamed(first())),
            ("two/b.mara.md", renamed(second())),
            ("three/c.mara.md", renamed(third())),
        ],
    );
    assert_eq!(project.problems(), []);
}

const MOVE: &[&str] = &["item", "move", "REQ-MOVED", "two/b.mara.md"];
const RENAME: &[&str] = &["item", "rename", "REQ-MOVED", "REQ-RENAMED"];

#[test]
fn refuses_requests_that_cannot_run_and_writes_nothing() {
    let project = Project::three_directories();
    project.write(
        "four/d.mara.md",
        &crate::item("01M7D8DZ4A2C6WS0NP9J4TQ1VH", "REQ-TWICE", "", ""),
    );
    let tree = project.tree();
    for (request, extra, code, status) in [
        ("unknown", ["REQ-NONE"].as_slice(), "unknown-item", 1),
        (
            "undeclared",
            ["REQ-MOVED", "--bogus", "x"].as_slice(),
            "invalid-argument",
            2,
        ),
    ] {
        for words in [&["item", "move"][..], &["item", "rename"]] {
            let mut args = words.to_vec();
            args.extend(extra);
            args.push(if words[1] == "move" {
                "two/b.mara.md"
            } else {
                "REQ-NEW"
            });
            if request == "undeclared" {
                args.retain(|arg| !["--bogus", "x"].contains(arg));
                args.extend(["--bogus", "x"]);
            }
            let result = project.json(&args, status);
            assert_eq!(result["error"]["code"], code, "{args:?}: {result}");
            unchanged(&tree, &project.tree(), request);
        }
    }
    // Two items with one ID.
    project.write(
        "four/d.mara.md",
        &crate::item("01M7D8DZ4A2C6WS0NP9J4TQ1VH", "REQ-MOVED", "", ""),
    );
    let tree = project.tree();
    for args in [MOVE, RENAME] {
        let result = project.json(args, 1);
        assert_eq!(result["error"]["code"], "ambiguous-item", "{result}");
        unchanged(&tree, &project.tree(), "ambiguous");
    }
    // A schema with an error.
    project.write(".mara/schema.toml", "format_version = 2\n");
    let tree = project.tree();
    for args in [MOVE, RENAME] {
        let result = project.json(args, 1);
        assert_eq!(result["error"]["code"], "invalid-project", "{result}");
        unchanged(&tree, &project.tree(), "invalid project");
    }
}

/// Changes `path` while the writer waits at the pause whose number is `pause`, and requires a
/// refusal that replaced nothing.
fn refused_after_change(args: &[&str], path: &str, pause: usize) {
    let project = Project::three_directories();
    let before = project.tree();
    let changed_text = project.read(path) + "\nChanged by another program.\n";
    let (output, pauses) = paused(&project, args, |_, index| {
        if index == pause {
            project.write(path, &changed_text);
        }
    });
    let case = format!("{args:?} with {path} changed at pause {pause} of {pauses:?}");
    assert_eq!(output.status.code(), Some(1), "{case}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error[changed-file]"),
        "{case}"
    );
    let after = project.tree();
    assert_eq!(changed(&before, &after), [path], "{case}");
    assert_eq!(project.read(path), changed_text, "{case}");
    assert!(!temporary_left(&after), "{case}");
}

#[test]
fn refuses_an_edit_whose_file_changed_before_its_first_write() {
    // Before step 2: the first and a later file of a multi-file edit, and the only file of a
    // single-file one.
    refused_after_change(MOVE, "two/b.mara.md", 0);
    refused_after_change(MOVE, "three/c.mara.md", 0);
    refused_after_change(
        &["item", "move", "REQ-MOVED", "one/a.mara.md", "--line", "1"],
        "one/a.mara.md",
        0,
    );
    refused_after_change(RENAME, "three/c.mara.md", 0);
    refused_after_change(RENAME, "two/b.mara.md", 0);
    // At the first file's second check.
    refused_after_change(MOVE, "two/b.mara.md", 1);
    refused_after_change(RENAME, "three/c.mara.md", 1);
}

/// A write waits while another Mara writer holds the lock, then is computed on what that writer
/// left.
#[test]
fn a_rename_waits_for_the_lock_and_applies_to_the_changed_file() {
    let project = Project::three_directories();
    let lock = WriteLock::take(&project.root()).unwrap();
    let before = project.tree();
    let mut child = (project.mara(RENAME, &[]))
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let held = Instant::now() + Duration::from_millis(500);
    while Instant::now() < held {
        assert!(
            child.try_wait().unwrap().is_none(),
            "the rename ran while the lock was held"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    unchanged(&before, &project.tree(), "while the lock is held");
    let changed_text = project.read("two/b.mara.md") + "\nAdded by the other writer.\n";
    project.write("two/b.mara.md", &changed_text);
    drop(lock);
    assert!(child.wait().unwrap().success());
    let after = project.read("two/b.mara.md");
    assert!(
        after.contains("Added by the other writer.") && after.contains("[REQ-RENAMED]("),
        "{after}"
    );
}

/// Problems outside what a move or rename edits stay as they were.
#[test]
fn leaves_the_problems_it_does_not_edit() {
    for args in [MOVE, RENAME] {
        let project = Project::three_directories();
        let stale = format!("Also [the third](../one/a.mara.md#{Z}).\n\n");
        let text = project
            .read("two/b.mara.md")
            .replace("Body of REQ-DEST.", &format!("{stale}Body of REQ-DEST."));
        project.write("two/b.mara.md", &text);
        let problems = "# Four\n\n---\n\n### Without anchor\n\n| Type | Requirement |\n| --- | --- |\n\
             | ID | REQ-LOST |\n| Status | Draft |\n\nBody.\n\n---\n\n\
             <a id=\"01M7D8DZ4A2C6WS0NP9J4TQ1VH\"></a>\n\n### Bare\n\n| Type | Requirement |\n\
             | --- | --- |\n| ID | REQ-BARE |\n| Status | Draft |\n| Refines | REQ-THIRD |\n\nBody.\n\
             Narrative right before an anchor.\n\n<a id=\"01M7D8DZ4CF8P5Q9K3N5D9V7M1\"></a>\n\n\
             Setext\n------\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-SETEXT |\n\
             | Status | Draft |\n\nBody.\n";
        project.write("four/d.mara.md", problems);
        let before = project.any_problems();
        let codes: Vec<&str> = before.iter().map(|(code, _)| code.as_str()).collect();
        for code in ["stale-link-path", "bare-id-value", "missing-separator"] {
            assert!(codes.contains(&code), "{code} in {before:?}");
        }
        let four = project.tree()["four/d.mara.md"].clone();
        project.json(args, 0);
        assert_eq!(project.any_problems(), before, "{args:?}");
        assert_eq!(project.tree()["four/d.mara.md"], four, "{args:?}");
        assert!(project.read("two/b.mara.md").contains(&stale), "{args:?}");
    }
}
