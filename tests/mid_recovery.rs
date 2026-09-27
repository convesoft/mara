mod support;
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;
use tempfile::TempDir;

fn project() -> TempDir {
    let fixture = fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    fixture
}
fn invoke(root: &Path, through_mcp: bool, backfill: bool) -> Result<Value, String> {
    if through_mcp {
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    if backfill {
                        "project_mid_backfill"
                    } else {
                        "project_transaction_rollback"
                    },
                    json!({"project":root}),
                ),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        if result["isError"] == true {
            Err(result.to_string())
        } else {
            Ok(result["structuredContent"].clone())
        }
    } else {
        let args = if backfill {
            ["--format", "json", "project", "mid", "backfill"]
        } else {
            ["--format", "json", "project", "transaction", "rollback"]
        };
        let output = mara(root, &args);
        if output.status.success() {
            Ok(serde_json::from_slice(&output.stdout).unwrap())
        } else {
            Err(format!("{}{}", stdout(&output), stderr(&output)))
        }
    }
}
fn valid(root: &Path) -> bool {
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let (corpus, diagnostics) = mara::load_corpus_for_validation(&project, &schema).unwrap();
    diagnostics.is_empty() && mara::validate_corpus(&corpus, &schema).is_empty()
}
const MID: &str = "01ARZ3NDEKTSV4RRFFQ69G5F00";
const LEGACY: &str = ":::mara scenario SCN-LEGACY\n:title: Legacy scenario\n\nLegacy.\n:::\n\n:::mara requirement REQ-LEGACY\n:title: Legacy requirement\n:derives_from: SCN-LEGACY\n\nLegacy body.\n:::\n";

// @mara implements VER-MID-AND-RECOVERY
// @mara checks REQ-MID-BACKFILL
// @mara checks DES-MID-BACKFILL
#[test]
fn backfill_cli_and_mcp_preserve_bytes_identities_permissions_and_result_lines() {
    for through_mcp in [false, true] {
        for newline in ["\n", "\r\n"] {
            let fixture = project();
            let root = fixture.path();
            let source = LEGACY.replace('\n', newline);
            let path = root.join("legacy.mara.md");
            fs::write(&path, &source).unwrap();
            let existing = format!(
                ":::mara requirement REQ-EXISTING\n:mid: {MID}\n:title: Existing\n\nExisting.\n:::\n"
            );
            fs::write(root.join("existing.mara.md"), &existing).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            }
            let permissions = fs::metadata(&path).unwrap().permissions();
            assert!(mara(root, &["get", "REQ-LEGACY"]).status.success());
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            assert!(!valid(root));
            let result = invoke(root, through_mcp, true).unwrap();
            assert_eq!(result["project"], json!(root));
            let changed = result["changed"].as_array().unwrap();
            assert_eq!(changed.len(), 2);
            for (entry, id, line) in [
                (&changed[0], "SCN-LEGACY", 2),
                (&changed[1], "REQ-LEGACY", 9),
            ] {
                assert_eq!(entry["id"], id);
                assert_eq!(entry["line"], line);
                assert_eq!(entry["path"], "legacy.mara.md");
                let mid = entry["mid"].as_str().unwrap();
                assert_eq!(ulid::Ulid::from_string(mid).unwrap().to_string(), mid);
                assert_ne!(entry["mid"], MID);
            }
            assert_ne!(changed[0]["mid"], changed[1]["mid"]);
            let after = fs::read_to_string(&path).unwrap();
            let without_mids: String = after
                .split_inclusive('\n')
                .filter(|line| !line.starts_with(":mid: "))
                .collect();
            assert_eq!(without_mids, source);
            assert_eq!(
                after
                    .lines()
                    .filter(|line| line.starts_with(":mid: "))
                    .count(),
                2
            );
            if newline == "\r\n" {
                assert!(!after.replace("\r\n", "").contains('\n'));
            }
            assert_eq!(
                fs::read_to_string(root.join("existing.mara.md")).unwrap(),
                existing
            );
            assert_eq!(fs::metadata(&path).unwrap().permissions(), permissions);
            assert!(valid(root));
            assert_eq!(
                invoke(root, through_mcp, true).unwrap()["changed"],
                json!([])
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), after);
            assert!(!root.join(".mara/transaction.json").exists());
        }
    }
}

