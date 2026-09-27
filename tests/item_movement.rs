use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;
fn valid(root: &Path) {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let corpus = mara::load_corpus(&project, &schema).unwrap();
    assert!(mara::validate_corpus(&corpus, &schema).is_empty());
}
fn move_item(
    root: &Path,
    mcp: bool,
    reference: &str,
    file: &str,
    line: Option<usize>,
) -> Result<Value, String> {
    if mcp {
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "item_move",
                    json!({"reference":reference,"file":file,"line":line}),
                ),
            ],
        );
        let response = &mcp_response(&responses, 2)["result"];
        if response["isError"] == true {
            Err(response.to_string())
        } else {
            Ok(response["structuredContent"].clone())
        }
    } else {
        let mut args = vec!["--format", "json", "item", "move", reference, file];
        let n = line.map(|n| n.to_string());
        if let Some(n) = &n {
            args.extend(["--line", n]);
        }
        let output = mara(root, &args);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        if output.status.success() {
            Ok(value)
        } else {
            Err(value["error"]["message"].as_str().unwrap().into())
        }
    }
}
fn related_snapshot(root: &Path, id: &str) -> Vec<Value> {
    let output = mara(root, &["--format", "json", "related", id]);
    assert!(output.status.success(), "{}", stderr(&output));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    result["connections"].as_array().unwrap().iter().filter(|entry| entry["relation"] != "contains").map(|entry| json!({
        "direction": entry["direction"], "relation": entry["relation"], "mid": entry["neighbour"]["mid"]
    })).collect()
}

