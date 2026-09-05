use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use mara::resolve_project;
use serde_json::{Value, json};
use tempfile::TempDir;

fn mara(current_directory: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mara"))
        .current_dir(current_directory)
        .args(arguments)
        .output()
        .expect("run Mara CLI")
}

fn mara_with_stdin(
    current_directory: &Path,
    arguments: &[&str],
    input: &str,
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mara"))
        .current_dir(current_directory)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run Mara CLI");
    child
        .stdin
        .take()
        .expect("capture Mara stdin")
        .write_all(input.as_bytes())
        .expect("write Mara stdin");
    child.wait_with_output().expect("read Mara CLI output")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn is_mid(value: &str) -> bool {
    value.len() == 26
        && value.chars().all(|character| {
            matches!(
                character,
                '0'..='9'
                    | 'A'..='H'
                    | 'J'..='K'
                    | 'M'..='N'
                    | 'P'..='T'
                    | 'V'..='Z'
            )
        })
}

fn mcp_exchange(current_directory: &Path, requests: &[Value]) -> Vec<Value> {
    mcp_exchange_with_arguments(current_directory, &["mcp"], requests)
}

fn mcp_exchange_with_arguments(
    current_directory: &Path,
    arguments: &[&str],
    requests: &[Value],
) -> Vec<Value> {
    let input = requests
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let output = mara_with_stdin(current_directory, arguments, &input);
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP response is JSON"))
        .collect()
}

fn mcp_request(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn mcp_initialize(id: u64) -> Value {
    mcp_request(
        id,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "mara-test", "version": "1" },
        }),
    )
}

fn mcp_call(id: u64, name: &str, arguments: Value) -> Value {
    mcp_request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

fn mcp_response(responses: &[Value], id: u64) -> &Value {
    responses
        .iter()
        .find(|response| response["id"] == id)
        .unwrap_or_else(|| panic!("missing MCP response {id}"))
}

#[test]
fn initializes_the_current_directory_without_touching_existing_content() {
    let fixture = TempDir::new().unwrap();
    let existing = fixture.path().join("README.md");
    fs::write(&existing, "keep me\n").unwrap();

    let output = mara(fixture.path(), &["project", "init"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.path().join(".mara/project.toml").is_file());
    assert!(fixture.path().join(".mara/schema.yaml").is_file());
    assert_eq!(fs::read_to_string(existing).unwrap(), "keep me\n");
    let schema = fs::read_to_string(fixture.path().join(".mara/schema.yaml")).unwrap();
    for flavour in ["scenario", "requirement", "design", "decision"] {
        assert!(schema.contains(&format!("  {flavour}:\n")));
    }
}

#[test]
fn initializes_a_named_missing_or_existing_directory() {
    let fixture = TempDir::new().unwrap();
    let missing = fixture.path().join("missing");
    let output = mara(fixture.path(), &["project", "init", "missing"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(missing.join(".mara/project.toml").is_file());

    let existing = fixture.path().join("existing");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("notes.txt"), "untouched").unwrap();
    let output = mara(fixture.path(), &["project", "init", "existing"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(existing.join("notes.txt")).unwrap(),
        "untouched"
    );

    let explicit = fixture.path().join("explicit");
    let output = mara(
        fixture.path(),
        &["project", "init", "--project", explicit.to_str().unwrap()],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(explicit.join(".mara/project.toml").is_file());
}

#[test]
fn rejects_ambiguous_initialization_targets() {
    let fixture = TempDir::new().unwrap();

    let output = mara(
        fixture.path(),
        &["project", "init", "named", "--project", "explicit"],
    );

    assert!(!output.status.success());
    assert!(stderr(&output).contains("cannot be used together"));
    assert!(!fixture.path().join("named").exists());
    assert!(!fixture.path().join("explicit").exists());
}

#[test]
fn refuses_to_overwrite_an_existing_project_or_target_file() {
    let fixture = TempDir::new().unwrap();
    let first = mara(fixture.path(), &["project", "init"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let original_project = fs::read(fixture.path().join(".mara/project.toml")).unwrap();

    let repeated = mara(fixture.path(), &["project", "init"]);

    assert!(!repeated.status.success());
    assert!(stderr(&repeated).contains("already exists"));
    assert_eq!(
        fs::read(fixture.path().join(".mara/project.toml")).unwrap(),
        original_project
    );

    let conflict = fixture.path().join("conflict");
    fs::create_dir_all(conflict.join(".mara")).unwrap();
    fs::write(conflict.join(".mara/schema.yaml"), "do not replace\n").unwrap();
    let output = mara(fixture.path(), &["project", "init", "conflict"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(conflict.join(".mara/schema.yaml")).unwrap(),
        "do not replace\n"
    );
    assert!(!conflict.join(".mara/project.toml").exists());
}

#[test]
fn empty_template_creates_no_project_flavours() {
    let fixture = TempDir::new().unwrap();

    let output = mara(fixture.path(), &["project", "init", "--template", "empty"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(fixture.path().join(".mara/schema.yaml")).unwrap(),
        "format_version: 1\nflavours: {}\nrelations: {}\n"
    );
}

#[test]
fn real_cli_initializes_projects_resolved_by_nearest_and_explicit_roots() {
    let fixture = TempDir::new().unwrap();
    let outer = fixture.path().join("outer");
    let nested = outer.join("nested");
    fs::create_dir_all(&nested).unwrap();
    for root in [&outer, &nested] {
        let output = mara(fixture.path(), &["project", "init", root.to_str().unwrap()]);
        assert!(output.status.success(), "{}", stderr(&output));
    }
    let deep = nested.join("a/b");
    fs::create_dir_all(&deep).unwrap();

    let discovered = resolve_project(None, &deep).unwrap();
    assert_eq!(discovered.root(), nested.canonicalize().unwrap());

    let explicit = resolve_project(Some(Path::new("../../..")), &deep).unwrap();
    assert_eq!(explicit.root(), outer.canonicalize().unwrap());
}

#[test]
fn rejects_non_project_relative_content_patterns() {
    for pattern in ["../**/*.mara.md", "/tmp/**/*.mara.md"] {
        let fixture = TempDir::new().unwrap();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let project_path = fixture.path().join(".mara/project.toml");
        let source = fs::read_to_string(&project_path).unwrap();
        fs::write(&project_path, source.replace("**/*.mara.md", pattern)).unwrap();

        let error = resolve_project(None, fixture.path()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("content.include entries must be project-relative patterns"),
            "{error}"
        );
    }
}

#[test]
fn current_directory_content_patterns_discover_project_documents() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("**/*.mara.md", "./**/*.mara.md"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("included.mara.md"),
        ":::mara requirement REQ-INCLUDED\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Included\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-INCLUDED"]);

    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid item 'REQ-INCLUDED'"));
}

#[test]
fn project_and_item_validation_run_through_the_real_cli() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Valid\n\nA complete requirement.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid project"));

    let item = mara(fixture.path(), &["item", "validate", "REQ-VALID"]);
    assert!(item.status.success(), "{}", stderr(&item));
    assert!(stdout(&item).contains("valid item 'REQ-VALID'"));
}

#[test]
fn project_validation_treats_mid_as_structural_metadata() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        r#":::mara requirement REQ-VALID
:mid: 01JQZ4W7G5H8K2M3N6P9R0STVX
:title: Valid

A complete requirement.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(validate.status.success(), "{}", stderr(&validate));
    assert!(stdout(&validate).contains("valid project"));
}

#[test]
fn project_validation_reports_missing_malformed_duplicate_and_misplaced_mids() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-MISSING
:title: Missing MID

Body.
:::

:::mara requirement REQ-MALFORMED
:mid: not-a-mid
:title: Malformed MID

Body.
:::

:::mara requirement REQ-OVERFLOW
:mid: ZZZZZZZZZZZZZZZZZZZZZZZZZZ
:title: Overflow MID

Body.
:::

:::mara requirement REQ-DUPLICATE-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate one

Body.
:::

:::mara requirement REQ-DUPLICATE-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Duplicate two

Body.
:::

:::mara requirement REQ-MISPLACED
:title: Misplaced MID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item 'REQ-MISSING' is missing its MID",
        "invalid item MID 'not-a-mid'",
        "invalid item MID 'ZZZZZZZZZZZZZZZZZZZZZZZZZZ'",
        "duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F00'",
        "item 'REQ-MISPLACED' MID must immediately follow its opener",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_rejects_non_bijective_item_identities() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Shared ID one

Body.
:::

:::mara requirement REQ-SHARED-ID
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Shared ID two

Body.
:::

:::mara requirement REQ-SHARED-MID-ONE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID one

Body.
:::

:::mara requirement REQ-SHARED-MID-TWO
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Shared MID two

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert_eq!(
        errors.matches("duplicate item ID 'REQ-SHARED-ID'").count(),
        2,
        "{errors}"
    );
    assert_eq!(
        errors
            .matches("duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F02'")
            .count(),
        2,
        "{errors}"
    );
}

#[test]
fn project_validation_reports_the_duplicated_secondary_mid_entry() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: First

Body.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["--format", "json", "project", "validate"]);

    assert!(!validate.status.success());
    assert!(stderr(&validate).is_empty(), "{}", stderr(&validate));
    let validate: Value = serde_json::from_str(&stdout(&validate)).unwrap();
    let duplicate_mids = validate["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| {
            diagnostic["message"] == "duplicate item MID '01ARZ3NDEKTSV4RRFFQ69G5F01'"
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_mids.len(), 2, "{validate:#}");
    assert!(
        duplicate_mids
            .iter()
            .any(|diagnostic| diagnostic["line"] == 3),
        "{validate:#}"
    );
    assert!(
        duplicate_mids
            .iter()
            .any(|diagnostic| diagnostic["line"] == 10),
        "{validate:#}"
    );
}

#[test]
fn project_mid_backfill_is_deliberate_preflighted_and_idempotent() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = fixture.path().join("legacy.mara.md");
    fs::write(
        &path,
        r#":::mara scenario SCN-LEGACY
:title: Legacy scenario

Legacy.
:::

:::mara requirement REQ-LEGACY
:title: Legacy requirement
:derives_from: SCN-LEGACY

Legacy body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(!validate.status.success());
    assert!(stderr(&validate).contains("is missing its MID"));

    let backfill = mara(
        fixture.path(),
        &["--format", "json", "project", "mid", "backfill"],
    );
    assert!(backfill.status.success(), "{}", stderr(&backfill));
    let backfill: Value = serde_json::from_str(&stdout(&backfill)).unwrap();
    let changed = backfill["changed"].as_array().unwrap();
    assert_eq!(changed.len(), 2);
    assert_eq!(changed[0]["id"], "SCN-LEGACY");
    assert_eq!(changed[0]["line"], 2);
    assert_eq!(changed[1]["id"], "REQ-LEGACY");
    assert_eq!(changed[1]["line"], 9);
    assert!(
        changed
            .iter()
            .all(|entry| is_mid(entry["mid"].as_str().unwrap()))
    );

    let source = fs::read_to_string(&path).unwrap();
    assert!(source.contains(":::mara scenario SCN-LEGACY\n:mid: "));
    assert!(source.contains(":::mara requirement REQ-LEGACY\n:mid: "));
    assert!(source.contains(":derives_from: SCN-LEGACY"));
    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));

    let again = mara(
        fixture.path(),
        &["--format", "json", "project", "mid", "backfill"],
    );
    assert!(again.status.success(), "{}", stderr(&again));
    let again: Value = serde_json::from_str(&stdout(&again)).unwrap();
    assert!(again["changed"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);

    fs::write(
        &path,
        r#":::mara requirement REQ-BROKEN
:mid: invalid
:title: Broken

Body.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let rejected = mara(fixture.path(), &["project", "mid", "backfill"]);
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("cannot backfill MIDs while validation fails"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn project_mid_backfill_preflight_does_not_match_user_text_as_missing_mid() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: REQ-\n    body: required\n    fields: {}",
            "    id_prefix: REQ-\n    body: required\n    fields:\n      blocked:\n        type: boolean",
        ),
    )
    .unwrap();
    let path = fixture.path().join("legacy.mara.md");
    fs::write(
        &path,
        r#":::mara requirement REQ-LEGACY
:title: Legacy requirement
:blocked: bad is missing its MID

Legacy body.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();

    let rejected = mara(fixture.path(), &["project", "mid", "backfill"]);

    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("invalid boolean value 'bad is missing its MID'"),
        "{}",
        stderr(&rejected)
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn mcp_project_mid_backfill_backfills_a_selected_project() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("legacy.mara.md"),
        ":::mara requirement REQ-LEGACY\n:title: Legacy\n\nBody.\n:::\n",
    )
    .unwrap();

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_mid_backfill", json!({})),
            mcp_call(3, "project_validate", json!({})),
        ],
    );

    let backfill = &mcp_response(&responses, 2)["result"]["structuredContent"];
    assert_eq!(backfill["changed"].as_array().unwrap().len(), 1);
    assert!(is_mid(backfill["changed"][0]["mid"].as_str().unwrap()));
    assert_eq!(
        mcp_response(&responses, 3)["result"]["structuredContent"]["valid"],
        true
    );
}