// @mara checks REQ-MID-BACKFILL
#[test]
fn backfill_preflight_rejects_all_other_errors_without_matching_authored_text() {
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        let schema_path = root.join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_path).unwrap().replace(
            "    id_prefix: REQ-\n    body: required\n    fields: {}",
            "    id_prefix: REQ-\n    body: required\n    fields:\n      blocked: {type: boolean}",
        );
        fs::write(schema_path, schema).unwrap();
        fs::write(root.join("a-legacy.mara.md"), LEGACY).unwrap();
        for broken in [
            ":mid: not-a-mid\n:title: Invalid\n\nBody.\n:::\n",
            ":title: Invalid\n:blocked: bad is missing its MID\n\nBody.\n:::\n",
            ":title: Invalid\n:depends_on: REQ-MISSING\n\nBody.\n:::\n",
            ":title: Invalid\n\n:::\n",
        ] {
            let broken = format!(":::mara requirement REQ-BROKEN\n{broken}");
            fs::write(root.join("z-invalid.mara.md"), &broken).unwrap();
            let error = invoke(root, through_mcp, true).unwrap_err();
            assert!(
                error.contains("cannot backfill MIDs while validation fails"),
                "{error}"
            );
            assert_eq!(
                fs::read_to_string(root.join("a-legacy.mara.md")).unwrap(),
                LEGACY
            );
            assert_eq!(
                fs::read_to_string(root.join("z-invalid.mara.md")).unwrap(),
                broken
            );
        }
        fs::write(root.join("z-invalid.mara.md"), [0xff]).unwrap();
        assert!(invoke(root, through_mcp, true).is_err());
        assert_eq!(
            fs::read_to_string(root.join("a-legacy.mara.md")).unwrap(),
            LEGACY
        );
        assert_eq!(fs::read(root.join("z-invalid.mara.md")).unwrap(), [0xff]);
    }
}

fn mode(path: &Path, include_unix: bool) -> Value {
    let permissions = fs::metadata(path).unwrap().permissions();
    let mut mode = json!({"readonly":permissions.readonly()});
    if include_unix {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            mode["unix_mode"] = json!(permissions.mode());
        }
        #[cfg(not(unix))]
        {
            mode["unix_mode"] = json!(0o100600);
        }
    }
    mode
}
fn journal(root: &Path, include_unix: bool) -> Value {
    json!({"format_version":1,"changes":[
        {"path":"source.mara.md","before":LEGACY,"after":"","mode":mode(&root.join("source.mara.md"),include_unix)},
        {"path":"new.mara.md","before":null,"after":LEGACY,"mode":null}
    ]})
}
fn write_journal(root: &Path, journal: &Value) {
    fs::write(root.join(".mara/transaction.json"), journal.to_string()).unwrap();
}

// @mara checks REQ-RECOVERABLE-MUTATION
// @mara checks DES-MUTATION-RECOVERY
#[test]
fn rollback_cli_and_mcp_restore_files_with_optional_unix_mode_and_no_schema() {
    for through_mcp in [false, true] {
        for include_unix in [false, true] {
            let fixture = project();
            let root = fixture.path();
            fs::write(root.join("source.mara.md"), LEGACY).unwrap();
            let permissions = fs::metadata(root.join("source.mara.md"))
                .unwrap()
                .permissions();
            let journal = journal(root, include_unix);
            // Recovery must not require a readable schema or well-formed current corpus.
            fs::write(root.join(".mara/schema.yaml"), [0xff]).unwrap();
            for partial in [false, true] {
                write_journal(root, &journal);
                fs::write(
                    root.join("source.mara.md"),
                    if partial { LEGACY } else { "" },
                )
                .unwrap();
                if !partial {
                    fs::write(root.join("new.mara.md"), LEGACY).unwrap();
                }
                let result = invoke(root, through_mcp, false).unwrap();
                assert_eq!(
                    result,
                    json!({"project":root,"restored":["source.mara.md","new.mara.md"]})
                );
                assert_eq!(
                    fs::read_to_string(root.join("source.mara.md")).unwrap(),
                    LEGACY
                );
                let restored_permissions = fs::metadata(root.join("source.mara.md"))
                    .unwrap()
                    .permissions();
                assert_eq!(restored_permissions.readonly(), permissions.readonly());
                #[cfg(unix)]
                if include_unix {
                    assert_eq!(restored_permissions, permissions);
                }
                assert!(!root.join("new.mara.md").exists());
                assert!(!root.join(".mara/transaction.json").exists());
                assert_eq!(
                    invoke(root, through_mcp, false).unwrap(),
                    json!({"project":root,"restored":[]})
                );
            }
        }
    }
}

