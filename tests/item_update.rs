use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, io::Write, path::Path, process::Stdio};
use tempfile::TempDir;
mod support;
use support::*;

fn diagnostics(root: &Path) -> Vec<mara::Diagnostic> {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let corpus = mara::load_corpus(&project, &schema).unwrap();
    mara::validate_corpus(&corpus, &schema)
}
fn valid(root: &Path) {
    assert!(diagnostics(root).is_empty());
}
fn mara_with_stdin(root: &Path, args: &[&str], text: &str) -> std::process::Output {
    let mut child = command(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn update_fixture() -> (TempDir, String, String) {
    let fixture = support::fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap().replace(
        "    id_prefix: REQ-\n    body: required\n    fields: {}",
        "    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        required: true\n        values: [draft, accepted]\n      tag:\n        type: string\n        repeatable: true\n      count:\n        type: integer\n      enabled:\n        type: boolean\n      weight:\n        type: number",
    );
    fs::write(schema_path, schema).unwrap();
    let created = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "create",
            "requirement",
            "REQ-EDIT",
            "edit.mara.md",
            "--title",
            "Original",
            "--field",
            "status=draft",
            "--body",
            "Original body.",
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));
    let result: Value = serde_json::from_slice(&created.stdout).unwrap();
    let mid = result["mid"].as_str().unwrap().to_owned();
    let neighbor = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-KEEP",
            "other.mara.md",
            "--title",
            "Keep",
            "--field",
            "status=draft",
            "--body",
            "Keep [[REQ-EDIT]].",
        ],
    );
    assert!(neighbor.status.success(), "{}", stderr(&neighbor));
    let source = format!(
        "# Before  \r\n\r\n:::mara requirement REQ-EDIT\r\n:mid: {mid}\r\n:title:  Original \t\r\n:tag:\told-one  \r\n:depends_on: REQ-KEEP\r\n:status: draft\t\r\n:tag: old-two\r\n \t\r\nOriginal **body**.\r\n\r\n:::\r\n\r\nAfter without newline"
    );
    fs::write(fixture.path().join("edit.mara.md"), &source).unwrap();
    valid(fixture.path());
    (fixture, source, mid)
}

// @mara implements VER-ITEM-UPDATE
// @mara checks REQ-ITEM-UPDATE
// @mara checks DES-ITEM-UPDATE
// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_preserves_source_and_permissions_while_replacing_repeated_fields() {
    let (fixture, source, mid) = update_fixture();
    let path = fixture.path().join("edit.mara.md");
    let other = fs::read(fixture.path().join("other.mara.md")).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let result = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "update",
            &mid,
            "--title",
            " New title ",
            "--field",
            "tag=first",
            "--field",
            "status=accepted",
            "--field",
            "tag=second",
            "--field",
            "tag=third",
            "--field",
            "count=42",
            "--field",
            "enabled=true",
            "--field",
            "weight=2.5",
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        result,
        json!({"id":"REQ-EDIT", "mid":mid, "path":"edit.mara.md",
        "changed_fields":["count","enabled","status","tag","title","weight"], "warnings":[]})
    );
    let expected = source
        .replace(":title:  Original \t", ":title:  New title \t")
        .replace(":tag:\told-one  ", ":tag:\tfirst  ")
        .replace(":status: draft\t", ":status: accepted\t")
        .replace(
            ":tag: old-two\r\n",
            ":tag: second\r\n:tag: third\r\n:count: 42\r\n:enabled: true\r\n:weight: 2.5\r\n",
        );
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    assert_eq!(
        fs::read(fixture.path().join("other.mara.md")).unwrap(),
        other
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let reduced = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--field", "tag=only"],
    );
    assert!(reduced.status.success(), "{}", stderr(&reduced));
    assert!(stdout(&reduced).contains(&mid));
    assert!(stdout(&reduced).contains("changed fields: tag"));
    assert!(stdout(&reduced).contains("edit.mara.md"));
    let expected = expected
        .replace(":tag:\tfirst  ", ":tag:\tonly  ")
        .replace(":tag: second\r\n", "")
        .replace(":tag: third\r\n", "");
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    let emptied = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--field", "tag="],
    );
    assert!(emptied.status.success(), "{}", stderr(&emptied));
    let expected = expected.replace(":tag:\tonly  ", ":tag:\t  ");
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    let cleared = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--clear-field", "tag"],
    );
    assert!(cleared.status.success(), "{}", stderr(&cleared));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        expected.replace(":tag:\t  \r\n", "")
    );
    valid(fixture.path());
    assert!(!fixture.path().join(".mara/transaction.json").exists());
}

// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_reads_body_from_stdin_and_handles_optional_empty_body_and_noops() {
    let (fixture, source, _) = update_fixture();
    let body = "New paragraph.\n\n```markdown\n:::mara is an example\n:::\n```\n[[REQ-KEEP]]";
    let result = mara_with_stdin(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--body", "-"],
        body,
    );
    assert!(result.status.success(), "{}", stderr(&result));
    let expected = source.replace(
        "Original **body**.\r\n\r\n",
        &(body.replace('\n', "\r\n") + "\r\n"),
    );
    let path = fixture.path().join("edit.mara.md");
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    let noop = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "update",
            "REQ-EDIT",
            "--title",
            "Original",
            "--clear-field",
            "count",
        ],
    );
    assert!(noop.status.success(), "{}", stderr(&noop));
    assert_eq!(
        serde_json::from_slice::<Value>(&noop.stdout).unwrap()["changed_fields"],
        json!([])
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path)
        .unwrap()
        .replace("body: required", "body: optional");
    fs::write(schema_path, schema).unwrap();
    let empty = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--body", ""],
    );
    assert!(empty.status.success(), "{}", stderr(&empty));
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        source.replace("Original **body**.\r\n\r\n", "")
    );
}

// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_allows_continued_drafting_and_completes_scaffolds() {
    let (fixture, source, _) = update_fixture();
    let created = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-DRAFT",
            "draft.mara.md",
            "--title",
            "Draft",
            "--field",
            "status=draft",
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));
    assert!(stdout(&created).contains("complete: false"));
    // Other existing scaffolds also remain available during incremental drafting.
    let result = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--title", "Changed"],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(
        fs::read_to_string(fixture.path().join("edit.mara.md")).unwrap(),
        source.replace("Original \t", "Changed \t")
    );
    let result = mara(
        fixture.path(),
        &[
            "item",
            "update",
            "REQ-DRAFT",
            "--title",
            "Working draft",
            "--field",
            "tag=planning",
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(stderr(&result).contains("warning: draft.mara.md:"));
    assert!(stderr(&result).contains("required body is empty"));
    let cli = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "update",
            "REQ-DRAFT",
            "--title",
            "Working draft",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli["warnings"].as_array().unwrap().len(), 1);
    assert_eq!(cli["warnings"][0]["path"], "draft.mara.md");
    assert_eq!(
        cli["warnings"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["line", "message", "path", "scope"]
    );
    assert_eq!(cli["warnings"][0]["scope"], "item");
    assert!(cli["warnings"][0]["line"].as_u64().unwrap() > 0);
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_update",
                json!({"reference":"REQ-DRAFT", "title":"Working draft"}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        cli
    );
    let missing = diagnostics(fixture.path());
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].code(), mara::DiagnosticCode::FieldInvalid);
    let draft = fs::read(fixture.path().join("draft.mara.md")).unwrap();
    let empty = mara(
        fixture.path(),
        &["item", "update", "REQ-DRAFT", "--body", ""],
    );
    assert!(!empty.status.success());
    assert_eq!(
        fs::read(fixture.path().join("draft.mara.md")).unwrap(),
        draft
    );
    let result = mara_with_stdin(
        fixture.path(),
        &["item", "update", "REQ-DRAFT", "--body", "-"],
        "Completed draft.\n",
    );
    assert!(result.status.success(), "{}", stderr(&result));
    valid(fixture.path());
}

// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_invalid_requests_preserve_all_files() {
    let (fixture, source, _) = update_fixture();
    let other = fs::read(fixture.path().join("other.mara.md")).unwrap();
    for args in [
        vec![],
        vec!["--title", " \t"],
        vec!["--title", "bad\nvalue"],
        vec!["--field", "status=unknown"],
        vec!["--field", "status=draft", "--field", "status=accepted"],
        vec!["--field", "count=no"],
        vec!["--field", "enabled=yes"],
        vec!["--field", "weight=no"],
        vec!["--field", "tag=bad\nvalue"],
        vec!["--field", "unknown=value"],
        vec!["--clear-field", "unknown"],
        vec!["--clear-field", "status"],
        vec!["--clear-field", "tag", "--field", "tag=value"],
        vec!["--field", "mid=01M1PXP2KG381MM1VNN6XC7S4M"],
        vec!["--clear-field", "mid"],
        vec!["--field", "id=REQ-RENAMED"],
        vec!["--field", "flavour=design"],
        vec!["--field", "title=Changed"],
        vec!["--field", "body=Changed"],
        vec!["--field", "depends_on=REQ-KEEP"],
        vec!["--clear-field", "depends_on"],
        vec!["--body", ""],
        vec!["--body", " \n"],
        vec!["--body", "[[REQ-MISSING]]"],
        vec!["--body", ":::\nEscaped"],
        vec!["--body", "```\nUnclosed fence"],
        vec![
            "--body",
            ":::mara requirement REQ-NESTED\n:title: Nested\n\nBody\n:::",
        ],
    ] {
        let mut command = vec!["--format", "json", "item", "update", "REQ-EDIT"];
        command.extend(args);
        let result = mara(fixture.path(), &command);
        assert!(!result.status.success(), "unexpected success: {command:?}");
        assert!(
            serde_json::from_slice::<Value>(&result.stdout).unwrap()["error"]["message"]
                .is_string()
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("edit.mara.md")).unwrap(),
            source,
            "{command:?}"
        );
        assert_eq!(
            fs::read(fixture.path().join("other.mara.md")).unwrap(),
            other
        );
    }
    for reference in ["REQ-MISSING", "req-edit", "01M1PXP2KG381MM1VNN6XC7S4M"] {
        assert!(
            !mara(
                fixture.path(),
                &["item", "update", reference, "--title", "Changed"]
            )
            .status
            .success()
        );
    }
    // A diagnostic containing scaffold-like text is not a missing-body exception.
    fs::write(
        fixture.path().join("other.mara.md"),
        String::from_utf8(other.clone())
            .unwrap()
            .replace(":status: draft", ":status: required body is empty"),
    )
    .unwrap();
    assert!(
        !mara(
            fixture.path(),
            &["item", "update", "REQ-EDIT", "--title", "Changed"]
        )
        .status
        .success()
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("edit.mara.md")).unwrap(),
        source
    );
    fs::write(fixture.path().join("other.mara.md"), other).unwrap();
    fs::write(fixture.path().join(".mara/transaction.json"), "pending").unwrap();
    let result = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--title", "Changed"],
    );
    assert!(!result.status.success());
    assert!(stderr(&result).contains("pending transaction"));
    assert_eq!(
        fs::read_to_string(fixture.path().join("edit.mara.md")).unwrap(),
        source
    );
}

// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_mcp_matches_cli_against_real_files() {
    let (fixture, source, mid) = update_fixture();
    let cli = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "update",
            "REQ-EDIT",
            "--title",
            "New",
            "--field",
            "tag=one",
            "--field",
            "tag=two",
            "--clear-field",
            "count",
            "--body",
            "Updated [[REQ-KEEP]].",
        ],
    );
    assert!(cli.status.success(), "{}", stderr(&cli));
    let expected = fs::read(fixture.path().join("edit.mara.md")).unwrap();
    fs::write(fixture.path().join("edit.mara.md"), &source).unwrap();
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_update",
                json!({"project":fixture.path(), "reference":mid, "title":"New", "fields":[{"key":"tag","value":"one"},{"key":"tag","value":"two"}], "clear_fields":["count"], "body":"Updated [[REQ-KEEP]]."}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        serde_json::from_slice::<Value>(&cli.stdout).unwrap()
    );
    assert_eq!(
        fs::read(fixture.path().join("edit.mara.md")).unwrap(),
        expected
    );
    for arguments in [
        json!({"reference":"REQ-EDIT"}),
        json!({"reference":"REQ-EDIT", "mid":mid, "title":"Rejected"}),
        json!({"reference":"REQ-EDIT", "fields":[{"key":"depends_on","value":"REQ-KEEP"}]}),
        json!({"reference":"REQ-EDIT", "body":"[[REQ-MISSING]]"}),
        json!({"reference":"REQ-EDIT", "project":fixture.path(), "title":"Rejected"}),
    ] {
        let responses = mcp_exchange_with_arguments(
            fixture.path(),
            &["mcp", "--project", fixture.path().to_str().unwrap()],
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "item_update", arguments),
            ],
        );
        let response = mcp_response(&responses, 2);
        assert!(
            response.get("error").is_some() || response["result"]["isError"] == true,
            "{response}"
        );
        assert_eq!(
            fs::read(fixture.path().join("edit.mara.md")).unwrap(),
            expected
        );
    }
}