#[test]
fn item_taking_operations_resolve_mids_but_author_relations_as_human_ids() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        r#":::mara scenario SCN-TARGET
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Target

Target.
:::

:::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Source

Source.
:::
"#,
    )
    .unwrap();

    let get = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "get",
            "01ARZ3NDEKTSV4RRFFQ69G5F01",
        ],
    );
    assert!(get.status.success(), "{}", stderr(&get));
    let item: Value = serde_json::from_str(&stdout(&get)).unwrap();
    assert_eq!(item["summary"]["id"], "REQ-SOURCE");
    assert_eq!(item["summary"]["mid"], "01ARZ3NDEKTSV4RRFFQ69G5F01");

    let list = mara(fixture.path(), &["--format", "json", "item", "list"]);
    assert!(list.status.success(), "{}", stderr(&list));
    let list: Value = serde_json::from_str(&stdout(&list)).unwrap();
    assert_eq!(list["items"][0]["id"], "SCN-TARGET");
    assert_eq!(list["items"][0]["mid"], "01ARZ3NDEKTSV4RRFFQ69G5F00");

    let search = mara(
        fixture.path(),
        &["--format", "json", "item", "search", "source"],
    );
    assert!(search.status.success(), "{}", stderr(&search));
    let search: Value = serde_json::from_str(&stdout(&search)).unwrap();
    assert_eq!(search["items"][0]["id"], "REQ-SOURCE");
    assert_eq!(search["items"][0]["mid"], "01ARZ3NDEKTSV4RRFFQ69G5F01");

    let add = mara(
        fixture.path(),
        &[
            "relation",
            "add",
            "01ARZ3NDEKTSV4RRFFQ69G5F01",
            "derives_from",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
        ],
    );
    assert!(add.status.success(), "{}", stderr(&add));
    let source = fs::read_to_string(fixture.path().join("items.mara.md")).unwrap();
    assert!(source.contains(":derives_from: SCN-TARGET"));
    assert!(!source.contains(":derives_from: 01ARZ3NDEKTSV4RRFFQ69G5F00"));
}

#[test]
fn relation_traversal_resolves_authored_mids_as_item_identity() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        r#":::mara scenario SCN-TARGET
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Target

Target.
:::

:::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Source
:derives_from: 01ARZ3NDEKTSV4RRFFQ69G5F00

Source.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);
    assert!(validate.status.success(), "{}", stderr(&validate));

    let get = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "get",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
        ],
    );
    assert!(get.status.success(), "{}", stderr(&get));
    let item: Value = serde_json::from_str(&stdout(&get)).unwrap();
    assert_eq!(item["incoming_relations"][0]["item"]["id"], "REQ-SOURCE");

    let related = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "related",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
        ],
    );
    assert!(related.status.success(), "{}", stderr(&related));
    let related: Value = serde_json::from_str(&stdout(&related)).unwrap();
    assert_eq!(related["items"][0]["item"]["id"], "REQ-SOURCE");
    assert_eq!(related["items"][0]["direction"], "incoming");
}

#[test]
fn incoming_relation_traversal_rejects_ambiguous_human_id_targets() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        r#":::mara requirement REQ-DUP
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: First

First.
:::

:::mara requirement REQ-DUP
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Second.
:::

:::mara design DES-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Source
:satisfies: REQ-DUP

Source.
:::
"#,
    )
    .unwrap();

    let get = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "get",
            "01ARZ3NDEKTSV4RRFFQ69G5F00",
        ],
    );

    assert!(!get.status.success());
    let error: Value = serde_json::from_str(&stdout(&get)).unwrap();
    assert_eq!(
        error["error"]["message"],
        "relation 'satisfies' from 'DES-SOURCE' references ambiguous item 'REQ-DUP'"
    );
}

#[test]
fn project_validation_reports_all_independently_available_diagnostics() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-RELATION

Mentions [[MISSING-MENTION]].
:::

:::mara mystery MYS-UNKNOWN
:title: Unknown

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "unknown metadata field 'unknown'",
        "references missing item 'MISSING-RELATION'",
        "mention references missing item 'MISSING-MENTION'",
        "unknown flavour 'mystery'",
        "validation failed with 7 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }

    let item = mara(fixture.path(), &["item", "validate", "WRONG-ID"]);
    assert!(!item.status.success());
    assert!(stderr(&item).contains("validation failed with 5 diagnostics"));
}

#[test]
fn item_validation_reports_ambiguous_relation_and_mention_targets() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::
"#,
    )
    .unwrap();
    for name in ["first", "second"] {
        fs::write(
            fixture.path().join(format!("{name}.mara.md")),
            format!(":::mara requirement REQ-TARGET\n:title: {name}\n\nTarget body.\n:::\n"),
        )
        .unwrap();
    }

    let validate = mara(fixture.path(), &["item", "validate", "REQ-SOURCE"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "relation 'derives_from' references ambiguous item 'REQ-TARGET'",
        "mention references ambiguous item 'REQ-TARGET'",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn item_validation_retains_recovered_syntax_diagnostics() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("nested.mara.md"),
        r#":::mara requirement REQ-OUTER
:title: Outer

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-INNER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("nested.mara.md:4: error: items cannot nest"),
        "{errors}"
    );
}

#[test]
fn item_validation_rejects_nested_opener_after_an_early_outer_error() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("nested.mara.md"),
        r#":::mara requirement REQ-OUTER

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-INNER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("nested.mara.md:3: error: items cannot nest"),
        "{errors}"
    );
    assert!(
        !stdout(&validate).contains("valid item"),
        "{}",
        stdout(&validate)
    );
}