// @mara checks DES-MUTATION-RECOVERY
#[test]
fn pending_journals_block_backfill_but_leave_reads_and_source_available() {
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        fs::write(root.join("source.mara.md"), LEGACY).unwrap();
        fs::write(root.join(".mara/transaction.json"), "interrupted journal").unwrap();
        let error = invoke(root, through_mcp, true).unwrap_err();
        assert!(error.contains("project transaction rollback"), "{error}");
        let error = invoke(root, through_mcp, false).unwrap_err();
        assert!(error.contains("unrecoverable transaction"), "{error}");
        assert!(mara(root, &["get", "REQ-LEGACY"]).status.success());
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            LEGACY
        );
        assert_eq!(
            fs::read_to_string(root.join(".mara/transaction.json")).unwrap(),
            "interrupted journal"
        );
    }
}

// @mara checks DES-MUTATION-RECOVERY
#[test]
fn active_os_lock_blocks_cli_and_mcp_writers_until_released() {
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        fs::write(root.join("source.mara.md"), LEGACY).unwrap();
        let path = root.join(".mara/mutation.lock");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        lock.try_lock().unwrap();
        for backfill in [false, true] {
            let error = invoke(root, through_mcp, backfill).unwrap_err();
            assert!(
                error.contains("another Mara mutation or recovery is active"),
                "{error}"
            );
        }
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            LEGACY
        );
        lock.unlock().unwrap();
        assert!(invoke(root, through_mcp, true).is_ok());
        assert!(path.exists());
    }
}

// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn malformed_journals_preserve_all_targets_and_the_journal() {
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        fs::write(root.join("source.mara.md"), LEGACY).unwrap();
        fs::write(root.join("new.mara.md"), LEGACY).unwrap();
        let original = journal(root, true);
        let mut malformed = Vec::new();
        for field in ["before", "mode"] {
            let mut value = original.clone();
            value["changes"][0].as_object_mut().unwrap().remove(field);
            malformed.push(value);
        }
        let mut value = original.clone();
        value["format_version"] = json!(2);
        malformed.push(value);
        let mut value = original.clone();
        value["extra"] = json!(true);
        malformed.push(value);
        let mut value = original.clone();
        value["changes"] = json!([]);
        malformed.push(value);
        let mut value = original.clone();
        value["changes"][1]["path"] = json!("source.mara.md");
        malformed.push(value);
        let mut value = original.clone();
        value["changes"][0]["mode"] = Value::Null;
        malformed.push(value);
        for path in [
            "../outside.mara.md",
            "/outside.mara.md",
            ".mara/project.toml",
        ] {
            let mut value = original.clone();
            value["changes"][0]["path"] = json!(path);
            malformed.push(value);
        }
        for value in malformed {
            write_journal(root, &value);
            assert!(invoke(root, through_mcp, false).is_err(), "{value}");
            assert_eq!(
                fs::read_to_string(root.join("source.mara.md")).unwrap(),
                LEGACY
            );
            assert_eq!(
                fs::read_to_string(root.join("new.mara.md")).unwrap(),
                LEGACY
            );
            assert_eq!(
                fs::read_to_string(root.join(".mara/transaction.json")).unwrap(),
                value.to_string()
            );
        }
    }
}