// @mara checks REQ-ITEM-UPDATE
#[test]
fn item_update_rejects_missing_or_ambiguous_identity_and_preserves_adjacent_items() {
    let (fixture, source, mid) = update_fixture();
    let path = fixture.path().join("edit.mara.md");
    let other = fs::read_to_string(fixture.path().join("other.mara.md")).unwrap();
    // A same-document neighbor and a closing delimiter without a final newline.
    let adjacent = format!(
        "{other}\n{}",
        source.trim_end_matches("\r\n\r\nAfter without newline")
    );
    fs::remove_file(fixture.path().join("other.mara.md")).unwrap();
    fs::write(&path, &adjacent).unwrap();
    let result = mara(
        fixture.path(),
        &["item", "update", &mid, "--title", "Changed"],
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        adjacent.replace(":title:  Original", ":title:  Changed")
    );
    for invalid_source in [
        source.replace(&format!(":mid: {mid}\r\n"), ""),
        source.replace(&format!(":mid: {mid}"), ":mid: invalid"),
        format!(
            "{source}\n\n{}",
            source.replace("REQ-EDIT", "REQ-DUPLICATE")
        ),
        format!("{source}\n\n{}", other.replace("REQ-KEEP", "REQ-EDIT")),
    ] {
        fs::write(&path, &invalid_source).unwrap();
        let result = mara(
            fixture.path(),
            &["item", "update", "REQ-EDIT", "--title", "Changed"],
        );
        assert!(!result.status.success());
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid_source);
    }
}