#[test]
fn project_validation_reports_schema_and_independent_syntax_diagnostics() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn item_validation_reports_syntax_for_an_identifiable_malformed_item() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN\n\nBody without a title.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-BROKEN"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors
            .contains("broken.mara.md:1: error: item must have exactly one non-empty title entry"),
        "{errors}"
    );
    assert!(
        !errors.contains("item 'REQ-BROKEN' was not found"),
        "{errors}"
    );
}

#[test]
fn item_validation_associates_a_malformed_opener_with_its_id() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("broken.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-BROKEN"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(errors.contains("item opener must be"), "{errors}");
    assert!(
        !errors.contains("item 'REQ-BROKEN' was not found"),
        "{errors}"
    );
}

#[test]
fn project_validation_continues_after_invalid_utf8() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(fixture.path().join("first.mara.md"), [0xff]).unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "first.mara.md:1: error: could not read Mara document",
        "second.mara.md:1: error: item opener must be",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn item_validation_fails_when_an_included_document_is_unreadable() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:title: Valid\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(fixture.path().join("bad.mara.md"), [0xff]).unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-VALID"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("bad.mara.md:1: error: could not read Mara document"),
        "{errors}"
    );
    assert!(
        !stdout(&validate).contains("valid item"),
        "{}",
        stdout(&validate)
    );
}

#[test]
fn item_validation_associates_a_missing_close_with_the_outer_item() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("missing-close.mara.md"),
        r#":::mara requirement REQ-OUTER
:title: Outer

Outer body.

:::mara requirement REQ-INNER
:title: Inner

Inner body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-OUTER"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(errors.contains("items cannot nest"), "{errors}");
    assert!(
        !errors.contains("item 'REQ-OUTER' was not found"),
        "{errors}"
    );
}

#[test]
fn project_validation_accumulates_independent_schema_diagnostics() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema
            .replace("id_prefix: REQ-", "id_prefix: REQ--")
            .replace("id_prefix: SCN-", "id_prefix: SCN--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "flavour 'scenario' has invalid ID prefix 'SCN--'",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_uses_unaffected_schema_declarations() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: SCN-", "id_prefix: SCN--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'scenario' has invalid ID prefix 'SCN--'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_runs_schema_independent_checks_after_schema_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nMentions [[MISSING-MENTION]].\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: Second\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "duplicate item ID 'REQ-DUPLICATE'",
        "mention references missing item 'MISSING-MENTION'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_accumulates_independent_configuration_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/project.toml"),
        r#"format_version = 1

[project]
name = ""
schema = ".mara/schema.yaml"

[content]
include = ["../**/*.mara.md"]
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "project.name must not be empty",
        "content.include entries must be project-relative patterns",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_retains_valid_include_entries_after_a_type_error() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace(
            "include = [\"**/*.mara.md\"]",
            "include = [\"recovered.mara.md\", 42]",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("recovered.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Recovered\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid project configuration value 'content.include[1]'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "validation failed with 3 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_retains_item_context_after_title_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        r#":::mara requirement REQ-DUPLICATE
:title: Second
:title: Duplicate title
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "item must have exactly one non-empty title entry",
        "duplicate item ID 'REQ-DUPLICATE'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 8 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_uses_declarations_unaffected_by_schema_decode_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    body: required\n    fields: {}\n  requirement:",
            "    body: invalid\n    fields: {}\n  requirement:",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value
:derives_from: MISSING-TARGET

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'scenario' is invalid",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'MISSING-TARGET'",
        "validation failed with 6 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_uses_properties_unaffected_by_a_flavour_decode_error() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/schema.yaml"),
        r#"format_version: 1
flavours:
  requirement:
    description: An independently verifiable obligation.
    id_prefix: REQ-
    body: invalid
    fields:
      count:
        type: integer
relations: {}
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:count: nope

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "flavour 'requirement' is invalid",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "invalid integer value 'nope' for field 'count'",
        "validation failed with 4 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_uses_flavours_when_the_relations_section_is_malformed() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join(".mara/schema.yaml"),
        r#"format_version: 1
flavours:
  requirement:
    description: An independently verifiable obligation.
    id_prefix: REQ-
    body: required
    fields:
      count:
        type: integer
relations: invalid
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:count: nope

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid schema configuration value 'relations'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "invalid integer value 'nope' for field 'count'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_retains_known_configuration_after_unknown_keys() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("[project]\n", "[project]\nunexpected = true\n"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "unknown project configuration key 'project.unexpected'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_retains_schema_after_unknown_root_keys() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(&schema_file, format!("unexpected: true\n{schema}")).unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        r#":::mara requirement WRONG-ID
:title: Invalid
:unknown: value

:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "unknown schema configuration key 'unexpected'",
        "item ID 'WRONG-ID' must start with 'REQ-'",
        "required body is empty",
        "unknown metadata field 'unknown'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_uses_fields_unaffected_by_configuration_type_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    let configured_name = fixture.path().file_name().unwrap().to_str().unwrap();
    fs::write(
        &project_file,
        project.replace(&format!("name = \"{configured_name}\""), "name = 42"),
    )
    .unwrap();
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid project configuration value 'project.name'",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_retains_item_identity_after_metadata_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("first.mara.md"),
        ":::mara requirement REQ-DUPLICATE\n:title: First\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("second.mara.md"),
        r#":::mara requirement REQ-DUPLICATE
:title: Second
:malformed

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid metadata entry",
        "duplicate item ID 'REQ-DUPLICATE'",
        "validation failed with 5 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_checks_metadata_recovered_before_an_error() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("partial.mara.md"),
        r#":::mara requirement REQ-PARTIAL
:title: Partial
:unknown: value
:derives_from: REQ-MISSING
:malformed

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "invalid metadata entry",
        "unknown metadata field 'unknown'",
        "relation 'derives_from' references missing item 'REQ-MISSING'",
        "validation failed with 4 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_checks_title_errors_proven_before_malformed_metadata() {
    for metadata in [
        ":title:\n:malformed",
        ":title: First\n:title: Second\n:malformed",
    ] {
        let fixture = TempDir::new().unwrap();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        fs::write(
            fixture.path().join("partial.mara.md"),
            format!(":::mara requirement REQ-PARTIAL\n{metadata}\n\nBody.\n:::\n"),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["project", "validate"]);

        assert!(!validate.status.success());
        let errors = stderr(&validate);
        for expected in [
            "invalid metadata entry",
            "item must have exactly one non-empty title entry",
            "validation failed with 3 diagnostics",
        ] {
            assert!(
                errors.contains(expected),
                "missing {expected:?} in {errors}"
            );
        }
    }
}

#[test]
fn project_validation_does_not_infer_missing_targets_after_item_parse_failures() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::

:::mara requirement REQ-TARGET trailing
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("item opener must be ':::mara <flavour> <id>' with no other tokens"),
        "{errors}"
    );
    assert!(
        !errors.contains("references missing item 'REQ-TARGET'"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

#[test]
fn project_validation_retains_opener_semantics_after_a_missing_close() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("complete.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Complete\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("partial.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Partial\n\nBody without a close.\n",
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("item is missing its closing delimiter"),
        "{errors}"
    );
    assert_eq!(
        errors.matches("duplicate item ID 'WRONG-ID'").count(),
        2,
        "{errors}"
    );
    assert_eq!(
        errors
            .matches("item ID 'WRONG-ID' must start with 'REQ-'")
            .count(),
        2,
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 7 diagnostics"),
        "{errors}"
    );
}

#[test]
fn project_validation_does_not_infer_missing_targets_after_include_recovery() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace(
            "include = [\"**/*.mara.md\"]",
            "include = [\"source.mara.md\", \"[\"]",
        ),
    )
    .unwrap();
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::
"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("target.mara.md"),
        r#":::mara requirement REQ-TARGET
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("invalid content.include pattern '['"),
        "{errors}"
    );
    assert!(
        !errors.contains("references missing item 'REQ-TARGET'"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

#[test]
fn item_validation_fails_when_incomplete_corpus_recovery_skips_context_checks() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("source.mara.md"),
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:derives_from: REQ-TARGET

Mentions [[REQ-TARGET]].
:::

:::mara requirement REQ-TARGET trailing
:title: Target

Body.
:::
"#,
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-SOURCE"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains(
            "item 'REQ-SOURCE' could not be fully validated because the project corpus is incomplete"
        ),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
    assert!(!stdout(&validate).contains("valid item"));
}

#[cfg(unix)]
#[test]
fn project_validation_continues_after_directory_walk_errors() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("valid.mara.md"),
        ":::mara requirement REQ-VALID\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Valid\n\nMentions [[MISSING-TARGET]].\n:::\n",
    )
    .unwrap();
    let unreadable = fixture.path().join("unreadable");
    fs::create_dir(&unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "could not discover Mara documents",
        "item opener must be ':::mara <flavour> <id>' with no other tokens",
        "validation failed with 2 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
    assert!(
        !errors.contains("mention references missing item 'MISSING-TARGET'"),
        "{errors}"
    );
}

#[test]
fn item_validation_reports_configuration_that_prevents_reliable_discovery() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("**/*.mara.md", "../**/*.mara.md"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-MISSING"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("content.include entries must be project-relative patterns"),
        "{errors}"
    );
    assert!(
        !errors.contains("item 'REQ-MISSING' was not found"),
        "{errors}"
    );
}