// @mara checks REQ-RECOVERABLE-MUTATION
#[test]
fn manual_edits_and_recorded_permission_conflicts_abort_before_any_restore() {
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        fs::write(root.join("source.mara.md"), "").unwrap();
        fs::write(root.join("new.mara.md"), "later manual edits").unwrap();
        let original = journal(root, true);
        write_journal(root, &original);
        let error = invoke(root, through_mcp, false).unwrap_err();
        assert!(
            error.contains("changed since transaction preflight"),
            "{error}"
        );
        assert_eq!(fs::read_to_string(root.join("source.mara.md")).unwrap(), "");
        assert_eq!(
            fs::read_to_string(root.join("new.mara.md")).unwrap(),
            "later manual edits"
        );
        assert_eq!(
            fs::read_to_string(root.join(".mara/transaction.json")).unwrap(),
            original.to_string()
        );
        fs::write(root.join("new.mara.md"), LEGACY).unwrap();
        let mut mismatches = Vec::new();
        let mut value = original.clone();
        value["changes"][0]["mode"]["readonly"] = json!(
            !original["changes"][0]["mode"]["readonly"]
                .as_bool()
                .unwrap()
        );
        value["changes"][0]["mode"]
            .as_object_mut()
            .unwrap()
            .remove("unix_mode");
        mismatches.push(value);
        #[cfg(unix)]
        {
            let mut value = original.clone();
            value["changes"][0]["mode"]["unix_mode"] = json!(
                original["changes"][0]["mode"]["unix_mode"]
                    .as_u64()
                    .unwrap()
                    ^ 0o100
            );
            mismatches.push(value);
        }
        for value in mismatches {
            write_journal(root, &value);
            let error = invoke(root, through_mcp, false).unwrap_err();
            assert!(error.contains("permissions"), "{error}");
            assert_eq!(fs::read_to_string(root.join("source.mara.md")).unwrap(), "");
            assert_eq!(
                fs::read_to_string(root.join("new.mara.md")).unwrap(),
                LEGACY
            );
            assert_eq!(
                fs::read_to_string(root.join(".mara/transaction.json")).unwrap(),
                value.to_string()
            );
        }
        write_journal(root, &original);
        invoke(root, through_mcp, false).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            LEGACY
        );
        assert!(!root.join("new.mara.md").exists());
    }
}

// @mara checks DES-MUTATION-RECOVERY
#[cfg(unix)]
#[test]
fn lock_journal_and_recovery_paths_reject_symlink_traversal() {
    use std::os::unix::fs::symlink;
    for through_mcp in [false, true] {
        let fixture = project();
        let root = fixture.path();
        let outside = tempfile::tempdir().unwrap();
        fs::write(root.join("source.mara.md"), LEGACY).unwrap();
        fs::write(outside.path().join("outside.mara.md"), "outside").unwrap();
        symlink(
            outside.path().join("outside.mara.md"),
            root.join(".mara/mutation.lock"),
        )
        .unwrap();
        assert!(
            invoke(root, through_mcp, true)
                .unwrap_err()
                .contains("symlinks")
        );
        assert!(
            invoke(root, through_mcp, false)
                .unwrap_err()
                .contains("symlinks")
        );
        fs::remove_file(root.join(".mara/mutation.lock")).unwrap();
        symlink(
            outside.path().join("outside.mara.md"),
            root.join(".mara/transaction.json"),
        )
        .unwrap();
        assert!(
            invoke(root, through_mcp, true)
                .unwrap_err()
                .contains("pending transaction")
        );
        assert!(
            invoke(root, through_mcp, false)
                .unwrap_err()
                .contains("symlinks")
        );
        fs::remove_file(root.join(".mara/transaction.json")).unwrap();
        symlink(
            outside.path().join("outside.mara.md"),
            root.join("new.mara.md"),
        )
        .unwrap();
        let original = journal(root, true);
        write_journal(root, &original);
        assert!(
            invoke(root, through_mcp, false)
                .unwrap_err()
                .contains("symlinks")
        );
        assert_eq!(
            fs::read_to_string(root.join("source.mara.md")).unwrap(),
            LEGACY
        );
        assert_eq!(
            fs::read_to_string(outside.path().join("outside.mara.md")).unwrap(),
            "outside"
        );
        assert_eq!(
            fs::read_to_string(root.join(".mara/transaction.json")).unwrap(),
            original.to_string()
        );
    }
}