fn move_fixture() -> (TempDir, String, String) {
    let fixture = support::fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    for (id, file, body) in [
        (
            "REQ-MOVE",
            "source.mara.md",
            "Exact Unicode body: żółć.\n\n`[[REQ-EXAMPLE]]`",
        ),
        ("REQ-STAY", "destination.mara.md", "Reference [[REQ-MOVE]]."),
    ] {
        let output = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                id,
                file,
                "--title",
                id,
                "--body",
                body,
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let relation = mara(
        fixture.path(),
        &["relation", "add", "REQ-STAY", "depends_on", "REQ-MOVE"],
    );
    assert!(relation.status.success(), "{}", stderr(&relation));
    let source = fs::read_to_string(fixture.path().join("source.mara.md"))
        .unwrap()
        .replace('\n', "\r\n");
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    let destination = fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap();
    (fixture, source, destination)
}

// @mara implements VER-ITEM-MOVEMENT
// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_cross_document_preserves_bytes_permissions_references_and_identity() {
    let (fixture, source, destination) = move_fixture();
    #[cfg(unix)]
    {
        fs::set_permissions(
            fixture.path().join("source.mara.md"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        fs::set_permissions(
            fixture.path().join("destination.mara.md"),
            fs::Permissions::from_mode(0o604),
        )
        .unwrap();
    }
    let original = mara(fixture.path(), &["--format", "json", "get", "REQ-MOVE"]);
    let original: Value = serde_json::from_slice(&original.stdout).unwrap();
    let original_related = related_snapshot(fixture.path(), "REQ-MOVE");
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "move",
            original["node"]["mid"].as_str().unwrap(),
            "destination.mara.md",
            "--line",
            "1",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result,
        json!({"id": "REQ-MOVE", "mid": original["node"]["mid"], "old_location": {"path": "source.mara.md", "line": 1}, "new_location": {"path": "destination.mara.md", "line": 1}})
    );
    assert_eq!(
        fs::read(fixture.path().join("source.mara.md")).unwrap(),
        b""
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        source.clone() + "\n" + &destination
    );
    #[cfg(unix)]
    for (path, mode) in [("source.mara.md", 0o640), ("destination.mara.md", 0o604)] {
        assert_eq!(
            fs::metadata(fixture.path().join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            mode
        );
    }
    let resolved = mara(fixture.path(), &["--format", "json", "get", "REQ-MOVE"]);
    let resolved: Value = serde_json::from_slice(&resolved.stdout).unwrap();
    assert_eq!(resolved["node"]["mid"], original["node"]["mid"]);
    assert_eq!(resolved["content"], original["content"]);
    assert_eq!(
        related_snapshot(fixture.path(), "REQ-MOVE"),
        original_related
    );
    valid(fixture.path());
    assert!(!fixture.path().join(".mara/transaction.json").exists());
}

// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_repositions_with_original_line_coordinates_and_creates_missing_document() {
    let (fixture, source, _) = move_fixture();
    let original = format!("# Heading\r\n\r\n{source}\r\nTail without newline");
    fs::write(fixture.path().join("source.mara.md"), &original).unwrap();
    let output = mara(
        fixture.path(),
        &["item", "move", "REQ-MOVE", "source.mara.md", "--line", "1"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("from source.mara.md:3 to source.mara.md:1"));
    let moved = fs::read_to_string(fixture.path().join("source.mara.md")).unwrap();
    assert_eq!(
        moved,
        format!("{source}\r\n# Heading\r\n\r\n\r\nTail without newline")
    );
    let eof = moved.lines().count() + 1;
    let output = mara(
        fixture.path(),
        &[
            "item",
            "move",
            "REQ-MOVE",
            "source.mara.md",
            "--line",
            &eof.to_string(),
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let appended = fs::read_to_string(fixture.path().join("source.mara.md")).unwrap();
    assert!(appended.ends_with(&source));
    assert!(appended.starts_with("\r\n# Heading\r\n\r\n\r\nTail without newline\r\n\r\n"));
    let output = mara(fixture.path(), &["item", "move", "REQ-MOVE", "new.mara.md"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(fixture.path().join("new.mara.md")).unwrap(),
        source
    );
    valid(fixture.path());
}

// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_rejections_leave_all_original_documents_unchanged() {
    let (fixture, source, destination) = move_fixture();
    fs::write(
        fixture.path().join("context.mara.md"),
        "```markdown\nexample\n```\n",
    )
    .unwrap();
    fs::write(fixture.path().join(".gitignore"), "ignored.mara.md\n").unwrap();
    for (file, line) in [
        ("destination.mara.md", "2"),
        ("source.mara.md", "2"),
        ("destination.mara.md", "0"),
        ("destination.mara.md", "9999"),
        ("../escape.mara.md", "1"),
        ("/tmp/escape.mara.md", "1"),
        ("missing/parent.mara.md", "1"),
        ("wrong.md", "1"),
        ("ignored.mara.md", "1"),
        ("context.mara.md", "2"),
    ] {
        let output = mara(
            fixture.path(),
            &["item", "move", "REQ-MOVE", file, "--line", line],
        );
        assert!(!output.status.success(), "accepted {file}:{line}");
        assert_eq!(
            fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
            destination
        );
        assert!(!fixture.path().join(".mara/transaction.json").exists());
    }
    fs::write(
        fixture.path().join("broken.mara.md"),
        destination
            .replace("REQ-STAY", "REQ-BROKEN")
            .replace("[[REQ-MOVE]]", "[[REQ-MISSING]]"),
    )
    .unwrap();
    let output = mara(fixture.path(), &["item", "move", "REQ-MOVE", "new.mara.md"]);
    assert!(!output.status.success());
    assert!(!fixture.path().join("new.mara.md").exists());
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
}

// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_mcp_matches_cli_and_rejects_bound_project_overrides() {
    let (fixture, source, destination) = move_fixture();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "move",
            "REQ-MOVE",
            "destination.mara.md",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("destination.mara.md"), &destination).unwrap();
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", fixture.path().to_str().unwrap()],
        &[
            mcp_initialize(1),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            mcp_call(
                2,
                "item_move",
                json!({"reference": "REQ-MOVE", "file": "destination.mara.md"}),
            ),
            mcp_call(
                3,
                "item_move",
                json!({"reference": "REQ-MOVE", "file": "source.mara.md", "project": fixture.path()}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        expected
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    valid(fixture.path());
}

// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_without_final_newline_preserves_authored_bytes_and_handles_noop() {
    let (fixture, source, destination) = move_fixture();
    let source = source.trim_end_matches("\r\n");
    fs::write(fixture.path().join("source.mara.md"), source).unwrap();
    let noop = mara(
        fixture.path(),
        &["item", "move", "REQ-MOVE", "source.mara.md", "--line", "1"],
    );
    assert!(noop.status.success(), "{}", stderr(&noop));
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
    let missing = mara(
        fixture.path(),
        &["item", "move", "REQ-ABSENT", "source.mara.md"],
    );
    assert!(!missing.status.success());
    let moved = mara(
        fixture.path(),
        &[
            "item",
            "move",
            "REQ-MOVE",
            "destination.mara.md",
            "--line",
            "1",
        ],
    );
    assert!(moved.status.success(), "{}", stderr(&moved));
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        format!("{source}\n\n{destination}")
    );
    valid(fixture.path());
}

#[cfg(unix)]
// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn item_move_rejects_symlink_destinations_without_touching_sources() {
    use std::os::unix::fs::symlink;
    let (fixture, source, destination) = move_fixture();
    let external = support::fixture();
    symlink(external.path(), fixture.path().join("external")).unwrap();
    symlink(
        fixture.path().join("destination.mara.md"),
        fixture.path().join("link.mara.md"),
    )
    .unwrap();
    symlink(
        fixture.path().join("absent.mara.md"),
        fixture.path().join("dangling.mara.md"),
    )
    .unwrap();
    for path in ["external/new.mara.md", "link.mara.md", "dangling.mara.md"] {
        let moved = mara(fixture.path(), &["item", "move", "REQ-MOVE", path]);
        assert!(!moved.status.success());
    }
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        destination
    );
    assert!(!external.path().join("new.mara.md").exists());
}

fn reference_fixture(prefix: &str, body: &str, suffix: &str) -> (TempDir, String) {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    let source = format!(
        "{prefix}:::mara requirement REQ-ONE\n:mid: 01M1PXP2KG381MM1VNN6XC7S4M\n:title: One\n\n{body}\n:::\n{suffix}"
    );
    fs::write(fixture.path().join("a.mara.md"), &source).unwrap();
    (fixture, source)
}

// @mara checks DES-ITEM-MOVEMENT
#[test]
fn movement_preserves_incoming_and_carried_markdown_destinations() {
    for mcp in [false, true] {
        for (prefix, body, suffix, file, destination) in [
            (
                "[inside](#inner)\n\n",
                "# Inner\n\nContent.",
                "",
                "b.mara.md",
                None,
            ),
            (
                "# Outer\n\nContent.\n\n",
                "[carried](#outer)",
                "",
                "b.mara.md",
                None,
            ),
            (
                "<a name=\"outside\"></a>\n\nParagraph.\n\n",
                "[carried](a.mara.md#outside)",
                "",
                "nested/b.mara.md",
                None,
            ),
            (
                "# Local\n\nOriginal.\n\n",
                "[carried](#local)",
                "",
                "b.mara.md",
                Some("# Local\n\nDifferent.\n"),
            ),
            (
                "# One\n\nFirst.\n\n[dest]: #one\n\n",
                "[ref][dest]",
                "",
                "b.mara.md",
                Some("# Two\n\nSecond.\n\n[dest]: #two\n"),
            ),
            (
                "",
                "# Same\n\nFirst.",
                "\n# Same\n\nLast.\n\n[link](#same)\n",
                "a.mara.md",
                None,
            ),
        ] {
            let (fixture, source) = reference_fixture(prefix, body, suffix);
            let root = fixture.path();
            fs::create_dir(root.join("nested")).unwrap();
            if let Some(text) = destination {
                fs::write(root.join(file), text).unwrap();
            }
            valid(root);
            let error = move_item(root, mcp, "REQ-ONE", file, None).unwrap_err();
            assert!(error.contains("link") && error.contains("bytes"), "{error}");
            assert_eq!(fs::read_to_string(root.join("a.mara.md")).unwrap(), source);
            if file != "a.mara.md" {
                if let Some(text) = destination {
                    assert_eq!(fs::read_to_string(root.join(file)).unwrap(), text);
                } else {
                    assert!(!root.join(file).exists());
                }
            }
            assert!(!root.join(".mara/transaction.json").exists());
        }
    }
}

// @mara checks REQ-ITEM-MOVEMENT
// @mara checks DES-ITEM-MOVEMENT
#[test]
fn identity_links_and_contained_self_links_survive_cross_document_movement() {
    for mcp in [false, true] {
        let prefix = "[[REQ-ONE]] [[01M1PXP2KG381MM1VNN6XC7S4M]]\n\n";
        let (fixture, source) =
            reference_fixture(prefix, "# Inner\n\n[self](#inner) [[REQ-ONE]]", "");
        let root = fixture.path();
        let result = move_item(root, mcp, "01M1PXP2KG381MM1VNN6XC7S4M", "b.mara.md", None).unwrap();
        assert_eq!(result["mid"], "01M1PXP2KG381MM1VNN6XC7S4M");
        assert_eq!(result["new_location"], json!({"path":"b.mara.md","line":1}));
        assert_eq!(fs::read_to_string(root.join("a.mara.md")).unwrap(), prefix);
        assert_eq!(
            fs::read_to_string(root.join("b.mara.md")).unwrap(),
            source.strip_prefix(prefix).unwrap()
        );
        valid(root);
        let read = mara(root, &["--format", "json", "get", "REQ-ONE"]);
        assert!(read.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&read.stdout).unwrap()["node"]["source"]["path"],
            "b.mara.md"
        );
    }
}