#[test]
fn item_validation_reports_context_errors_and_a_proven_missing_item() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    let configured_name = fixture.path().file_name().unwrap().to_str().unwrap();
    fs::write(
        &project_file,
        project.replace(&format!("name = \"{configured_name}\""), "name = \"\""),
    )
    .unwrap();
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["item", "validate", "REQ-MISSING"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    for expected in [
        "project.name must not be empty",
        "flavour 'requirement' has invalid ID prefix 'REQ--'",
        "item 'REQ-MISSING' was not found",
        "validation failed with 3 diagnostics",
    ] {
        assert!(
            errors.contains(expected),
            "missing {expected:?} in {errors}"
        );
    }
}

#[test]
fn project_validation_does_not_invent_a_schema_version_after_decode_failure() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(&schema_file, schema.replacen("format_version: 1\n", "", 1)).unwrap();

    let validate = mara(fixture.path(), &["project", "validate"]);

    assert!(!validate.status.success());
    let errors = stderr(&validate);
    assert!(
        errors.contains("schema configuration key 'format_version' is required"),
        "{errors}"
    );
    assert!(
        !errors.contains("unsupported schema format version 0"),
        "{errors}"
    );
    assert!(
        errors.contains("validation failed with 1 diagnostic"),
        "{errors}"
    );
}

#[test]
fn real_cli_discovers_and_inspects_the_effective_minimal_schema() {
    let fixture = TempDir::new().unwrap();
    let project_root = fixture.path().join("project");
    let init = mara(
        fixture.path(),
        &["project", "init", project_root.to_str().unwrap()],
    );
    assert!(init.status.success(), "{}", stderr(&init));
    let nested = project_root.join("nested/deeper");
    fs::create_dir_all(&nested).unwrap();

    let complete = mara(&nested, &["schema", "get"]);
    assert!(complete.status.success(), "{}", stderr(&complete));
    let complete = stdout(&complete);
    assert!(complete.contains("format_version: 1"));
    assert!(complete.contains("requirement:"));
    assert!(complete.contains("satisfies:"));

    let flavours = mara(&nested, &["schema", "list", "flavour"]);
    assert!(flavours.status.success(), "{}", stderr(&flavours));
    let flavours = stdout(&flavours);
    assert!(flavours.contains("requirement\tAn independently verifiable obligation."));
    assert!(flavours.contains("design\tA solution or interface contract"));

    let relations = mara(&nested, &["schema", "list", "relation"]);
    assert!(relations.status.success(), "{}", stderr(&relations));
    let relations = stdout(&relations);
    assert!(relations.contains("derives_from\tThe source originates"));
    assert!(relations.contains("supersedes\tThe source replaces"));

    let relation = mara(&nested, &["schema", "get", "relation", "satisfies"]);
    assert!(relation.status.success(), "{}", stderr(&relation));
    let relation = stdout(&relation);
    assert!(relation.starts_with("satisfies:\n"));
    assert!(relation.contains("source:\n  - design"));
    assert!(relation.contains("target:\n  - requirement"));

    let valid = mara(&nested, &["schema", "validate"]);
    assert!(valid.status.success(), "{}", stderr(&valid));
    assert!(stdout(&valid).contains("valid schema"));
}

#[test]
fn schema_commands_load_the_schema_configured_by_the_selected_project() {
    let fixture = TempDir::new().unwrap();
    let selected = fixture.path().join("selected");
    let init = mara(
        fixture.path(),
        &["project", "init", selected.to_str().unwrap()],
    );
    assert!(init.status.success(), "{}", stderr(&init));

    let project_file = selected.join(".mara/project.toml");
    let project_source = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project_source.replace(".mara/schema.yaml", ".mara/custom.yaml"),
    )
    .unwrap();
    fs::write(
        selected.join(".mara/custom.yaml"),
        r#"format_version: 1
flavours:
  note:
    description: A concise project note.
    id_prefix: NOTE-
    body: optional
    fields:
      text:
        type: string
      count:
        type: integer
      ratio:
        type: number
      enabled:
        type: boolean
      status:
        type: enum
        required: true
        repeatable: false
        values: [draft, accepted]
relations:
  depends_on:
    description: The source requires the target.
    source: [note]
    target: [note]
"#,
    )
    .unwrap();

    let note = mara(
        fixture.path(),
        &[
            "--project",
            selected.to_str().unwrap(),
            "schema",
            "get",
            "flavour",
            "note",
        ],
    );
    assert!(note.status.success(), "{}", stderr(&note));
    let note = stdout(&note);
    assert!(note.starts_with("note:\n"));
    assert!(note.contains("id_prefix: NOTE-"));
    assert!(note.contains("type: enum"));
    assert!(note.contains("values:\n      - draft\n      - accepted"));
    assert!(!note.contains("values: null"));

    let valid = mara(
        fixture.path(),
        &[
            "--project",
            selected.to_str().unwrap(),
            "schema",
            "validate",
        ],
    );
    assert!(valid.status.success(), "{}", stderr(&valid));
    assert!(stdout(&valid).contains(".mara/custom.yaml"));
}

#[test]
fn schema_validation_rejects_unknown_relation_endpoints() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("target: [scenario, requirement]", "target: [missing]"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate)
            .contains("relation 'derives_from' target references unknown flavour 'missing'"),
        "{}",
        stderr(&validate)
    );
}

#[test]
fn schema_validation_rejects_an_id_prefix_with_an_empty_segment() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace("id_prefix: REQ-", "id_prefix: REQ--"),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate).contains("flavour 'requirement' has invalid ID prefix 'REQ--'"),
        "{}",
        stderr(&validate)
    );
}

#[test]
fn schema_validation_rejects_structural_names_as_custom_fields() {
    for field in ["mid", "flavour", "id", "title", "body"] {
        let fixture = TempDir::new().unwrap();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace(
                "    id_prefix: REQ-\n    body: required\n    fields: {}",
                &format!(
                    "    id_prefix: REQ-\n    body: required\n    fields:\n      {field}:\n        type: string"
                ),
            ),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(!validate.status.success(), "field '{field}' was accepted");
        assert!(
            stderr(&validate).contains(&format!(
                "flavour 'requirement' field '{field}' is reserved for item structure"
            )),
            "{}",
            stderr(&validate)
        );
    }
}

#[test]
fn schema_validation_rejects_enum_values_with_surrounding_whitespace() {
    for value in [" draft ", "   "] {
        let fixture = TempDir::new().unwrap();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace(
                "    id_prefix: REQ-\n    body: required\n    fields: {}",
                &format!(
                    "    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        values: [\"{value}\"]"
                ),
            ),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(
            !validate.status.success(),
            "enum value '{value}' was accepted"
        );
        assert!(
            stderr(&validate).contains(
                "flavour 'requirement' enum field 'status' values must not have surrounding whitespace"
            ),
            "{}",
            stderr(&validate)
        );
    }
}

#[test]
fn schema_validation_rejects_structural_names_as_relations() {
    for relation in ["mid", "flavour", "id", "title", "body"] {
        let fixture = TempDir::new().unwrap();
        let init = mara(fixture.path(), &["project", "init"]);
        assert!(init.status.success(), "{}", stderr(&init));
        let schema_file = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_file).unwrap();
        fs::write(
            &schema_file,
            schema.replace("  derives_from:\n", &format!("  {relation}:\n")),
        )
        .unwrap();

        let validate = mara(fixture.path(), &["schema", "validate"]);

        assert!(
            !validate.status.success(),
            "relation '{relation}' was accepted"
        );
        assert!(
            stderr(&validate).contains(&format!(
                "relation '{relation}' is reserved for item structure"
            )),
            "{}",
            stderr(&validate)
        );
    }
}

#[test]
fn schema_validation_rejects_relation_and_source_field_name_collisions() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: SCN-\n    body: required\n    fields: {}",
            "    id_prefix: SCN-\n    body: required\n    fields:\n      depends_on:\n        type: string",
        ),
    )
    .unwrap();

    let validate = mara(fixture.path(), &["schema", "validate"]);

    assert!(!validate.status.success());
    assert!(
        stderr(&validate).contains(
            "relation 'depends_on' conflicts with field 'depends_on' on source flavour 'scenario'"
        ),
        "{}",
        stderr(&validate)
    );
}

#[test]
fn schema_get_rejects_an_unknown_declaration() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));

    let get = mara(fixture.path(), &["schema", "get", "flavour", "missing"]);

    assert!(!get.status.success());
    assert!(stderr(&get).contains("unknown flavour 'missing'"));
}

