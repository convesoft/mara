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
fn delete(root: &Path, mcp: bool, reference: &str) -> Result<Value, String> {
    if mcp {
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "item_delete", json!({"reference":reference})),
            ],
        );
        let response = &mcp_response(&responses, 2)["result"];
        if response["isError"] == true {
            Err(response["content"][0]["text"].as_str().unwrap().into())
        } else {
            Ok(response["structuredContent"].clone())
        }
    } else {
        let output = mara(root, &["--format", "json", "item", "delete", reference]);
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        if output.status.success() {
            Ok(result)
        } else {
            Err(result["error"]["message"].as_str().unwrap().into())
        }
    }
}
fn delete_fixture() -> (TempDir, String, String, String) {
    let fixture = support::fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let mut mid = String::new();
    for (id, file) in [
        ("REQ-DELETE", "delete.mara.md"),
        ("REQ-KEEP", "keep.mara.md"),
    ] {
        let output = mara(
            fixture.path(),
            &[
                "--format",
                "json",
                "item",
                "create",
                "requirement",
                id,
                file,
                "--title",
                id,
                "--body",
                "Exact Unicode body: żółć.",
            ],
        );
        assert!(output.status.success(), "{}", stdout(&output));
        if id == "REQ-DELETE" {
            mid = serde_json::from_slice::<Value>(&output.stdout).unwrap()["mid"]
                .as_str()
                .unwrap()
                .to_owned();
        }
    }
    let source = fs::read_to_string(fixture.path().join("delete.mara.md")).unwrap();
    let other = fs::read_to_string(fixture.path().join("keep.mara.md")).unwrap();
    (fixture, source, other, mid)
}

// @mara implements VER-ITEM-DELETION
// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn item_delete_preserves_source_permissions_and_empty_documents() {
    let (fixture, block, other, mid) = delete_fixture();
    let path = fixture.path().join("delete.mara.md");
    // Same-document survivor, CRLF, narrative, and no final newline.
    let source =
        format!("Before żółć.\n\n{block}\n{other}\nAfter without newline").replace('\n', "\r\n");
    fs::remove_file(fixture.path().join("keep.mara.md")).unwrap();
    fs::write(&path, &source).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let output = mara(
        fixture.path(),
        &["--format", "json", "item", "delete", &mid],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"id":"REQ-DELETE", "mid":mid, "path":"delete.mara.md"})
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!("Before żółć.\n\n{other}\nAfter without newline").replace('\n', "\r\n")
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    valid(fixture.path());
    assert!(!mara(fixture.path(), &["get", &mid]).status.success());
    // End/start boundaries and a closing delimiter without a final newline.
    for (source, expected) in [
        (block.clone(), String::new()),
        (block.trim_end_matches('\n').to_owned(), String::new()),
        (format!("{block}\nTail"), "\nTail".into()),
        (format!("\n{block}\nTail"), "\nTail".into()),
        (
            format!("Head\n\r\n{block}\r\nTail"),
            "Head\n\r\nTail".into(),
        ),
        (format!("Head\n\n{block}"), "Head\n\n".into()),
        (
            format!("Head\n\n\n{block}\n\nTail"),
            "Head\n\n\n\nTail".into(),
        ),
    ] {
        fs::write(&path, source).unwrap();
        let output = mara(fixture.path(), &["item", "delete", "REQ-DELETE"]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(stdout(&output).contains(&format!(
            "deleted item 'REQ-DELETE' with MID {mid} from delete.mara.md"
        )));
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
        assert!(path.is_file());
    }
}

// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn item_delete_reports_every_incoming_occurrence_with_cli_mcp_parity() {
    let (fixture, source, other, mid) = delete_fixture();
    let other = other
        .replace(
            ":title: REQ-KEEP",
            &format!(":title: REQ-KEEP\n:depends_on: REQ-DELETE\n:depends_on: {mid}"),
        )
        .replace(
            "Exact Unicode body: żółć.",
            &format!("[[REQ-DELETE]] [[{mid}]] [[REQ-DELETE]]"),
        );
    fs::write(fixture.path().join("keep.mara.md"), &other).unwrap();
    let third = other.replace("REQ-KEEP", "REQ-THIRD");
    // Use a generated identity for the second surviving document.
    let created = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "create",
            "requirement",
            "REQ-THIRD",
            "third.mara.md",
            "--title",
            "Third",
            "--body",
            "Third.",
        ],
    );
    assert!(created.status.success(), "{}", stdout(&created));
    let third_mid = serde_json::from_slice::<Value>(&created.stdout).unwrap()["mid"]
        .as_str()
        .unwrap()
        .to_owned();
    let keep_mid = other
        .lines()
        .find_map(|line| line.strip_prefix(":mid: "))
        .unwrap();
    let third = third.replace(keep_mid, &third_mid);
    fs::write(fixture.path().join("third.mara.md"), &third).unwrap();
    let narrative = "[[REQ-DELETE]]\n";
    fs::write(fixture.path().join("narrative.mara.md"), narrative).unwrap();
    valid(fixture.path());
    let cli = mara(
        fixture.path(),
        &["--format", "json", "item", "delete", &mid],
    );
    assert!(!cli.status.success());
    let result: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let error = result["error"]["message"].as_str().unwrap();
    assert_eq!(error.matches("(bytes ").count(), 11, "{error}");
    assert!(
        error.contains("narrative.mara.md:1 (bytes 0..14)"),
        "{error}"
    );
    for (file, body) in [("keep.mara.md", &other), ("third.mara.md", &third)] {
        for (offset, _) in body
            .match_indices(":depends_on:")
            .chain(body.match_indices("[["))
        {
            let line = body[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1;
            assert!(
                error.contains(&format!("{file}:{line} (bytes {offset}..")),
                "{error}"
            );
        }
    }
    let human = mara(fixture.path(), &["item", "delete", "REQ-DELETE"]);
    assert!(!human.status.success());
    assert!(stderr(&human).contains(error));
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_delete", json!({"reference":"REQ-DELETE"})),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(error)
    );
    for (file, expected) in [
        ("delete.mara.md", source),
        ("keep.mara.md", other),
        ("third.mara.md", third),
        ("narrative.mara.md", narrative.to_owned()),
    ] {
        assert_eq!(
            fs::read_to_string(fixture.path().join(file)).unwrap(),
            expected
        );
    }
}

// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn item_delete_ignores_outgoing_self_references_and_code_examples() {
    let (fixture, source, other, mid) = delete_fixture();
    let source = source.replace(":title: REQ-DELETE", &format!(":title: REQ-DELETE\n:depends_on: REQ-KEEP\n:depends_on: REQ-DELETE\n:depends_on: {mid}"))
        .replace("Exact Unicode body: żółć.", &format!("[[REQ-KEEP]] [[REQ-DELETE]] [[{mid}]]"));
    let other = other.replace("Exact Unicode body: żółć.", &format!("`[[REQ-DELETE]] [[{mid}]]`\n\n```text\n[[REQ-DELETE]] [[{mid}]]\n```\n\n\\[[REQ-DELETE]] \\[[{mid}]]"));
    let other = format!("Narrative `[[REQ-DELETE]] [[{mid}]]`.\n\n{other}");
    fs::write(fixture.path().join("delete.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("keep.mara.md"), &other).unwrap();
    valid(fixture.path());
    let cli = mara(
        fixture.path(),
        &["--format", "json", "item", "delete", "REQ-DELETE"],
    );
    assert!(cli.status.success(), "{}", stdout(&cli));
    fs::write(fixture.path().join("delete.mara.md"), &source).unwrap();
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_delete",
                json!({"project":fixture.path(), "reference":mid}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        serde_json::from_slice::<Value>(&cli.stdout).unwrap()
    );
    valid(fixture.path());
    assert_eq!(
        fs::read_to_string(fixture.path().join("delete.mara.md")).unwrap(),
        ""
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("keep.mara.md")).unwrap(),
        other
    );
}

// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn item_delete_refuses_invalid_projects_and_invalid_requests_without_source_changes() {
    let (fixture, source, other, mid) = delete_fixture();
    let path = fixture.path().join("delete.mara.md");
    let other_path = fixture.path().join("keep.mara.md");
    for invalid_other in [
        other.replace("Exact Unicode body: żółć.", "[[REQ-MISSING]]"),
        other.replace(
            "Exact Unicode body: żółć.",
            "[[00000000000000000000000000]]",
        ),
        other.replace("Exact Unicode body: żółć.", ""),
        other
            .lines()
            .filter(|line| !line.starts_with(":mid:"))
            .collect::<Vec<_>>()
            .join("\n"),
        format!("{other}\n{}", source.replace("REQ-DELETE", "REQ-DUPLICATE")),
        other.replace("REQ-KEEP", "REQ-DELETE"),
        other.replace(":::mara requirement", ":::mara unknown"),
        other.trim_end_matches(":::\n").to_owned(),
    ] {
        fs::write(&other_path, &invalid_other).unwrap();
        let result = mara(fixture.path(), &["item", "delete", &mid]);
        assert!(!result.status.success(), "{invalid_other}");
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        assert_eq!(fs::read_to_string(&other_path).unwrap(), invalid_other);
    }
    fs::write(&other_path, &other).unwrap();
    for reference in ["REQ-MISSING", "req-delete", "00000000000000000000000000"] {
        assert!(
            !mara(fixture.path(), &["item", "delete", reference])
                .status
                .success()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
    for arguments in [
        json!({"reference":"REQ-DELETE", "force":true}),
        json!({"reference":mid, "project":fixture.path()}),
        json!({"id":"REQ-DELETE"}),
    ] {
        let responses = mcp_exchange_with_arguments(
            fixture.path(),
            &["mcp", "--project", fixture.path().to_str().unwrap()],
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "item_delete", arguments),
            ],
        );
        let response = mcp_response(&responses, 2);
        assert!(
            response.get("error").is_some() || response["result"]["isError"] == true,
            "{response}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
    for file in [".mara/project.toml", ".mara/schema.yaml"] {
        let config_path = fixture.path().join(file);
        let config = fs::read(&config_path).unwrap();
        fs::write(&config_path, "invalid: [").unwrap();
        assert!(
            !mara(fixture.path(), &["item", "delete", &mid])
                .status
                .success()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        fs::write(config_path, config).unwrap();
    }
}

// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn deletion_rejects_shifted_heading_and_contained_block_targets() {
    for mcp in [false, true] {
        for newline in ["\n", "\r\n"] {
            let (fixture, block, _, _) = delete_fixture();
            let root = fixture.path();
            let path = root.join("delete.mara.md");
            let heading = block.replace("Exact Unicode body: żółć.", "# Same\n\nDeleted.");
            let source = format!("[one](#same) [two](#same)\n\n{heading}\n# Same\n\nSurvivor.\n")
                .replace('\n', newline);
            fs::write(&path, &source).unwrap();
            valid(root);
            let error = delete(root, mcp, "REQ-DELETE").unwrap_err();
            assert_eq!(error.matches("untouched link").count(), 2, "{error}");
            assert!(error.contains("bytes") && error.contains("delete.mara.md:1"));
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            let block = block.replace(
                "Exact Unicode body: żółć.",
                "<a name=\"stable\"></a>\n\nParagraph.",
            );
            let source = block.replace('\n', newline);
            fs::write(&path, &source).unwrap();
            let links = "[contained](delete.mara.md#stable)\n";
            fs::write(root.join("links.mara.md"), links).unwrap();
            valid(root);
            assert!(
                delete(root, mcp, "REQ-DELETE")
                    .unwrap_err()
                    .contains("links.mara.md:1")
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            assert_eq!(
                fs::read_to_string(root.join("links.mara.md")).unwrap(),
                links
            );
            fs::remove_file(root.join("links.mara.md")).unwrap();
            // A Markdown self-link disappears with its own target and does not block.
            let source = source.replace("Paragraph.", "Paragraph [self](#stable).");
            fs::write(&path, &source).unwrap();
            assert!(delete(root, mcp, "REQ-DELETE").is_ok());
            assert_eq!(fs::read_to_string(&path).unwrap(), "");
            valid(root);
        }
    }
}

// @mara checks REQ-ITEM-DELETION
#[test]
fn demoted_inline_relations_still_block_deletion_as_mentions() {
    for mcp in [false, true] {
        let (fixture, source, other, mid) = delete_fixture();
        let root = fixture.path();
        let other = other.replace(
            "Exact Unicode body: żółć.",
            &format!("[[depends_on:REQ-DELETE]] and [[depends_on:{mid}]]."),
        );
        fs::write(root.join("keep.mara.md"), &other).unwrap();
        valid(root);
        assert!(
            delete(root, mcp, "REQ-DELETE")
                .unwrap_err()
                .contains("depends_on")
        );
        assert_eq!(
            fs::read_to_string(root.join("delete.mara.md")).unwrap(),
            source
        );
        let removed = mara(
            root,
            &["relation", "remove", "REQ-KEEP", "depends_on", "REQ-DELETE"],
        );
        assert!(removed.status.success(), "{}", stderr(&removed));
        let demoted = other.replace("[[depends_on:", "[[");
        assert_eq!(
            fs::read_to_string(root.join("keep.mara.md")).unwrap(),
            demoted
        );
        let error = delete(root, mcp, "REQ-DELETE").unwrap_err();
        assert_eq!(error.matches("(bytes ").count(), 2, "{error}");
        assert_eq!(
            fs::read_to_string(root.join("delete.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.join("keep.mara.md")).unwrap(),
            demoted
        );
        // Deleting the source drops its outgoing mentions, permitting target deletion.
        assert!(delete(root, mcp, "REQ-KEEP").is_ok());
        assert!(delete(root, mcp, &mid).is_ok());
        assert_eq!(fs::read_to_string(root.join("keep.mara.md")).unwrap(), "");
        let list = mara(root, &["--format", "json", "item", "list"]);
        assert!(list.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&list.stdout).unwrap()["items"],
            json!([])
        );
        valid(root);
    }
}

// @mara checks DES-ITEM-DELETION
#[test]
fn deletion_allows_external_outgoing_assertions_and_retains_whitespace_only_lines() {
    for mcp in [false, true] {
        let (fixture, block, _, _) = delete_fixture();
        let root = fixture.path();
        let path = root.join("delete.mara.md");
        let schema = root.join(".mara/schema.yaml");
        fs::write(&schema,fs::read_to_string(&schema).unwrap()+"\n  tracked_by:\n    description: External ticket\n    source: [requirement]\n    target: []\n    external: true\n").unwrap();
        let block = block
            .replace(
                ":title: REQ-DELETE",
                ":title: REQ-DELETE\n:tracked_by: external:https://example.invalid/A",
            )
            .replace(
                "Exact Unicode body: żółć.",
                "[[tracked_by:external:HTTPS://example.invalid/B]]",
            );
        for (source, expected) in [
            (format!("Before\n \t\n{block}\nTail"), "Before\n \t\n\nTail"),
            (format!("Before\n\n{block} \t\nTail"), "Before\n\n \t\nTail"),
        ] {
            fs::write(&path, &source).unwrap();
            valid(root);
            assert!(delete(root, mcp, "REQ-DELETE").is_ok());
            assert_eq!(fs::read_to_string(&path).unwrap(), expected);
        }
    }
}

// @mara checks DES-ITEM-DELETION
#[test]
fn active_and_pending_writers_block_deletion() {
    for mcp in [false, true] {
        let (fixture, source, _, _) = delete_fixture();
        let root = fixture.path();
        fs::write(root.join(".mara/transaction.json"), "pending").unwrap();
        assert!(
            delete(root, mcp, "REQ-DELETE")
                .unwrap_err()
                .contains("pending transaction")
        );
        assert_eq!(
            fs::read_to_string(root.join("delete.mara.md")).unwrap(),
            source
        );
        fs::remove_file(root.join(".mara/transaction.json")).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(".mara/mutation.lock"))
            .unwrap();
        lock.lock().unwrap();
        assert!(
            delete(root, mcp, "REQ-DELETE")
                .unwrap_err()
                .contains("another Mara mutation")
        );
        assert_eq!(
            fs::read_to_string(root.join("delete.mara.md")).unwrap(),
            source
        );
        lock.unlock().unwrap();
    }
}

// @mara checks REQ-ITEM-DELETION
// @mara checks DES-ITEM-DELETION
#[test]
fn code_markers_prevent_target_deletion_without_editing_code() {
    for mcp in [false, true] {
        let (fixture, source, _, mid) = delete_fixture();
        let root = fixture.path();
        let config = root.join(".mara/project.toml");
        let text = fs::read_to_string(&config).unwrap().replacen(
            "format_version = 1",
            "format_version = 3",
            1,
        ) + "\n[[code.languages]]\nname = \"rust\"\nextensions = [\"rs\"]\ngrammar = \".mara/code/rust.wasm\"\nquery = \".mara/code/rust.scm\"\nseparator = \"::\"\n";
        fs::write(config, text).unwrap();
        fs::create_dir(root.join(".mara/code")).unwrap();
        for name in ["rust.wasm", "rust.scm"] {
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(".mara/code")
                    .join(name),
                root.join(".mara/code").join(name),
            )
            .unwrap();
        }
        let schema = root.join(".mara/schema.yaml");
        fs::write(&schema,fs::read_to_string(&schema).unwrap()+"\n  code_check:\n    description: Checks requirement\n    source: []\n    target: [requirement]\n    code_source: true\n").unwrap();
        let path = root.join("check.rs");
        for reference in ["REQ-DELETE", mid.as_str()] {
            let code = format!("// @mara code_check {reference}\nfn check() {{}}\n");
            fs::write(&path, &code).unwrap();
            valid(root);
            let error = delete(root, mcp, "REQ-DELETE").unwrap_err();
            assert!(error.contains("check.rs:1"), "{error}");
            assert_eq!(
                fs::read_to_string(root.join("delete.mara.md")).unwrap(),
                source
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), code);
            assert!(!root.join(".mara/transaction.json").exists());
        }
        fs::write(&path, "fn check() {}\n").unwrap();
        assert!(delete(root, mcp, "REQ-DELETE").is_ok());
        valid(root);
    }
}