// @mara checks DES-ITEM-MOVEMENT
#[test]
fn same_document_item_boundaries_preserve_content_and_location() {
    for mcp in [false, true] {
        let (fixture, source, _) = move_fixture();
        let root = fixture.path();
        for line in [1, source.lines().count() + 1] {
            let result = move_item(root, mcp, "REQ-MOVE", "source.mara.md", Some(line)).unwrap();
            assert_eq!(result["old_location"], result["new_location"]);
            assert_eq!(
                fs::read_to_string(root.join("source.mara.md")).unwrap(),
                source
            );
            assert!(!root.join(".mara/transaction.json").exists());
        }
    }
}

// @mara checks DES-ITEM-MOVEMENT
// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn pending_active_and_incomplete_projects_refuse_movement() {
    for mcp in [false, true] {
        let (fixture, source, destination) = move_fixture();
        let root = fixture.path();
        fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
        assert!(
            move_item(root, mcp, "REQ-MOVE", "new.mara.md", None)
                .unwrap_err()
                .contains("pending transaction")
        );
        fs::remove_file(root.join(".mara/transaction.json")).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(".mara/mutation.lock"))
            .unwrap();
        lock.lock().unwrap();
        assert!(
            move_item(root, mcp, "REQ-MOVE", "new.mara.md", None)
                .unwrap_err()
                .contains("another Mara mutation")
        );
        lock.unlock().unwrap();
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            destination
        );
        let incomplete = destination.replace("Reference [[REQ-MOVE]].", "");
        fs::write(root.join("destination.mara.md"), &incomplete).unwrap();
        assert!(
            move_item(root, mcp, "REQ-MOVE", "new.mara.md", None)
                .unwrap_err()
                .contains("required body is empty")
        );
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            incomplete
        );
        assert!(!root.join("new.mara.md").exists());
        assert!(!root.join(".mara/transaction.json").exists());
    }
}