#[test]
fn item_create_writes_complete_items_and_required_body_scaffolds() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: REQ-\n    body: required\n    fields: {}",
            "    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        required: true\n        values: [draft, accepted]\n      tag:\n        type: string\n        repeatable: true",
        ),
    )
    .unwrap();
    fs::create_dir(fixture.path().join("docs")).unwrap();

    let missing_field = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-MISSING-FIELD",
            "docs/missing.mara.md",
            "--title",
            "Missing field",
            "--body",
            "Body.",
        ],
    );
    assert!(!missing_field.status.success());
    assert!(stderr(&missing_field).contains("required field 'status' is missing"));
    assert!(!fixture.path().join("docs/missing.mara.md").exists());

    let complete = mara_with_stdin(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-COMPLETE",
            "docs/items.mara.md",
            "--title",
            "Complete item",
            "--field",
            "status=draft",
            "--field",
            "tag=alpha",
            "--field",
            "tag=primary",
            "--body",
            "-",
        ],
        "Created from standard input.\n",
    );

    assert!(complete.status.success(), "{}", stderr(&complete));
    assert!(stdout(&complete).contains("created item 'REQ-COMPLETE'"));
    assert!(stdout(&complete).contains("complete: true"));
    let source = fs::read_to_string(fixture.path().join("docs/items.mara.md")).unwrap();
    let lines = source.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], ":::mara requirement REQ-COMPLETE");
    assert!(lines[1].strip_prefix(":mid: ").is_some_and(is_mid));
    assert_eq!(
        lines[2..],
        [
            ":title: Complete item",
            ":status: draft",
            ":tag: alpha",
            ":tag: primary",
            "",
            "Created from standard input.",
            ":::"
        ]
    );
    let valid = mara(fixture.path(), &["item", "validate", "REQ-COMPLETE"]);
    assert!(valid.status.success(), "{}", stderr(&valid));

    let scaffold = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-SCAFFOLD",
            "docs/items.mara.md",
            "--title",
            "Scaffolded item",
            "--field",
            "status=draft",
        ],
    );

    assert!(scaffold.status.success(), "{}", stderr(&scaffold));
    assert!(stdout(&scaffold).contains("complete: false"));
    assert!(stdout(&scaffold).contains("missing: body"));
    let source = fs::read_to_string(fixture.path().join("docs/items.mara.md")).unwrap();
    assert!(source.contains(":::\n\n:::mara requirement REQ-SCAFFOLD\n:mid: "));
    assert!(source.contains(":title: Scaffolded item\n:status: draft\n\n:::\n"));
    let invalid = mara(fixture.path(), &["item", "validate", "REQ-SCAFFOLD"]);
    assert!(!invalid.status.success());
    assert!(stderr(&invalid).contains("required body is empty"));
}

#[test]
fn item_create_inserts_at_an_explicit_safe_line_without_corrupting_the_source() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = fixture.path().join("notes.mara.md");
    fs::write(&path, "# Notes\n\nBefore.\n\nAfter.\n").unwrap();
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

    let missing_parent = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-NO-PARENT",
            "missing/items.mara.md",
            "--title",
            "No parent",
            "--body",
            "Body.",
        ],
    );
    assert!(!missing_parent.status.success());
    assert!(!fixture.path().join("missing").exists());

    let insert = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-INSERTED",
            "notes.mara.md",
            "--title",
            "Inserted item",
            "--body",
            "Inserted body.",
            "--line",
            "5",
        ],
    );

    assert!(insert.status.success(), "{}", stderr(&insert));
    let source = fs::read_to_string(&path).unwrap();
    let lines = source.lines().collect::<Vec<_>>();
    assert_eq!(
        lines[0..5],
        [
            "# Notes",
            "",
            "Before.",
            "",
            ":::mara requirement REQ-INSERTED"
        ]
    );
    assert!(lines[5].strip_prefix(":mid: ").is_some_and(is_mid));
    assert_eq!(
        lines[6..],
        [
            ":title: Inserted item",
            "",
            "Inserted body.",
            ":::",
            "",
            "After."
        ]
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let valid = mara(fixture.path(), &["project", "validate"]);
    assert!(valid.status.success(), "{}", stderr(&valid));

    let inside_item = source
        .lines()
        .position(|line| line == ":title: Inserted item")
        .unwrap()
        + 1;
    let rejected = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-UNSAFE",
            "notes.mara.md",
            "--title",
            "Unsafe item",
            "--body",
            "Unsafe body.",
            "--line",
            &inside_item.to_string(),
        ],
    );

    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("inside item 'REQ-INSERTED'"));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);

    let structurally_invalid = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-BROKEN",
            "notes.mara.md",
            "--title",
            "Broken item",
            "--body",
            ":::mara requirement REQ-NESTED\n:title: Nested item\n\nNested.\n:::",
        ],
    );
    assert!(!structurally_invalid.status.success());
    assert!(stderr(&structurally_invalid).contains("items cannot nest"));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn item_create_rejects_destinations_excluded_from_project_discovery() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("**/*.mara.md", "docs/**/*.mara.md"),
    )
    .unwrap();
    fs::create_dir(fixture.path().join("docs")).unwrap();
    fs::write(fixture.path().join(".gitignore"), "docs/ignored.mara.md\n").unwrap();

    for path in ["outside.mara.md", "docs/ignored.mara.md"] {
        let rejected = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                "REQ-HIDDEN",
                path,
                "--title",
                "Undiscoverable item",
                "--body",
                "Body.",
            ],
        );

        assert!(!rejected.status.success());
        assert!(
            stderr(&rejected).contains("is excluded by project content discovery"),
            "{}",
            stderr(&rejected)
        );
        assert!(!fixture.path().join(path).exists());
    }

    let created = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-VISIBLE",
            "docs/visible.mara.md",
            "--title",
            "Discoverable item",
            "--body",
            "Body.",
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));
    let valid = mara(fixture.path(), &["item", "validate", "REQ-VISIBLE"]);
    assert!(valid.status.success(), "{}", stderr(&valid));
}

#[test]
fn item_create_rejects_bodies_that_escape_the_created_item() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));

    for (id, body) in [
        ("REQ-TRUNCATED", ":::\nrest"),
        (
            "REQ-INJECTOR",
            "Outer body.\n:::\n\n:::mara requirement REQ-INJECTED\n:title: Injected item\n\nInjected body.",
        ),
    ] {
        let rejected = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                id,
                "items.mara.md",
                "--title",
                "Escaping body",
                "--body",
                body,
            ],
        );

        assert!(!rejected.status.success());
        assert!(
            stderr(&rejected).contains("body must remain inside the created item"),
            "{}",
            stderr(&rejected)
        );
        assert!(!fixture.path().join("items.mara.md").exists());
    }

    let fenced = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-FENCED",
            "items.mara.md",
            "--title",
            "Fenced delimiters",
            "--body",
            "```markdown\n:::\n\n:::mara requirement REQ-EXAMPLE\n```\n",
        ],
    );
    assert!(fenced.status.success(), "{}", stderr(&fenced));
    let valid = mara(fixture.path(), &["item", "validate", "REQ-FENCED"]);
    assert!(valid.status.success(), "{}", stderr(&valid));
}

#[cfg(unix)]
#[test]
fn item_create_rejects_destinations_below_directory_symlinks() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project_file = fixture.path().join(".mara/project.toml");
    let project = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        project.replace("**/*.mara.md", "docs/**/*.mara.md"),
    )
    .unwrap();
    fs::create_dir(fixture.path().join("real")).unwrap();
    std::os::unix::fs::symlink("real", fixture.path().join("docs")).unwrap();

    let rejected = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-SYMLINKED",
            "docs/item.mara.md",
            "--title",
            "Symlinked destination",
            "--body",
            "Body.",
        ],
    );

    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("is excluded by project content discovery"),
        "{}",
        stderr(&rejected)
    );
    assert!(!fixture.path().join("real/item.mara.md").exists());
}

#[test]
fn relation_add_and_remove_validate_endpoints_and_update_only_the_source_item() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("items.mara.md"),
        ":::mara scenario SCN-TARGET\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Target scenario\n\nTarget.\n:::\n\n:::mara design DES-WRONG\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Wrong target\n\nWrong.\n:::\n\n:::mara requirement REQ-SOURCE\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: Source requirement\n\nSource.\n:::\n",
    )
    .unwrap();

    let add = mara(
        fixture.path(),
        &[
            "relation",
            "add",
            "REQ-SOURCE",
            "derives_from",
            "SCN-TARGET",
        ],
    );

    assert!(add.status.success(), "{}", stderr(&add));
    assert!(stdout(&add).contains("added relation 'derives_from'"));
    let authored = fs::read_to_string(fixture.path().join("items.mara.md")).unwrap();
    assert!(authored.contains(
        ":::mara requirement REQ-SOURCE\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: Source requirement\n:derives_from: SCN-TARGET\n\nSource.\n:::\n"
    ));
    assert_eq!(authored.matches(":derives_from: SCN-TARGET").count(), 1);
    let valid = mara(fixture.path(), &["project", "validate"]);
    assert!(valid.status.success(), "{}", stderr(&valid));

    for (arguments, expected) in [
        (
            [
                "relation",
                "add",
                "REQ-MISSING",
                "derives_from",
                "SCN-TARGET",
            ],
            "source item 'REQ-MISSING' was not found",
        ),
        (
            [
                "relation",
                "add",
                "REQ-SOURCE",
                "derives_from",
                "SCN-MISSING",
            ],
            "target item 'SCN-MISSING' was not found",
        ),
        (
            ["relation", "add", "REQ-SOURCE", "derives_from", "DES-WRONG"],
            "does not allow target flavour 'design'",
        ),
    ] {
        let rejected = mara(fixture.path(), &arguments);
        assert!(!rejected.status.success());
        assert!(
            stderr(&rejected).contains(expected),
            "{}",
            stderr(&rejected)
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("items.mara.md")).unwrap(),
            authored
        );
    }

    let remove = mara(
        fixture.path(),
        &[
            "relation",
            "remove",
            "REQ-SOURCE",
            "derives_from",
            "SCN-TARGET",
        ],
    );

    assert!(remove.status.success(), "{}", stderr(&remove));
    assert!(stdout(&remove).contains("removed relation 'derives_from'"));
    let removed = fs::read_to_string(fixture.path().join("items.mara.md")).unwrap();
    assert!(!removed.contains(":derives_from: SCN-TARGET"));
    let valid = mara(fixture.path(), &["project", "validate"]);
    assert!(valid.status.success(), "{}", stderr(&valid));
}