fn reference_body_update(prefix: &str, body: &str, replacement: &str, succeeds: bool) {
    for use_mcp in [false, true] {
        let fixture = support::fixture();
        assert!(mara(fixture.path(), &["project", "init"]).status.success());
        let path = fixture.path().join("a.mara.md");
        let source = format!(
            "{prefix}:::mara requirement REQ-ONE\n:mid: 01M1PXP2KG381MM1VNN6XC7S4M\n:title: One\n\n{body}\n:::\n"
        );
        fs::write(&path, &source).unwrap();
        if use_mcp {
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(
                        2,
                        "item_update",
                        json!({"reference":"REQ-ONE", "body":replacement}),
                    ),
                ],
            );
            let result = &mcp_response(&responses, 2)["result"];
            assert_eq!(result["isError"], !succeeds, "{result}");
            if !succeeds {
                assert!(result.to_string().contains("a.mara.md:"), "{result}");
            }
        } else {
            let output = mara(
                fixture.path(),
                &["item", "update", "REQ-ONE", "--body", replacement],
            );
            assert_eq!(
                output.status.success(),
                succeeds,
                "{replacement:?}: {}",
                stderr(&output)
            );
            if !succeeds {
                assert!(stderr(&output).contains("a.mara.md:"));
            }
        }
        let after = fs::read_to_string(&path).unwrap();
        if succeeds {
            assert_eq!(
                after,
                source.replace(
                    &format!("\n\n{body}\n:::\n"),
                    &format!("\n\n{replacement}\n:::\n")
                )
            );
        } else {
            assert_eq!(after, source);
        }
        valid(fixture.path());
    }
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_protects_relocated_unchanged_links() {
    let body = "# Same\n\nFirst.\n\n# Same\n\nSecond.\n\n[link](#same)";
    reference_body_update("", body, "[link](#same)\n\n# Same\n\nSecond.", false);
    reference_body_update(
        "",
        body,
        "# Same\n\nSecond.\n\n# Same\n\nFirst.\n\n[link](#same)",
        false,
    );
    reference_body_update(
        "",
        body,
        "[link](#same)\n\n# Same\n\nFirst.\n\n# Same\n\nSecond.",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_explicit_definition_edits() {
    let body = "# One\n\nFirst.\n\n# Two\n\nSecond.\n\n[dest]: #one";
    for prefix in ["[ref][dest]\n\n", "[dest][] [dest]\n\n"] {
        reference_body_update(
            prefix,
            body,
            &body.replace("[dest]: #one", "[dest]: #two"),
            true,
        );
        reference_body_update(
            prefix,
            body,
            &body.replace("[dest]: #one", "[dest]: #absent"),
            false,
        );
    }
    let body = format!("[ref][dest]\n\n{body}");
    reference_body_update(
        "",
        &body,
        &body.replace("[dest]: #one", "[dest]: #two"),
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_prefix_edits_to_anchored_paragraphs() {
    let body = "<a name=\"stable\"></a>\n\nFirst sentence.";
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "<a name=\"stable\"></a>\n\nThe First sentence.",
        true,
    );
    // Inserting a separate paragraph really does retarget the anchor.
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "<a name=\"stable\"></a>\n\nDifferent paragraph.\n\nFirst sentence.",
        false,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_replacing_anchored_paragraph_prefixes() {
    reference_body_update(
        "[ref](#stable)\n\n",
        "<a name=\"stable\"></a>\n\nFirst sentence.",
        "<a name=\"stable\"></a>\n\nSecond sentence.",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_reordering_intact_unique_sections() {
    reference_body_update(
        "[ref](#alpha) [other](#beta)\n\n",
        "# Alpha\n\nAlpha text.\n\n# Beta\n\nBeta text.",
        "# Beta\n\nBeta text.\n\n# Alpha\n\nAlpha text.",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_explicit_literal_context_edits() {
    for link in ["[ref](#alpha)", "[[REQ-ONE]]"] {
        let body = format!("# Alpha\n\n{link}");
        for literal in [
            format!("`{link}`"),
            format!("```\n{link}\n```"),
            format!("\\{link}"),
        ] {
            reference_body_update("", &body, &format!("# Alpha\n\n{literal}"), true);
        }
    }
    // Literalizing one occurrence must not exempt another active link.
    reference_body_update(
        "",
        "# Same\n\nFirst.\n\n# Same\n\nSecond.\n\n[example](#same) [active](#same)",
        "# Same\n\nSecond.\n\n`[example](#same)` [active](#same)",
        false,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_complete_anchored_paragraph_replacement() {
    reference_body_update(
        "[ref](#stable)\n\n",
        "<a name=\"stable\"></a>\n\nYes",
        "<a name=\"stable\"></a>\n\nNo",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_rejects_punctuation_only_correspondence_across_slots() {
    let body = "<a name=\"stable\"></a>\n\nAAA.";
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "Inserted\n\n<a name=\"stable\"></a>\n\nBBB.",
        false,
    );
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "<a name=\"stable\"></a>\n\nBBB.",
        true,
    );
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "Inserted\n\n<a name=\"stable\"></a>\n\nAAA.",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_rejects_same_slot_when_old_text_survives_elsewhere() {
    let body = "<a name=\"stable\"></a>\n\nAAA";
    for (replacement, succeeds) in [
        ("<a name=\"stable\"></a>\n\nBBB\n\nAAAX", false),
        ("<a name=\"stable\"></a>\n\nAAAX\n\nBBB", true),
        ("<a name=\"stable\"></a>\n\nBBB", true),
    ] {
        reference_body_update("[ref](#stable)\n\n", body, replacement, succeeds);
    }
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_protects_anchors_on_duplicate_blocks() {
    let body = "<a name=\"stable\"></a>\n\nFirst.\n\nFirst.";
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "<a name=\"stable\"></a>\n\nInserted.\n\nFirst.\n\nFirst.",
        false,
    );
    // Repeated content alone must not prevent edits after the linked block.
    reference_body_update(
        "[ref](#stable)\n\n",
        body,
        "<a name=\"stable\"></a>\n\nFirst.\n\nInserted.\n\nFirst.",
        true,
    );
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_rejects_siblings_taking_an_anchored_blocks_slot() {
    for (original, sibling) in [("A", "B"), ("First.", "Second.")] {
        let body = format!("<a name=\"stable\"></a>\n\n{original}\n\n{sibling}");
        reference_body_update(
            "[ref](#stable)\n\n",
            &body,
            &format!("<a name=\"stable\"></a>\n\n{sibling}"),
            false,
        );
        // Rewriting the target completely is still allowed when the sibling stays put.
        reference_body_update(
            "[ref](#stable)\n\n",
            &body,
            &format!("<a name=\"stable\"></a>\n\nReplacement\n\n{sibling}"),
            true,
        );
    }
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_allows_section_extent_changes() {
    let nested = "# Alpha\n\nAlpha text.\n\n## Beta\n\nBeta text.";
    let siblings = "# Alpha\n\nAlpha text.\n\n# Beta\n\nBeta text.";
    reference_body_update("[ref](#alpha) [other](#beta)\n\n", nested, siblings, true);
    reference_body_update("[ref](#alpha) [other](#beta)\n\n", siblings, nested, true);
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn reference_review_rejects_replacing_a_renamed_unique_heading() {
    reference_body_update(
        "[ref](#alpha)\n\n",
        "# Alpha\n\nOld",
        "# Beta\n\nOld\n\n# Alpha\n\nNew",
        false,
    );
}

// @mara checks REQ-ITEM-UPDATE
// @mara checks DES-ITEM-UPDATE
#[test]
fn body_updates_author_inline_relations_and_reject_invalid_targets_without_writes() {
    for mcp in [false, true] {
        let (fixture, original, _) = update_fixture();
        let root = fixture.path();
        let path = root.join("edit.mara.md");
        let update = |body: &str, succeeds: bool| {
            if mcp {
                let replies = mcp_exchange(
                    root,
                    &[
                        mcp_initialize(1),
                        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                        mcp_call(
                            2,
                            "item_update",
                            json!({"reference":"REQ-EDIT","body":body}),
                        ),
                    ],
                );
                let response = &mcp_response(&replies, 2)["result"];
                assert_eq!(response["isError"], !succeeds, "{response}");
            } else {
                let output = mara(root, &["item", "update", "REQ-EDIT", "--body", body]);
                assert_eq!(output.status.success(), succeeds, "{}", stderr(&output));
            }
        };
        let body = "Repeated [[depends_on:REQ-KEEP]] and [[depends_on:REQ-KEEP]].";
        update(body, true);
        let written = fs::read(&path).unwrap();
        assert_eq!(
            String::from_utf8(written.clone()).unwrap(),
            original.replace("Original **body**.\r\n\r\n", &format!("{body}\r\n"))
        );
        let inspected = mara(
            root,
            &[
                "--format",
                "json",
                "relation",
                "get",
                "REQ-EDIT",
                "depends_on",
                "REQ-KEEP",
            ],
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&inspected.stdout).unwrap()["occurrence_count"],
            3
        );
        for body in [
            "[[unknown:REQ-KEEP]]",
            "[[depends_on:REQ-MISSING]]",
            "[[depends_on\n:REQ-KEEP]]",
            "[[\ndepends_on:REQ-KEEP]]",
        ] {
            update(body, false);
            assert_eq!(fs::read(&path).unwrap(), written);
        }
        // Explicitly literalizing inline assertions leaves metadata relations intact.
        update("`[[depends_on:REQ-KEEP]]`", true);
        let inspected = mara(
            root,
            &[
                "--format",
                "json",
                "relation",
                "get",
                "REQ-EDIT",
                "depends_on",
                "REQ-KEEP",
            ],
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&inspected.stdout).unwrap()["occurrence_count"],
            1
        );
        valid(root);
        // Preserve the old selected-item relation-resolution prerequisite.
        let broken = fs::read_to_string(&path)
            .unwrap()
            .replace("`[[depends_on:REQ-KEEP]]`", "[[depends_on:REQ-MISSING]]");
        fs::write(&path, &broken).unwrap();
        update("Repair body.", false);
        assert_eq!(fs::read_to_string(&path).unwrap(), broken);
    }
}

// @mara checks DES-ITEM-UPDATE
#[test]
fn mcp_body_dash_is_literal_null_is_omission_and_noops_do_not_replace_files() {
    let (fixture, source, _) = update_fixture();
    let root = fixture.path();
    let path = root.join("edit.mara.md");
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(&path).unwrap().ino()
    };
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_update",
                json!({"reference":"REQ-EDIT","title":null,"body":null,"clear_fields":["count"]}),
            ),
        ],
    );
    let response = &mcp_response(&replies, 2)["result"];
    assert_eq!(response["isError"], false);
    assert_eq!(response["structuredContent"]["changed_fields"], json!([]));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    }
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_update", json!({"reference":"REQ-EDIT","body":"-"})),
        ],
    );
    assert_eq!(mcp_response(&replies, 2)["result"]["isError"], false);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        source.replace("Original **body**.\r\n\r\n", "-\r\n")
    );
    let read = mara(root, &["--format", "json", "get", "REQ-EDIT"]);
    assert!(read.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&read.stdout).unwrap()["content"],
        "-\r\n"
    );
}