// @mara checks REQ-ITEM-MOVEMENT
#[test]
fn inline_and_external_assertions_keep_spelling_identity_and_new_locations() {
    for mcp in [false, true] {
        let (fixture, source, destination) = move_fixture();
        let root = fixture.path();
        let schema = root.join(".mara/schema.yaml");
        fs::write(&schema,fs::read_to_string(&schema).unwrap()+"\n  tracked_by:\n    description: External ticket\n    source: [requirement]\n    target: []\n    external: true\n").unwrap();
        let source = source.replace(
            "Exact Unicode body: żółć.",
            "[[depends_on:REQ-STAY]] and [[tracked_by:external:HTTPS://Example.invalid/A%2fb#C]].",
        );
        fs::write(root.join("source.mara.md"), &source).unwrap();
        valid(root);
        assert!(move_item(root, mcp, "REQ-MOVE", "new.mara.md", None).is_ok());
        assert_eq!(
            fs::read_to_string(root.join("new.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            destination
        );
        for (name, target) in [
            ("depends_on", "REQ-STAY"),
            ("tracked_by", "external:HTTPS://Example.invalid/A%2fb#C"),
        ] {
            let result = mara(
                root,
                &[
                    "--format", "json", "relation", "get", "REQ-MOVE", name, target,
                ],
            );
            assert!(result.status.success());
            let result: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(result["occurrence_count"], 1);
            assert_eq!(result["occurrences"][0]["source"]["path"], "new.mara.md");
        }
        valid(root);
    }
}