#[test]
fn relation_mutation_rejects_ambiguous_item_identities_before_writing() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = fixture.path().join("items.mara.md");
    fs::write(
        &path,
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:depends_on: REQ-SECOND

Source.
:::

:::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: First

First.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Second

Second.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();

    for action in ["add", "remove"] {
        let rejected = mara(
            fixture.path(),
            &["relation", action, "REQ-SOURCE", "depends_on", "REQ-FIRST"],
        );

        assert!(!rejected.status.success(), "{action}");
        assert!(
            stderr(&rejected).contains(
                "cannot mutate relations while item MID '01ARZ3NDEKTSV4RRFFQ69G5F01' is ambiguous"
            ),
            "{}",
            stderr(&rejected)
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}

#[test]
fn relation_mutation_rejects_secondary_authored_mids_before_writing() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let path = fixture.path().join("items.mara.md");
    fs::write(
        &path,
        r#":::mara requirement REQ-SOURCE
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Source
:depends_on: 01ARZ3NDEKTSV4RRFFQ69G5F02

Source.
:::

:::mara requirement REQ-FIRST
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: First

First.
:::

:::mara requirement REQ-SECOND
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Second

Second.
:::
"#,
    )
    .unwrap();
    let original = fs::read_to_string(&path).unwrap();

    let rejected = mara(
        fixture.path(),
        &[
            "relation",
            "remove",
            "REQ-SOURCE",
            "depends_on",
            "REQ-FIRST",
        ],
    );

    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains(
            "cannot mutate relations while item 'REQ-FIRST' does not have exactly one MID"
        ),
        "{}",
        stderr(&rejected)
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

fn retrieval_fixture() -> TempDir {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let schema_file = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_file).unwrap();
    fs::write(
        &schema_file,
        schema.replace(
            "    id_prefix: REQ-\n    body: required\n    fields: {}",
            "    id_prefix: REQ-\n    body: required\n    fields:\n      status:\n        type: enum\n        values: [draft, accepted]",
        ),
    )
    .unwrap();
    fs::create_dir(fixture.path().join("docs")).unwrap();
    fs::write(
        fixture.path().join("docs/a.mara.md"),
        ":::mara scenario SCN-BASE\n:title: Base scenario\n\nBase workflow.\n:::\n\n:::mara requirement REQ-ALPHA\n:title: Alpha requirement\n:status: draft\n:derives_from: SCN-BASE\n\nNeed searchable Zebra knowledge.\n:::\n",
    )
    .unwrap();
    fs::write(
        fixture.path().join("docs/b.mara.md"),
        ":::mara requirement REQ-BETA\n:title: Beta requirement\n:status: accepted\n:derives_from: SCN-BASE\n\nSecond requirement body.\n:::\n\n:::mara design DES-ALPHA\n:title: Alpha design\n:satisfies: REQ-ALPHA\n\nDesign body.\n:::\n\n:::mara scenario SCN-GERMAN\n:title: Straße\n\nGerman title.\n:::\n",
    )
    .unwrap();
    fixture
}

#[test]
fn item_get_returns_one_complete_item_with_authored_and_incoming_relations() {
    let fixture = retrieval_fixture();

    let get = mara(fixture.path(), &["item", "get", "REQ-ALPHA"]);

    assert!(get.status.success(), "{}", stderr(&get));
    assert_eq!(
        stdout(&get),
        "REQ-ALPHA\trequirement\tAlpha requirement\nsource\tdocs/a.mara.md\tstart_byte=69\tend_byte=202\tstart_line=7\tend_line=13\nmetadata\ntitle\tAlpha requirement\nstatus\tdraft\nderives_from\tSCN-BASE\nbody\nNeed searchable Zebra knowledge.\nrelations\noutgoing\tderives_from\tSCN-BASE\tscenario\tBase scenario\tdocs/a.mara.md:1\nincoming\tsatisfies\tDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\n"
    );

    let missing = mara(fixture.path(), &["item", "get", "REQ-MISSING"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("item 'REQ-MISSING' was not found"));

    let duplicate_file = fixture.path().join("docs/b.mara.md");
    let duplicate_source = fs::read_to_string(&duplicate_file).unwrap();
    fs::write(
        duplicate_file,
        format!(
            "{duplicate_source}\n:::mara requirement REQ-ALPHA\n:title: Duplicate alpha\n\nDuplicate.\n:::\n"
        ),
    )
    .unwrap();
    let ambiguous = mara(fixture.path(), &["item", "get", "REQ-ALPHA"]);
    assert!(!ambiguous.status.success());
    assert!(stderr(&ambiguous).contains("item ID 'REQ-ALPHA' is ambiguous"));
}

#[test]
fn item_list_and_search_return_deterministic_compact_filtered_summaries() {
    let fixture = retrieval_fixture();

    let listed = mara(
        fixture.path(),
        &["item", "list", "--flavour", "requirement"],
    );
    assert!(listed.status.success(), "{}", stderr(&listed));
    assert_eq!(
        stdout(&listed),
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\nREQ-BETA\trequirement\tBeta requirement\tdocs/b.mara.md:1\n"
    );
    assert!(!stdout(&listed).contains("requirement body"));

    let filtered = mara(
        fixture.path(),
        &[
            "item",
            "list",
            "--field",
            "status=draft",
            "--relation",
            "derives_from",
            "--path",
            "docs/a.mara.md",
            "--limit",
            "1",
        ],
    );
    assert!(filtered.status.success(), "{}", stderr(&filtered));
    assert_eq!(
        stdout(&filtered),
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\n"
    );

    let normalized_path = mara(
        fixture.path(),
        &["item", "list", "--path", "./docs/a.mara.md"],
    );
    assert!(
        normalized_path.status.success(),
        "{}",
        stderr(&normalized_path)
    );
    assert_eq!(
        stdout(&normalized_path),
        "SCN-BASE\tscenario\tBase scenario\tdocs/a.mara.md:1\nREQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\n"
    );

    for invalid_path in [
        fixture.path().join("docs/a.mara.md"),
        Path::new("../outside.mara.md").to_path_buf(),
    ] {
        let rejected = mara(
            fixture.path(),
            &["item", "list", "--path", invalid_path.to_str().unwrap()],
        );
        assert!(!rejected.status.success());
        assert!(
            stderr(&rejected).contains("path filter must be a project-relative path"),
            "{}",
            stderr(&rejected)
        );
    }

    for query in ["zEbRa", "accepted", "DES-ALPHA"] {
        let searched = mara(fixture.path(), &["item", "search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(stdout(&searched).lines().count(), 1, "query: {query}");
    }
    let searched = mara(fixture.path(), &["item", "search", "alpha"]);
    assert!(searched.status.success(), "{}", stderr(&searched));
    assert_eq!(
        stdout(&searched),
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\nDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\n"
    );

    let case_folded = mara(fixture.path(), &["item", "search", "STRASSE"]);
    assert!(case_folded.status.success(), "{}", stderr(&case_folded));
    assert_eq!(
        stdout(&case_folded),
        "SCN-GERMAN\tscenario\tStraße\tdocs/b.mara.md:16\n"
    );
}

#[test]
fn item_search_matches_distinct_complete_unicode_terms_across_values() {
    let fixture = retrieval_fixture();
    fs::write(
        fixture.path().join("docs/search.mara.md"),
        ":::mara requirement REQ-CROSS-FIELD\n:title: Project knowledge\n\nRetrieve bounded guidance before running validation.\n:::\n\n:::mara scenario SCN-UNICODE\n:title: Café workflow\n\nEquivalent Unicode forms remain searchable.\n:::\n\n:::mara design DES-PROJECTOR\n:title: Projector checks\n\nValidate displays without fuzzy matching.\n:::\n",
    )
    .unwrap();

    for query in [
        "project validation",
        "validation project",
        "project project validation",
    ] {
        let searched = mara(fixture.path(), &["item", "search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(
            stdout(&searched),
            "REQ-CROSS-FIELD\trequirement\tProject knowledge\tdocs/search.mara.md:1\n",
            "query: {query}"
        );
    }

    for query in ["project missing", "projects validation", "project valid"] {
        let searched = mara(fixture.path(), &["item", "search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(stdout(&searched), "", "query: {query}");
    }

    let unicode_equivalent = mara(fixture.path(), &["item", "search", "CAFE\u{301} WORKFLOW"]);
    assert!(
        unicode_equivalent.status.success(),
        "{}",
        stderr(&unicode_equivalent)
    );
    assert_eq!(
        stdout(&unicode_equivalent),
        "SCN-UNICODE\tscenario\tCafé workflow\tdocs/search.mara.md:7\n"
    );
}

#[test]
fn item_related_returns_filtered_direct_neighbours_with_relation_and_direction() {
    let fixture = retrieval_fixture();

    let related = mara(fixture.path(), &["item", "related", "REQ-ALPHA"]);
    assert!(related.status.success(), "{}", stderr(&related));
    assert_eq!(
        stdout(&related),
        "outgoing\tderives_from\tSCN-BASE\tscenario\tBase scenario\tdocs/a.mara.md:1\nincoming\tsatisfies\tDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\n"
    );

    let incoming = mara(
        fixture.path(),
        &[
            "item",
            "related",
            "REQ-ALPHA",
            "--direction",
            "incoming",
            "--relation",
            "satisfies",
            "--flavour",
            "design",
        ],
    );
    assert!(incoming.status.success(), "{}", stderr(&incoming));
    assert_eq!(
        stdout(&incoming),
        "incoming\tsatisfies\tDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\n"
    );

    let no_match = mara(
        fixture.path(),
        &[
            "item",
            "related",
            "REQ-ALPHA",
            "--direction",
            "outgoing",
            "--flavour",
            "design",
        ],
    );
    assert!(no_match.status.success(), "{}", stderr(&no_match));
    assert!(stdout(&no_match).is_empty());
}

#[test]
fn cli_parse_failures_follow_the_selected_output_format() {
    let fixture = TempDir::new().unwrap();

    for arguments in [
        &["--format", "json", "item", "get"][..],
        &["item", "get", "--format=json"][..],
    ] {
        let output = mara(fixture.path(), arguments);

        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).is_empty(), "{}", stderr(&output));
        let error: Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert!(
            error["error"]["message"].as_str().unwrap().contains("<ID>"),
            "{error:#}"
        );
    }

    let human = mara(fixture.path(), &["item", "get"]);
    assert_eq!(human.status.code(), Some(2));
    assert!(stdout(&human).is_empty());
    assert!(stderr(&human).contains("<ID>"), "{}", stderr(&human));

    let help = mara(fixture.path(), &["--format", "json", "--help"]);
    assert!(help.status.success(), "{}", stderr(&help));
    assert!(stderr(&help).is_empty(), "{}", stderr(&help));
    assert!(stdout(&help).contains("Usage: mara"), "{}", stdout(&help));
}

#[test]
fn mcp_rejects_undeclared_arguments_without_mutating_the_selected_project() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_request(2, "tools/list", json!({})),
            mcp_call(
                3,
                "item_create",
                json!({
                    "flavour": "requirement",
                    "id": "REQ-UNDECLARED",
                    "file": "items.mara.md",
                    "title": "Undeclared project override",
                    "body": "Must not be written.",
                    "workspace": "other"
                }),
            ),
            mcp_call(4, "project_validate", json!({ "workspace": "other" })),
        ],
    );

    for tool in mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            tool["inputSchema"]["additionalProperties"], false,
            "{} accepts undeclared arguments: {tool:#}",
            tool["name"]
        );
    }
    for id in [3, 4] {
        let response = mcp_response(&responses, id);
        let rejected = response.get("error").is_some() || response["result"]["isError"] == true;
        assert!(rejected, "MCP call {id} was not rejected: {response:#}");
        assert!(
            response.to_string().contains("unknown field"),
            "MCP call {id} did not identify its undeclared argument: {response:#}"
        );
    }
    assert!(!fixture.path().join("items.mara.md").exists());
}

