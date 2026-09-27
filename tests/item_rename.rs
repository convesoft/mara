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
    let diagnostics =
        mara::validate_corpus(&mara::load_corpus(&project, &schema).unwrap(), &schema);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
fn rename(root: &Path, mcp: bool, reference: &str, new_id: &str) -> Result<Value, String> {
    if mcp {
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "item_rename",
                    json!({"reference":reference,"new_id":new_id}),
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
        let output = mara(
            root,
            &["--format", "json", "item", "rename", reference, new_id],
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        if output.status.success() {
            Ok(result)
        } else {
            Err(result["error"]["message"].as_str().unwrap().into())
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

fn rename_fixture() -> (TempDir, String, String, String) {
    let (fixture, source, _) = move_fixture();
    let original: Value = serde_json::from_slice(
        &mara(fixture.path(), &["--format", "json", "get", "REQ-MOVE"]).stdout,
    )
    .unwrap();
    let mid = original["node"]["mid"].as_str().unwrap().to_owned();
    let source = source
        .replace(
            ":title: REQ-MOVE\r\n",
            ":title: REQ-MOVE\r\n:depends_on: REQ-MOVE\r\n",
        )
        .replace(
            "Exact Unicode body:",
            "Self [[REQ-MOVE]]. Exact Unicode body:",
        );
    let destination = fs::read_to_string(fixture.path().join("destination.mara.md"))
        .unwrap()
        .replace(
            ":depends_on: REQ-MOVE",
            &format!(":depends_on:\tREQ-MOVE  \n:depends_on: {mid}"),
        )
        .replace(
            "Reference [[REQ-MOVE]].",
            &format!(
                r#"Reference [[REQ-MOVE]], [[REQ-MOVE]] and [[{mid}]].
Unicode żółć REQ-MOVE prose; [REQ-MOVE](https://example.com/REQ-MOVE).
Literal `[[REQ-MOVE]]` and escaped \[[REQ-MOVE]].
Unsupported labelled syntax [[REQ-MOVE|REQ-MOVE]].

```markdown
[[REQ-MOVE]]
```

<!-- [[REQ-MOVE]] -->

<div>
[[REQ-MOVE]]
</div>
"#
            ),
        );
    let destination = format!("Narrative `[[REQ-MOVE]]`.\n\n{destination}\nTail REQ-MOVE");
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("destination.mara.md"), &destination).unwrap();
    fs::write(
        fixture.path().join("untouched.mara.md"),
        "REQ-MOVE `[[REQ-MOVE]]`\r\n",
    )
    .unwrap();
    valid(fixture.path());
    (fixture, source, destination, mid)
}

// @mara implements VER-ITEM-RENAME
// @mara checks REQ-ITEM-RENAME
// @mara checks DES-ITEM-RENAME
#[test]
fn item_rename_preserves_bytes_and_mid_graph_across_documents() {
    let (fixture, source, destination, mid) = rename_fixture();
    #[cfg(unix)]
    for (path, mode) in [("source.mara.md", 0o640), ("destination.mara.md", 0o604)] {
        fs::set_permissions(fixture.path().join(path), fs::Permissions::from_mode(mode)).unwrap();
    }
    let before_related = related_snapshot(fixture.path(), "REQ-MOVE");
    let before: Value =
        serde_json::from_slice(&mara(fixture.path(), &["--format", "json", "get", &mid]).stdout)
            .unwrap();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "rename",
            &mid,
            "REQ-RENAMED-LONGER",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result,
        json!({"mid":mid, "old_id":"REQ-MOVE", "new_id":"REQ-RENAMED-LONGER", "paths":["destination.mara.md","source.mara.md"]})
    );
    let expected_source = source
        .replace("requirement REQ-MOVE", "requirement REQ-RENAMED-LONGER")
        .replace(":depends_on: REQ-MOVE", ":depends_on: REQ-RENAMED-LONGER")
        .replace("Self [[REQ-MOVE]]", "Self [[REQ-RENAMED-LONGER]]");
    let expected_destination = destination
        .replace(":depends_on:\tREQ-MOVE", ":depends_on:\tREQ-RENAMED-LONGER")
        .replace(
            "Reference [[REQ-MOVE]], [[REQ-MOVE]]",
            "Reference [[REQ-RENAMED-LONGER]], [[REQ-RENAMED-LONGER]]",
        );
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        expected_source
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        expected_destination
    );
    assert_eq!(
        fs::read(fixture.path().join("untouched.mara.md")).unwrap(),
        b"REQ-MOVE `[[REQ-MOVE]]`\r\n"
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
    let after: Value = serde_json::from_slice(
        &mara(
            fixture.path(),
            &["--format", "json", "get", "REQ-RENAMED-LONGER"],
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(after["node"]["mid"], before["node"]["mid"]);
    assert_eq!(
        related_snapshot(fixture.path(), "REQ-RENAMED-LONGER"),
        before_related
    );
    assert!(!mara(fixture.path(), &["get", "REQ-MOVE"]).status.success());
    assert!(mara(fixture.path(), &["get", &mid]).status.success());
    valid(fixture.path());
    assert!(!fixture.path().join(".mara/transaction.json").exists());
    assert!(
        !isolated_command("git", fixture.path())
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

// @mara checks REQ-ITEM-RENAME
// @mara checks DES-ITEM-RENAME
#[test]
fn item_rename_cli_mcp_and_human_results_agree() {
    let (fixture, source, destination, mid) = rename_fixture();
    let output = mara(
        fixture.path(),
        &["--format", "json", "item", "rename", "REQ-MOVE", "REQ-X"],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected_source = fs::read(fixture.path().join("source.mara.md")).unwrap();
    let expected_destination = fs::read(fixture.path().join("destination.mara.md")).unwrap();
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("destination.mara.md"), &destination).unwrap();
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", fixture.path().to_str().unwrap()],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            mcp_call(2, "item_rename", json!({"reference":mid,"new_id":"REQ-X"})),
            mcp_call(
                3,
                "item_rename",
                json!({"reference":mid,"new_id":"REQ-X","project":fixture.path()}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        expected
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert_eq!(
        fs::read(fixture.path().join("source.mara.md")).unwrap(),
        expected_source
    );
    assert_eq!(
        fs::read(fixture.path().join("destination.mara.md")).unwrap(),
        expected_destination
    );
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_rename",
                json!({"project":fixture.path(),"reference":"REQ-X","new_id":"REQ-X"}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        json!({"mid":mid,"old_id":"REQ-X","new_id":"REQ-X","paths":[]})
    );
    let output = mara(fixture.path(), &["item", "rename", "REQ-X", "REQ-MOVE"]);
    assert!(output.status.success());
    for value in [
        &mid,
        "REQ-X",
        "REQ-MOVE",
        "source.mara.md",
        "destination.mara.md",
    ] {
        assert!(stdout(&output).contains(value));
    }
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        destination
    );
}

// @mara checks REQ-ITEM-RENAME
// @mara checks DES-ITEM-RENAME
#[test]
fn item_rename_rejections_leave_source_unchanged_with_cli_mcp_parity() {
    let (fixture, source, destination, mid) = rename_fixture();
    for (reference, new_id) in [
        ("REQ-MOVE", "bad"),
        ("REQ-MOVE", "DES-WRONG"),
        ("REQ-MOVE", "REQ-STAY"),
        ("REQ-MOVE", &mid),
        ("REQ-MOVE", "REQ-A\nREQ-B"),
        ("REQ-UNKNOWN", "REQ-X"),
    ] {
        let output = mara(
            fixture.path(),
            &["--format", "json", "item", "rename", reference, new_id],
        );
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "item_rename",
                    json!({"reference":reference,"new_id":new_id}),
                ),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(error["error"]["message"].as_str().unwrap())
        );
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
        fixture.path().join("destination.mara.md"),
        destination.replace("[[REQ-MOVE]]", "[[REQ-MISSING]]"),
    )
    .unwrap();
    let output = mara(fixture.path(), &["item", "rename", "REQ-MOVE", "REQ-X"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("validation fails"));
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
}

// @mara checks REQ-ITEM-RENAME
// @mara checks DES-ITEM-RENAME
#[test]
fn narrative_mentions_are_rewritten_but_heading_links_block_retargeting() {
    for mcp in [false, true] {
        let (fixture, source, destination, mid) = rename_fixture();
        let root = fixture.path();
        let narrative = format!(
            "[[REQ-MOVE]] [[{mid}]] `[[REQ-MOVE]]` \\[[REQ-MOVE]]\n\n<!-- [[REQ-MOVE]] -->\n"
        );
        fs::write(root.join("narrative.mara.md"), &narrative).unwrap();
        let heading_source = source.replace(
            "Exact Unicode body:",
            "\r\n\r\n# [[REQ-MOVE]]\r\n\r\nExact Unicode body:",
        );
        fs::write(root.join("source.mara.md"), &heading_source).unwrap();
        fs::write(
            root.join("links.mara.md"),
            "[heading](source.mara.md#req-move)\n",
        )
        .unwrap();
        valid(root);
        let error = rename(root, mcp, "REQ-MOVE", "REQ-NEW").unwrap_err();
        assert!(
            error.contains("untouched link") && error.contains("links.mara.md:1"),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            heading_source
        );
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            destination
        );
        assert_eq!(
            fs::read_to_string(root.join("narrative.mara.md")).unwrap(),
            narrative
        );
        fs::remove_file(root.join("links.mara.md")).unwrap();
        let result = rename(root, mcp, &mid, "REQ-NEW").unwrap();
        assert_eq!(result["mid"], mid);
        assert_eq!(
            fs::read_to_string(root.join("narrative.mara.md")).unwrap(),
            narrative.replacen("[[REQ-MOVE]]", "[[REQ-NEW]]", 1)
        );
        assert!(
            fs::read_to_string(root.join("source.mara.md"))
                .unwrap()
                .contains("# [[REQ-NEW]]")
        );
        valid(root);
    }
}

// @mara checks REQ-ITEM-RENAME
// @mara checks DES-ITEM-RENAME
#[test]
fn rename_preserves_inverse_symmetric_inline_mid_and_external_edges() {
    for mcp in [false, true] {
        let (fixture, source, destination, mid) = rename_fixture();
        let root = fixture.path();
        let schema = root.join(".mara/schema.yaml");
        let text = fs::read_to_string(&schema)
            .unwrap()
            .replace("  depends_on:\n", "  depends_on:\n    inverse: needed_by\n")
            + "\n  associated_with:\n    description: Association\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n  tracked_by:\n    description: External ticket\n    source: [requirement]\n    target: []\n    external: true\n";
        fs::write(schema, text).unwrap();
        let source=source.replace("Exact Unicode body:","[[associated_with:REQ-STAY]] [[tracked_by:external:HTTPS://Example.invalid/REQ-MOVE]]. Exact Unicode body:");
        let destination=destination.replace("Reference [[REQ-MOVE]],",&format!("[[needed_by:REQ-MOVE]] [[associated_with:REQ-MOVE]] [[needed_by:{mid}]] `[[needed_by:REQ-MOVE]]` \\[[needed_by:REQ-MOVE]]. Reference [[REQ-MOVE]],"));
        fs::write(root.join("source.mara.md"), &source).unwrap();
        fs::write(root.join("destination.mara.md"), &destination).unwrap();
        valid(root);
        let source_graph = related_snapshot(root, "REQ-MOVE");
        assert!(rename(root, mcp, "REQ-MOVE", "REQ-NEW").is_ok());
        let actual = fs::read_to_string(root.join("destination.mara.md")).unwrap();
        let expected = destination
            .replace(":depends_on:\tREQ-MOVE", ":depends_on:\tREQ-NEW")
            .replacen("[[needed_by:REQ-MOVE]]", "[[needed_by:REQ-NEW]]", 1)
            .replace(
                "[[associated_with:REQ-MOVE]]",
                "[[associated_with:REQ-NEW]]",
            )
            .replace(
                "Reference [[REQ-MOVE]], [[REQ-MOVE]]",
                "Reference [[REQ-NEW]], [[REQ-NEW]]",
            );
        assert_eq!(actual, expected);
        assert_eq!(related_snapshot(root, "REQ-NEW"), source_graph);
        let actual = fs::read_to_string(root.join("source.mara.md")).unwrap();
        assert!(actual.contains("external:HTTPS://Example.invalid/REQ-MOVE"));
        assert_eq!(
            actual,
            source
                .replace("requirement REQ-MOVE", "requirement REQ-NEW")
                .replace(":depends_on: REQ-MOVE", ":depends_on: REQ-NEW")
                .replace("Self [[REQ-MOVE]]", "Self [[REQ-NEW]]")
        );
        valid(root);
    }
}

// @mara checks DES-ITEM-RENAME
#[test]
fn code_markers_stay_read_only_and_mid_targets_survive_rename() {
    for mcp in [false, true] {
        let (fixture, source, destination, mid) = rename_fixture();
        let root = fixture.path();
        code_index::configure(root, "rust", &["rs"], true);
        let schema = root.join(".mara/schema.yaml");
        fs::write(&schema,fs::read_to_string(&schema).unwrap()+"\n  code_check:\n    description: Checks requirement\n    source: []\n    target: [requirement]\n    code_source: true\n").unwrap();
        let path = root.join("check.rs");
        let code = "// @mara code_check REQ-MOVE\nfn check() {}\n";
        fs::write(&path, code).unwrap();
        code_index::write_single(root, "rust", "check.rs", "check", "check().");
        valid(root);
        let error = rename(root, mcp, "REQ-MOVE", "REQ-NEW").unwrap_err();
        assert!(error.contains("check.rs:1"), "{error}");
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            destination
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), code);
        let code = code.replace("REQ-MOVE", &mid);
        fs::write(&path, &code).unwrap();
        code_index::write_single(root, "rust", "check.rs", "check", "check().");
        assert!(rename(root, mcp, "REQ-MOVE", "REQ-NEW").is_ok());
        assert_eq!(fs::read_to_string(&path).unwrap(), code);
        valid(root);
        let edge = mara(
            root,
            &[
                "--format",
                "json",
                "relation",
                "get",
                "code:check.rs::rust::check().",
                "code_check",
                "REQ-NEW",
            ],
        );
        assert!(edge.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&edge.stdout).unwrap()["occurrence_count"],
            1
        );
    }
}

// @mara checks DES-ITEM-RENAME
// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn no_op_requires_valid_source_and_available_writer_lock() {
    for mcp in [false, true] {
        let (fixture, source, destination, _) = rename_fixture();
        let root = fixture.path();
        let path = root.join("source.mara.md");
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&path).unwrap().ino()
        };
        assert_eq!(
            rename(root, mcp, "REQ-MOVE", "REQ-MOVE").unwrap()["paths"],
            json!([])
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        }
        fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
        assert!(
            rename(root, mcp, "REQ-MOVE", "REQ-MOVE")
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
            rename(root, mcp, "REQ-MOVE", "REQ-NEW")
                .unwrap_err()
                .contains("another Mara mutation")
        );
        lock.unlock().unwrap();
        let incomplete = source.replace(
            "Self [[REQ-MOVE]]. Exact Unicode body: żółć.\r\n\r\n`[[REQ-EXAMPLE]]`",
            "",
        );
        assert_ne!(incomplete, source);
        fs::write(&path, &incomplete).unwrap();
        assert!(
            rename(root, mcp, "REQ-MOVE", "REQ-MOVE")
                .unwrap_err()
                .contains("required body is empty")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), incomplete);
        assert_eq!(
            fs::read_to_string(root.join("destination.mara.md")).unwrap(),
            destination
        );
    }
}