#[test]
fn mcp_starts_outside_a_project_and_initializes_an_absolute_target() {
    let fixture = TempDir::new().unwrap();
    let project = fixture.path().join("new-project");

    let initialized = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(
                2,
                "project_init",
                json!({ "project": project, "template": "minimal" }),
            ),
        ],
    );
    let result = &mcp_response(&initialized, 2)["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["project"]["root"],
        project.to_string_lossy().as_ref()
    );
    assert!(project.join(".mara/project.toml").is_file());
    assert!(project.join(".mara/schema.yaml").is_file());

    let validated = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_validate", json!({ "project": project })),
            mcp_call(3, "project_validate", json!({ "project": "new-project" })),
        ],
    );
    assert_eq!(
        mcp_response(&validated, 2)["result"]["structuredContent"]["valid"],
        true
    );
    assert_eq!(mcp_response(&validated, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&validated, 3)
            .to_string()
            .contains("must be absolute")
    );
}

#[test]
fn mcp_bound_server_initializes_its_selected_target_without_an_override() {
    let fixture = TempDir::new().unwrap();
    let project = fixture.path().join("new-project");
    let project_path = project.to_str().unwrap();

    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", project_path],
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_init", json!({ "template": "minimal" })),
            mcp_call(3, "project_init", json!({ "project": project_path })),
        ],
    );

    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], false);
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"]["project"]["root"],
        project_path
    );
    assert!(project.join(".mara/project.toml").is_file());
    assert!(project.join(".mara/schema.yaml").is_file());
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("started with --project")
    );
}

#[test]
fn mcp_unbound_project_init_requires_an_absolute_target() {
    let fixture = TempDir::new().unwrap();

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_init", json!({})),
        ],
    );

    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 2)
            .to_string()
            .contains("requires an absolute project path")
    );
    assert!(!fixture.path().join(".mara/project.toml").exists());
}

#[test]
fn mcp_project_option_after_the_command_binds_the_server() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    let project = fixture.path().to_str().unwrap();

    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", project],
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_validate", json!({})),
            mcp_call(3, "project_validate", json!({ "project": project })),
        ],
    );

    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"]["valid"],
        true
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("started with --project")
    );
}

#[test]
fn mcp_exposes_every_project_bound_alpha_operation_with_cli_equivalent_results() {
    let fixture = retrieval_fixture();
    let cli_item = mara(
        fixture.path(),
        &["--format", "json", "item", "get", "REQ-ALPHA"],
    );
    assert!(cli_item.status.success(), "{}", stderr(&cli_item));
    let cli_item: Value = serde_json::from_str(&stdout(&cli_item)).unwrap();
    let cli_search = mara(
        fixture.path(),
        &["--format", "json", "item", "search", "alpha zebra"],
    );
    assert!(cli_search.status.success(), "{}", stderr(&cli_search));
    let cli_search: Value = serde_json::from_str(&stdout(&cli_search)).unwrap();
    let cli_schema = mara(
        fixture.path(),
        &["--format", "json", "schema", "list", "relation"],
    );
    assert!(cli_schema.status.success(), "{}", stderr(&cli_schema));
    let cli_schema: Value = serde_json::from_str(&stdout(&cli_schema)).unwrap();

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_request(2, "tools/list", json!({})),
            mcp_call(3, "item_get", json!({ "id": "REQ-ALPHA" })),
            mcp_call(4, "item_search", json!({ "query": "alpha zebra" })),
            mcp_call(5, "schema_list", json!({ "kind": "relation" })),
        ],
    );

    let tools = mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap();
    let tool_names = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        tool_names,
        BTreeSet::from([
            "project_init",
            "project_validate",
            "project_mid_backfill",
            "project_transaction_rollback",
            "schema_get",
            "schema_list",
            "schema_validate",
            "item_create",
            "item_move",
            "item_get",
            "item_list",
            "item_search",
            "item_related",
            "item_validate",
            "relation_add",
            "relation_remove",
        ])
    );
    for tool in tools {
        assert!(
            tool["inputSchema"]["properties"].get("project").is_some(),
            "{} does not declare project selection: {tool:#}",
            tool["name"]
        );
        let project_is_required = tool["inputSchema"]["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|field| field == "project"));
        assert!(
            !project_is_required,
            "{} unexpectedly requires project selection: {tool:#}",
            tool["name"],
        );
    }
    assert_eq!(
        mcp_response(&responses, 3)["result"]["structuredContent"],
        cli_item
    );
    assert_eq!(
        mcp_response(&responses, 4)["result"]["structuredContent"],
        cli_search
    );
    assert_eq!(
        mcp_response(&responses, 5)["result"]["structuredContent"],
        cli_schema
    );
}

#[test]
fn primary_workflows_run_end_to_end_against_real_source_files() {
    let fixture = TempDir::new().unwrap();

    let initialized = mara(fixture.path(), &["project", "init"]);
    assert!(initialized.status.success(), "{}", stderr(&initialized));
    let schema = mara(fixture.path(), &["schema", "get"]);
    assert!(schema.status.success(), "{}", stderr(&schema));
    assert!(stdout(&schema).contains("scenario"));

    fs::create_dir(fixture.path().join("docs")).unwrap();
    let scenario = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "scenario",
            "SCN-DOGFOOD",
            "docs/workflow.mara.md",
            "--title",
            "Dogfood the alpha workflow",
            "--body",
            "A user initializes and authors a real Mara project.",
        ],
    );
    assert!(scenario.status.success(), "{}", stderr(&scenario));
    let requirement = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-DOGFOOD",
            "docs/workflow.mara.md",
            "--title",
            "Retrieve bounded dogfood knowledge",
            "--body",
            "Mara retrieves bounded knowledge from the authored source file.",
        ],
    );
    assert!(requirement.status.success(), "{}", stderr(&requirement));
    let related = mara(
        fixture.path(),
        &[
            "relation",
            "add",
            "REQ-DOGFOOD",
            "derives_from",
            "SCN-DOGFOOD",
        ],
    );
    assert!(related.status.success(), "{}", stderr(&related));

    let validated = mara(fixture.path(), &["project", "validate"]);
    assert!(validated.status.success(), "{}", stderr(&validated));

    let searched = mara(
        fixture.path(),
        &[
            "item",
            "search",
            "bounded",
            "--flavour",
            "requirement",
            "--relation",
            "derives_from",
            "--path",
            "docs/workflow.mara.md",
            "--limit",
            "1",
        ],
    );
    assert!(searched.status.success(), "{}", stderr(&searched));
    assert!(stdout(&searched).contains("REQ-DOGFOOD"));
    let fetched = mara(fixture.path(), &["item", "get", "REQ-DOGFOOD"]);
    assert!(fetched.status.success(), "{}", stderr(&fetched));
    assert!(stdout(&fetched).contains("Mara retrieves bounded knowledge"));
    let neighbours = mara(fixture.path(), &["item", "related", "REQ-DOGFOOD"]);
    assert!(neighbours.status.success(), "{}", stderr(&neighbours));
    assert!(stdout(&neighbours).contains("outgoing\tderives_from\tSCN-DOGFOOD"));
}

#[test]
fn dogfooded_repository_validates_and_retrieves_equivalently_through_cli_and_mcp() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_validation = mara(repository, &["--format", "json", "project", "validate"]);
    assert!(
        cli_validation.status.success(),
        "{}",
        stderr(&cli_validation)
    );
    let cli_validation: Value = serde_json::from_str(&stdout(&cli_validation)).unwrap();
    assert_eq!(cli_validation["valid"], true);

    let cli_search = mara(
        repository,
        &[
            "--format",
            "json",
            "item",
            "search",
            "Start a project",
            "--flavour",
            "scenario",
            "--path",
            "docs/alpha.mara.md",
            "--limit",
            "1",
        ],
    );
    assert!(cli_search.status.success(), "{}", stderr(&cli_search));
    let cli_search: Value = serde_json::from_str(&stdout(&cli_search)).unwrap();
    assert_eq!(cli_search["items"][0]["id"], "SCN-START-STRUCTURED-PROJECT");

    let cli_item = mara(
        repository,
        &[
            "--format",
            "json",
            "item",
            "get",
            "SCN-START-STRUCTURED-PROJECT",
        ],
    );
    assert!(cli_item.status.success(), "{}", stderr(&cli_item));
    let cli_item: Value = serde_json::from_str(&stdout(&cli_item)).unwrap();
    let cli_related = mara(
        repository,
        &[
            "--format",
            "json",
            "item",
            "related",
            "SCN-START-STRUCTURED-PROJECT",
        ],
    );
    assert!(cli_related.status.success(), "{}", stderr(&cli_related));
    let cli_related: Value = serde_json::from_str(&stdout(&cli_related)).unwrap();
    assert!(!cli_related["items"].as_array().unwrap().is_empty());

    let responses = mcp_exchange(
        repository,
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_validate", json!({})),
            mcp_call(
                3,
                "item_search",
                json!({
                    "query": "Start a project",
                    "flavours": ["scenario"],
                    "paths": ["docs/alpha.mara.md"],
                    "limit": 1
                }),
            ),
            mcp_call(
                4,
                "item_get",
                json!({ "id": "SCN-START-STRUCTURED-PROJECT" }),
            ),
            mcp_call(
                5,
                "item_related",
                json!({ "id": "SCN-START-STRUCTURED-PROJECT" }),
            ),
        ],
    );

    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        cli_validation
    );
    assert_eq!(
        mcp_response(&responses, 3)["result"]["structuredContent"],
        cli_search
    );
    assert_eq!(
        mcp_response(&responses, 4)["result"]["structuredContent"],
        cli_item
    );
    assert_eq!(
        mcp_response(&responses, 5)["result"]["structuredContent"],
        cli_related
    );
}

#[test]
fn cli_json_and_mcp_return_the_same_structured_validation_diagnostics() {
    let fixture = TempDir::new().unwrap();
    let init = mara(fixture.path(), &["project", "init"]);
    assert!(init.status.success(), "{}", stderr(&init));
    fs::write(
        fixture.path().join("invalid.mara.md"),
        ":::mara requirement WRONG-ID\n:title: Invalid\n\n\n:::\n",
    )
    .unwrap();

    let cli = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    assert!(!cli.status.success());
    assert!(stderr(&cli).is_empty(), "{}", stderr(&cli));
    let cli_result: Value = serde_json::from_str(&stdout(&cli)).unwrap();
    assert_eq!(cli_result["valid"], false);
    assert_eq!(cli_result["diagnostics"].as_array().unwrap().len(), 3);

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            mcp_call(2, "project_validate", json!({})),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"], cli_result);
}

fn move_fixture() -> (TempDir, String, String) {
    let fixture = TempDir::new().unwrap();
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
    let original = mara(
        fixture.path(),
        &["--format", "json", "item", "get", "REQ-MOVE"],
    );
    let original: Value = serde_json::from_slice(&original.stdout).unwrap();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "move",
            original["summary"]["mid"].as_str().unwrap(),
            "destination.mara.md",
            "--line",
            "1",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result,
        json!({"id": "REQ-MOVE", "mid": original["summary"]["mid"], "old_location": {"path": "source.mara.md", "line": 1}, "new_location": {"path": "destination.mara.md", "line": 1}})
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
    let resolved = mara(
        fixture.path(),
        &["--format", "json", "item", "get", "REQ-MOVE"],
    );
    let resolved: Value = serde_json::from_slice(&resolved.stdout).unwrap();
    assert_eq!(resolved["summary"]["mid"], original["summary"]["mid"]);
    assert_eq!(resolved["body"], original["body"]);
    assert_eq!(
        resolved["incoming_relations"][0]["item"]["mid"],
        original["incoming_relations"][0]["item"]["mid"]
    );
    assert_eq!(resolved["incoming_relations"][0]["relation"], "depends_on");
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    assert!(!fixture.path().join(".mara/transaction.json").exists());
}

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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
}

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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
}

#[test]
fn pending_transaction_blocks_cli_and_mcp_mutations_and_exposes_recovery_errors() {
    let (fixture, source, destination) = move_fixture();
    fs::write(
        fixture.path().join(".mara/transaction.json"),
        "interrupted journal",
    )
    .unwrap();
    for args in [
        vec!["item", "move", "REQ-MOVE", "destination.mara.md"],
        vec![
            "item",
            "create",
            "requirement",
            "REQ-NEW",
            "new.mara.md",
            "--title",
            "New",
        ],
        vec!["project", "mid", "backfill"],
        vec!["relation", "remove", "REQ-STAY", "depends_on", "REQ-MOVE"],
    ] {
        let output = mara(fixture.path(), &args);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("project transaction rollback"));
    }
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            mcp_call(
                2,
                "item_move",
                json!({"reference": "REQ-MOVE", "file": "destination.mara.md"}),
            ),
            mcp_call(3, "project_transaction_rollback", json!({})),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("unrecoverable transaction")
    );
    let output = mara(fixture.path(), &["project", "transaction", "rollback"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unrecoverable transaction"));
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        destination
    );
}

#[test]
fn transaction_rollback_cli_and_mcp_restore_real_files_and_are_idempotent() {
    assert_transaction_rollback_cli_and_mcp(true);
}

#[test]
fn transaction_rollback_cli_and_mcp_accept_permissions_without_unix_mode() {
    assert_transaction_rollback_cli_and_mcp(false);
}

fn assert_transaction_rollback_cli_and_mcp(include_unix_mode: bool) {
    let (fixture, source, _) = move_fixture();
    let metadata = fs::metadata(fixture.path().join("source.mara.md")).unwrap();
    let mut mode = json!({"readonly": metadata.permissions().readonly()});
    if include_unix_mode {
        #[cfg(unix)]
        let unix_mode = metadata.permissions().mode();
        // A Unix journal must also recover on platforms without Unix permissions.
        #[cfg(not(unix))]
        let unix_mode = 0o100600;
        mode["unix_mode"] = json!(unix_mode);
    }
    let journal = json!({"format_version": 1, "changes": [
        {"path": "source.mara.md", "before": source, "after": "", "mode": mode},
        {"path": "new.mara.md", "before": null, "after": source, "mode": null}
    ]});
    for through_mcp in [false, true] {
        // A published format-1 journal after both replacements, before journal cleanup.
        fs::write(
            fixture.path().join(".mara/transaction.json"),
            journal.to_string(),
        )
        .unwrap();
        fs::write(fixture.path().join("source.mara.md"), "").unwrap();
        fs::write(fixture.path().join("new.mara.md"), &source).unwrap();
        let result = if through_mcp {
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                    mcp_call(
                        2,
                        "project_transaction_rollback",
                        json!({"project": fixture.path()}),
                    ),
                ],
            );
            assert_eq!(mcp_response(&responses, 2)["result"]["isError"], false);
            mcp_response(&responses, 2)["result"]["structuredContent"].clone()
        } else {
            let output = mara(
                fixture.path(),
                &["--format", "json", "project", "transaction", "rollback"],
            );
            assert!(output.status.success(), "{}", stdout(&output));
            serde_json::from_slice(&output.stdout).unwrap()
        };
        assert_eq!(
            result,
            json!({"project": fixture.path(), "restored": ["source.mara.md", "new.mara.md"]})
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
            source
        );
        assert!(!fixture.path().join("new.mara.md").exists());
        assert!(!fixture.path().join(".mara/transaction.json").exists());
        let output = mara(fixture.path(), &["project", "transaction", "rollback"]);
        assert!(output.status.success());
        assert!(stdout(&output).contains("no pending transaction"));
        assert!(
            mara(fixture.path(), &["project", "validate"])
                .status
                .success()
        );
    }
}

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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
}

#[cfg(unix)]
#[test]
fn item_move_rejects_symlink_destinations_without_touching_sources() {
    use std::os::unix::fs::symlink;
    let (fixture, source, destination) = move_fixture();
    let external = TempDir::new().unwrap();
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
