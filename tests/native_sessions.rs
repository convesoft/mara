//! Real NativeRuntime calls, disk sources and configured subprocess indexing.
mod support;
use mara::{
    runtime::{NativeSessions, SessionPolicy},
    *,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}
fn runtime(root: &Path) -> NativeRuntime {
    NativeRuntime::new(Some(root.to_owned()), root.to_owned()).unwrap()
}
fn document(id: &str, mid: &str, title: &str) -> String {
    format!("# Heading\n\n:::mara requirement {id}\n:mid: {mid}\n:title: {title}\n\nBody.\n:::\n")
}
fn fixture() -> tempfile::TempDir {
    let f = support::fixture();
    initialize_project(f.path(), Template::Minimal).unwrap();
    fs::write(
        f.path().join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "First"),
    )
    .unwrap();
    fs::write(
        f.path().join("b.mara.md"),
        document("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "Second"),
    )
    .unwrap();
    f
}
fn get_a(r: &NativeRuntime) -> GetResult {
    r.get(decode(json!({"reference":"REQ-A"}))).unwrap()
}

// @mara checks REQ-FRESH-PROJECT-SESSIONS
// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn unchanged_operations_reuse_actual_parsing_discovery_relations_and_compiled_rules() {
    let f = fixture();
    let root = f.path();
    let r = runtime(root);
    let config = fs::read_to_string(root.join(PROJECT_FILE))
        .unwrap()
        .replacen("format_version = 1", "format_version = 2", 1)
        + "\n[rules]\nformat_version = 1\nfiles = ['rules.yaml']\n";
    fs::write(root.join(PROJECT_FILE), config).unwrap();
    fs::write(root.join("rules.yaml"), "- id: rule:body\n  targetClass: requirement\n  property:\n    - {path: status, minCount: 1}\n").unwrap();
    let schema = fs::read_to_string(root.join(SCHEMA_FILE))
        .unwrap()
        .replace("fields: {}", "fields: {status: {type: string}}");
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    for path in ["a.mara.md", "b.mara.md"] {
        let source = fs::read_to_string(root.join(path))
            .unwrap()
            .replace("\n\nBody.", "\n:status: draft\n\nBody.");
        fs::write(root.join(path), source).unwrap();
    }
    let trace = decode(json!({"all":true,"rules":["urn:mara:rule:body"]}));
    let start = Instant::now();
    get_a(&r);
    assert!(r.project_validate(&[]).unwrap().valid);
    assert!(r.trace_matrix(&trace).unwrap().evaluation_complete);
    r.related(decode(json!({"reference":"REQ-A"}))).unwrap();
    let cold = start.elapsed();
    let before = r.session_activity().unwrap();
    assert!(before.document_parses >= 2 && before.code_loads > 0 && before.rule_compilations > 0);
    assert!(before.discovery_builds > 0 && before.relation_builds > 0);
    let start = Instant::now();
    get_a(&r);
    assert!(r.project_validate(&[]).unwrap().valid);
    assert!(r.trace_matrix(&trace).unwrap().evaluation_complete);
    r.search(decode(json!({"query":"First"}))).unwrap();
    r.related(decode(json!({"reference":"REQ-B"}))).unwrap();
    assert_eq!(before, r.session_activity().unwrap());
    println!("cold={cold:?}, warm={:?}, work={before:?}", start.elapsed());
    // Request-local sources and parameter-specific compilation are also reused.
    fs::write(root.join("check.yaml"), "- id: rule:title\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected}}\n").unwrap();
    let check = decode(
        json!({"ids":["REQ-A"],"check":{"files":["check.yaml"],"shape":"urn:mara:rule:title","parameters":{"expected":"draft"}}}),
    );
    assert_eq!(r.trace_matrix(&check).unwrap().summaries[0].passed, 1);
    let before = r.session_activity().unwrap();
    assert_eq!(r.trace_matrix(&check).unwrap().summaries[0].passed, 1);
    assert_eq!(before, r.session_activity().unwrap());
    fs::write(root.join("rules.yaml"), "- id: rule:body\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: Missing}\n").unwrap();
    assert!(!r.project_validate(&[]).unwrap().valid);
    fs::write(root.join("rules.yaml"), "invalid: [").unwrap();
    assert!(!r.schema_validate().unwrap().valid);
}

// @mara checks REQ-FRESH-PROJECT-SESSIONS
#[test]
fn external_content_inventory_config_schema_and_ignore_changes_never_hide_current_errors() {
    let f = fixture();
    let root = f.path();
    let r = runtime(root);
    get_a(&r);
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Fresh"),
    )
    .unwrap();
    assert_eq!(get_a(&r).node.title.as_deref(), Some("Fresh"));
    fs::write(root.join(".gitignore"), "b.mara.md\n").unwrap();
    assert_eq!(r.item_list(Default::default()).unwrap().items.len(), 1);
    fs::remove_file(root.join(".gitignore")).unwrap();
    assert_eq!(r.item_list(Default::default()).unwrap().items.len(), 2);
    fs::write(
        root.join("c.mara.md"),
        document("REQ-C", "01ARZ3NDEKTSV4RRFFQ69G5F02", "New"),
    )
    .unwrap();
    assert_eq!(r.item_list(Default::default()).unwrap().items.len(), 3);
    fs::remove_file(root.join("c.mara.md")).unwrap();
    assert_eq!(r.item_list(Default::default()).unwrap().items.len(), 2);
    let schema = fs::read_to_string(root.join(SCHEMA_FILE)).unwrap();
    fs::write(root.join(SCHEMA_FILE), "invalid: [").unwrap();
    assert!(r.item_list(Default::default()).is_err());
    assert!(!r.schema_validate().unwrap().valid);
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    get_a(&r);
    let config = fs::read_to_string(root.join(PROJECT_FILE)).unwrap();
    fs::write(root.join(PROJECT_FILE), "invalid = [").unwrap();
    assert!(r.schema_get(None, None).is_err());
    assert!(!r.project_validate(&[]).unwrap().valid);
    fs::write(root.join(PROJECT_FILE), config).unwrap();
    get_a(&r);
    fs::write(
        root.join("a.mara.md"),
        ":::mara requirement REQ-A\n:title: Unclosed\n",
    )
    .unwrap();
    assert!(r.item_list(Default::default()).is_err());
    assert!(!r.project_validate(&[]).unwrap().valid);
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Recovered"),
    )
    .unwrap();
    assert_eq!(get_a(&r).node.title.as_deref(), Some("Recovered"));
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn writes_refresh_eviction_and_restart_keep_source_owned_cursor_and_handle_contracts() {
    let f = fixture();
    let root = f.path();
    let r = runtime(root);
    let page = r.item_list(decode(json!({"limit":1}))).unwrap();
    let cursor = page.next_cursor.unwrap();
    let search = r.search(decode(json!({"query":"Heading"}))).unwrap();
    let handle = search
        .results
        .iter()
        .find(|n| n.node.kind == DiscoveryKind::Section)
        .unwrap()
        .node
        .reference
        .clone();
    let original = fs::read(root.join("a.mara.md")).unwrap();
    let work = r.session_activity().unwrap();
    r.force_refresh().unwrap();
    get_a(&r);
    assert!(r.session_activity().unwrap().document_parses > work.document_parses);
    for candidate in [&r, &runtime(root)] {
        candidate
            .item_list(decode(json!({"limit":1,"cursor":cursor})))
            .unwrap();
        candidate.get(decode(json!({"reference":handle}))).unwrap();
    }
    let evicting = NativeSessions::new(SessionPolicy {
        max_idle_sessions: 0,
        idle_timeout: Duration::ZERO,
    });
    let e = runtime(root).with_sessions(evicting.clone());
    e.get(decode(json!({"reference":handle}))).unwrap();
    assert_eq!(evicting.retained_sessions(), 0);
    e.item_list(decode(json!({"limit":1,"cursor":cursor})))
        .unwrap();
    assert_eq!(fs::read(root.join("a.mara.md")).unwrap(), original);
    get_a(&r);
    let work = r.session_activity().unwrap();
    assert!(
        r.item_update(decode(
            json!({"reference":"REQ-A","body":"[[REQ-MISSING]]"})
        ))
        .is_err()
    );
    get_a(&r);
    assert!(r.session_activity().unwrap().document_parses > work.document_parses);
    let work = r.session_activity().unwrap();
    r.project_transaction_rollback().unwrap();
    get_a(&r);
    assert!(r.session_activity().unwrap().document_parses > work.document_parses);
    r.item_update(decode(json!({"reference":"REQ-A","title":"Written"})))
        .unwrap();
    assert_eq!(get_a(&r).node.title.as_deref(), Some("Written"));
    assert!(
        r.item_list(decode(json!({"limit":1,"cursor":cursor})))
            .is_err()
    );
    assert!(r.get(decode(json!({"reference":handle}))).is_err());
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn canonical_aliases_share_but_actual_git_worktrees_are_isolated() {
    let f = support::fixture();
    let parent = f.path();
    let left = parent.join("left");
    let right = parent.join("right");
    fs::create_dir(&left).unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Session Test",
            "-c",
            "user.email=session@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
        vec![
            "worktree",
            "add",
            "--quiet",
            right.to_str().unwrap(),
            "-b",
            "other",
        ],
    ] {
        if args[0] == "add" {
            initialize_project(&left, Template::Minimal).unwrap();
            fs::write(
                left.join("a.mara.md"),
                document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Left"),
            )
            .unwrap();
        }
        let o = support::isolated_command("git", &left)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", support::stderr(&o));
    }
    fs::write(
        right.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Right"),
    )
    .unwrap();
    let common = |root: &Path| {
        let output = support::isolated_command("git", root)
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .output()
            .unwrap();
        assert!(output.status.success());
        support::stdout(&output)
    };
    assert_eq!(common(&left), common(&right));
    let pool = NativeSessions::default();
    let a = runtime(&left).with_sessions(pool.clone());
    let b = runtime(&right).with_sessions(pool.clone());
    assert_eq!(get_a(&a).node.title.as_deref(), Some("Left"));
    assert_eq!(get_a(&b).node.title.as_deref(), Some("Right"));
    let before = a.session_activity().unwrap();
    let alias = runtime(&left.join(".")).with_sessions(pool.clone());
    get_a(&alias);
    assert_eq!(before, alias.session_activity().unwrap());
    assert_eq!(pool.retained_sessions(), 2);
    b.force_refresh().unwrap();
    get_a(&a);
    assert_eq!(before, a.session_activity().unwrap());
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn ignored_file_only_inputs_remain_fresh_without_a_language_indexer() {
    let f = fixture();
    let root = f.path();
    let r = runtime(root);
    let schema = fs::read_to_string(root.join(SCHEMA_FILE)).unwrap()
        + "  covered_by_code:\n    description: Implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: code_coverage\n";
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    let source = fs::read_to_string(root.join("a.mara.md")).unwrap().replace(
        ":title: First",
        ":title: First\n:code_coverage: code:ignored.txt",
    );
    fs::write(root.join("a.mara.md"), source).unwrap();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("ignored.txt"), "old").unwrap();
    let params = || {
        decode(
            json!({"reference":"REQ-A","direction":"incoming","relations":["schema:covered_by_code"]}),
        )
    };
    assert_eq!(r.related(params()).unwrap().connections.len(), 1);
    let before = r.session_activity().unwrap();
    assert_eq!(r.related(params()).unwrap().connections.len(), 1);
    assert_eq!(before, r.session_activity().unwrap());
    fs::write(root.join("ignored.txt"), "fresh").unwrap();
    assert_eq!(
        r.get(decode(json!({"reference":"code:ignored.txt"})))
            .unwrap()
            .content,
        "fresh"
    );
    fs::remove_file(root.join("ignored.txt")).unwrap();
    assert!(r.related(params()).is_err());
    assert!(!r.project_validate(&[]).unwrap().valid);
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
// macOS filesystems reject the non-UTF-8 directory names used by this fixture.
#[test]
#[cfg(target_os = "linux")]
fn native_cache_keys_preserve_non_utf8_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let f = support::fixture();
    let root = f.path().join(OsString::from_vec(b"project-\xff".to_vec()));
    let project = initialize_project(&root, Template::Minimal).unwrap();
    let r = runtime(&root);
    r.schema_get(None, None).unwrap();
    r.schema_get(None, None).unwrap();
    let output = support::mara(&root, &["--format", "json", "schema", "get"]);
    assert!(output.status.success(), "{}", support::stderr(&output));
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "First"),
    )
    .unwrap();
    let nested = root.join(OsString::from_vec(b"nested-\xfe".to_vec()));
    fs::create_dir(&nested).unwrap();
    fs::write(
        nested.join("b.mara.md"),
        document("REQ-B", "01ARZ3NDEKTSV4RRFFQ69G5F01", "Second"),
    )
    .unwrap();
    let schema = load_schema(&project).unwrap();
    // Both roots and discovered parent directories may have native non-UTF-8 bytes.
    assert_eq!(
        load_documents(&project, &schema).unwrap().documents.len(),
        2
    );
    // An operation may diagnose unsupported result paths, but must not panic/poison
    // the session before that owning operation can decide its path contract.
    let _ = r.item_list(Default::default());
    r.schema_get(None, None).unwrap();
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn parameterized_rule_retention_is_bounded_and_recent_variants_reuse() {
    let f = fixture();
    let root = f.path();
    let schema = fs::read_to_string(root.join(SCHEMA_FILE))
        .unwrap()
        .replace("fields: {}", "fields: {status: {type: string}}");
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    fs::write(root.join("check.yaml"), "- id: rule:status\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected}}\n").unwrap();
    let r = runtime(root);
    let check = |n| {
        decode(json!({"ids":["REQ-A"],"check":{
            "files":["check.yaml"],"shape":"urn:mara:rule:status",
            "parameters":{"expected":format!("{n}-{}", "x".repeat(2048))}
        }}))
    };
    for n in 0..40 {
        assert!(r.trace_matrix(&check(n)).unwrap().evaluation_complete);
    }
    let before = r.session_activity().unwrap();
    assert!(r.trace_matrix(&check(39)).unwrap().evaluation_complete);
    assert_eq!(before, r.session_activity().unwrap());
    assert!(r.trace_matrix(&check(0)).unwrap().evaluation_complete);
    assert_eq!(
        r.session_activity().unwrap().rule_compilations,
        before.rule_compilations + 1
    );
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn request_checks_do_not_accumulate_root_dependencies() {
    let f = fixture();
    let root = f.path();
    let schema = fs::read_to_string(root.join(SCHEMA_FILE))
        .unwrap()
        .replace("fields: {}", "fields: {status: {type: string}}");
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    fs::write(root.join(".gitignore"), "checks/\n").unwrap();
    fs::create_dir(root.join("checks")).unwrap();
    let r = runtime(root);
    let check = |n| {
        decode(json!({"ids":["REQ-A"],"check":{
            "files":[format!("checks/{n}.yaml")],"shape":"urn:mara:rule:status"
        }}))
    };
    for n in 0..40 {
        assert!(!r.trace_matrix(&check(n)).unwrap().evaluation_complete);
    }
    get_a(&r);
    let before = r.session_activity().unwrap();
    let source =
        "- id: rule:status\n  class: requirement\n  property:\n    - {path: status, minCount: 0}\n";
    // An evicted check's formerly missing, ignored path is not a root dependency.
    fs::write(root.join("checks/0.yaml"), source).unwrap();
    get_a(&r);
    assert_eq!(before, r.session_activity().unwrap());
    // Retained variants still re-read request sources, including ignored files.
    fs::write(root.join("checks/39.yaml"), source).unwrap();
    assert_eq!(r.trace_matrix(&check(39)).unwrap().summaries[0].passed, 1);
    fs::write(
        root.join("checks/39.yaml"),
        source.replace("minCount: 0", "minCount: 1"),
    )
    .unwrap();
    assert_eq!(r.trace_matrix(&check(39)).unwrap().summaries[0].failed, 1);
    assert_eq!(
        before.document_parses,
        r.session_activity().unwrap().document_parses
    );
}

// @mara checks REQ-FRESH-PROJECT-SESSIONS
#[test]
#[cfg(unix)]
fn retired_ignored_dependencies_do_not_disable_reuse() {
    use std::os::unix::fs::symlink;
    let f = fixture();
    let root = f.path();
    let original = fs::read_to_string(root.join("a.mara.md")).unwrap();
    let schema = fs::read_to_string(root.join(SCHEMA_FILE)).unwrap()
        + "  covered_by_code:\n    description: Implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: code_coverage\n";
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    fs::write(root.join(".gitignore"), "retired.txt\n").unwrap();
    fs::write(root.join("retired.txt"), "former dependency").unwrap();
    fs::write(
        root.join("a.mara.md"),
        original.replace(
            ":title: First",
            ":title: First\n:code_coverage: code:retired.txt",
        ),
    )
    .unwrap();
    let r = runtime(root);
    get_a(&r);
    let old_work = r.session_activity().unwrap();
    fs::write(root.join("a.mara.md"), original).unwrap();
    fs::remove_file(root.join("retired.txt")).unwrap();
    // An ignored, now-unused path cannot be inspected, even when tests run as root.
    symlink("retired.txt", root.join("retired.txt")).unwrap();
    assert!(fs::canonicalize(root.join("retired.txt")).is_err());
    get_a(&r);
    let before = r.session_activity().unwrap();
    assert!(before.document_parses > old_work.document_parses);
    get_a(&r);
    assert_eq!(before, r.session_activity().unwrap());
    r.force_refresh().unwrap();
    get_a(&r);
    let before = r.session_activity().unwrap();
    assert!(before.document_parses > 0 && before.code_loads > 0);
    get_a(&r);
    assert_eq!(before, r.session_activity().unwrap());
}

// @mara checks DES-NATIVE-PROJECT-SESSIONS
#[test]
fn idle_retention_prunes_lru_and_expired_state_without_touching_source() {
    let a = fixture();
    let b = fixture();
    let pool = NativeSessions::new(SessionPolicy {
        max_idle_sessions: 1,
        idle_timeout: Duration::from_secs(300),
    });
    let r = runtime(a.path()).with_sessions(pool.clone());
    get_a(&r);
    get_a(&runtime(b.path()).with_sessions(pool.clone()));
    assert_eq!(pool.retained_sessions(), 1);
    assert_eq!(r.session_activity().unwrap().document_parses, 0);
    assert!(a.path().join("a.mara.md").is_file());
    let pool = NativeSessions::new(SessionPolicy {
        max_idle_sessions: 16,
        idle_timeout: Duration::from_millis(5),
    });
    get_a(&runtime(a.path()).with_sessions(pool.clone()));
    std::thread::sleep(Duration::from_millis(10));
    pool.prune_idle();
    assert_eq!(pool.retained_sessions(), 0);
}

// @mara checks DES-CODE-TRACEABILITY
// @mara checks REQ-FRESH-PROJECT-SESSIONS
#[test]
#[ignore = "requires installed real scip-typescript; run explicitly for acceptance"]
fn real_indexer_activity() {
    let f = support::fixture();
    let output =
        support::isolated_command(std::env::current_exe().unwrap().to_str().unwrap(), f.path())
            .args([
                "--ignored",
                "--exact",
                "real_indexer_activity_child",
                "--nocapture",
            ])
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        support::stdout(&output),
        support::stderr(&output)
    );
    println!("{}", support::stdout(&output));
}
#[test]
#[ignore = "isolated subprocess helper"]
fn real_indexer_activity_child() {
    let f = support::fixture();
    let outer = f.path();
    let root = outer.join("project");
    fs::create_dir(&root).unwrap();
    initialize_project(&root, Template::Minimal).unwrap();
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Real"),
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"native-sessions\",\"version\":\"1.0.0\",\"private\":true}",
    )
    .unwrap();
    fs::write(
        root.join("tsconfig.json"),
        "{\"compilerOptions\":{\"target\":\"ES2020\"},\"include\":[\"service.ts\"]}",
    )
    .unwrap();
    let script = outer.join("index.sh");
    let count = outer.join("count");
    let entered = outer.join("entered");
    let release = outer.join("release");
    let pause = outer.join("pause");
    let command = format!(
        "#!/bin/sh\nset -eu\nprintf 'index\\n' >> '{}'\nif test -f '{}'; then touch '{}'; while ! test -f '{}'; do sleep 0.02; done; fi\nscip-typescript index --no-progress-bar --output \"$1\"\ncp \"$1\" '{}'\n",
        count.display(),
        pause.display(),
        entered.display(),
        release.display(),
        outer.join("index.scip").display()
    );
    fs::write(&script, &command).unwrap();
    let config_path = root.join(PROJECT_FILE);
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    ) + &format!(
        "\n[[code.languages]]\nname='typescript'\nextensions=['ts']\ncommand=['sh', '{}', '{{output}}']\n",
        script.display()
    );
    fs::write(&config_path, &config).unwrap();
    let runs = || {
        fs::read_to_string(&count)
            .unwrap_or_default()
            .lines()
            .count()
    };
    let r = runtime(&root);
    get_a(&r);
    assert_eq!(runs(), 0); // Empty language skips actual invocation.
    fs::write(
        root.join("service.ts"),
        "export function run(): number { return 1; }\n",
    )
    .unwrap();
    let cold = Instant::now();
    get_a(&r);
    let cold = cold.elapsed();
    assert_eq!(runs(), 1);
    use protobuf::Message;
    let index =
        scip::types::Index::parse_from_bytes(&fs::read(outer.join("index.scip")).unwrap()).unwrap();
    let occurrence = index
        .documents
        .iter()
        .find(|d| d.relative_path == "service.ts")
        .unwrap()
        .occurrences
        .iter()
        .find(|o| {
            o.symbol_roles & 1 != 0
                && o.symbol.ends_with("run().")
                && !scip::symbol::is_local_symbol(&o.symbol)
        })
        .unwrap();
    let descriptor = scip::symbol::format_symbol_with(
        scip::symbol::parse_symbol(&occurrence.symbol).unwrap(),
        scip::symbol::SymbolFormatOptions {
            include_scheme: false,
            include_package_manager: false,
            include_package_name: false,
            include_package_version: false,
            include_descriptor: true,
        },
    );
    let reference = format!("code:service.ts::typescript::{descriptor}");
    assert!(reference.contains("typescript::"));
    let before = r.session_activity().unwrap();
    let warm = Instant::now();
    r.get(decode(json!({"reference":reference}))).unwrap();
    get_a(&r);
    assert_eq!(before, r.session_activity().unwrap());
    assert_eq!(runs(), 1);
    println!(
        "real scip-typescript cold={cold:?} warm={:?} work={before:?}",
        warm.elapsed()
    );
    fs::write(
        root.join("service.ts"),
        "export function fresh(): number { return 2; }\n",
    )
    .unwrap();
    get_a(&r);
    assert_eq!(runs(), 2);
    assert!(r.get(decode(json!({"reference":reference}))).is_err()); // No descriptor fallback.
    fs::write(root.join("input.txt"), "changed non-source indexer input").unwrap();
    get_a(&r);
    assert_eq!(runs(), 3);
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/code");
    fs::create_dir(root.join("assets")).unwrap();
    for ext in ["wasm", "scm"] {
        fs::copy(
            assets.join(format!("typescript.{ext}")),
            root.join(format!("assets/typescript.{ext}")),
        )
        .unwrap();
    }
    fs::write(root.join(".gitignore"), "assets/\n").unwrap();
    let grammar_config =
        config.clone() + "grammar='assets/typescript.wasm'\nquery='assets/typescript.scm'\n";
    fs::write(&config_path, &grammar_config).unwrap();
    get_a(&r);
    assert_eq!(runs(), 4);
    let query_path = root.join("assets/typescript.scm");
    let query = fs::read_to_string(&query_path).unwrap();
    fs::write(&query_path, format!("{query}\n; changed query\n")).unwrap();
    get_a(&r);
    assert_eq!(runs(), 5);
    let grammar_path = root.join("assets/typescript.wasm");
    let grammar = fs::read(&grammar_path).unwrap();
    fs::write(&grammar_path, b"invalid grammar").unwrap();
    assert!(r.item_list(Default::default()).is_err());
    assert_eq!(runs(), 5);
    fs::write(&grammar_path, grammar).unwrap();
    get_a(&r);
    assert_eq!(runs(), 6);
    // The script is outside recorded project inputs: explicit refresh is necessary.
    fs::write(&script, "#!/bin/sh\nexit 7\n").unwrap();
    get_a(&r);
    assert_eq!(runs(), 6);
    r.force_refresh().unwrap();
    assert!(r.item_list(Default::default()).is_err());
    assert!(!r.project_validate(&[]).unwrap().evaluation_complete);
    fs::write(&script, &command).unwrap();
    r.force_refresh().unwrap();
    get_a(&r);
    assert_eq!(runs(), 7);
    fs::write(
        &config_path,
        grammar_config.replace(
            "command=['sh'",
            "command=['nonexistent-native-sessions-command'",
        ),
    )
    .unwrap();
    assert!(r.item_list(Default::default()).is_err());
    fs::write(&config_path, &grammar_config).unwrap();
    get_a(&r);
    assert_eq!(runs(), 8);
    fs::remove_file(root.join("service.ts")).unwrap();
    get_a(&r);
    assert_eq!(runs(), 8);
    fs::write(
        root.join("service.ts"),
        "export function run(): number { return 1; }\n",
    )
    .unwrap();
    get_a(&r);
    assert_eq!(runs(), 9);
    // Pruning during a real indexer call cannot evict its in-flight lease.
    let pool = NativeSessions::new(SessionPolicy {
        max_idle_sessions: 0,
        idle_timeout: Duration::ZERO,
    });
    let active = runtime(&root).with_sessions(pool.clone());
    fs::write(&pause, "pause").unwrap();
    let worker = std::thread::spawn(move || get_a(&active));
    let deadline = Instant::now() + Duration::from_secs(60);
    while !entered.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let observed = entered.exists();
    let retained = pool.retained_sessions();
    let pruned = pool.prune_idle();
    fs::write(&release, "release").unwrap();
    worker.join().unwrap();
    assert!(observed);
    assert_eq!(retained, 1);
    assert_eq!(pruned, 0);
    assert_eq!(pool.retained_sessions(), 0);
    assert_eq!(runs(), 10);
    // An indexer owns discovery and may emit ignored files absent from activation.
    fs::remove_file(&pause).unwrap();
    fs::write(root.join(".gitignore"), "assets/\nignored.ts\n").unwrap();
    fs::write(
        root.join("ignored.ts"),
        "export function hidden(): number { return 3; }\n",
    )
    .unwrap();
    fs::write(
        root.join("tsconfig.json"),
        "{\"compilerOptions\":{\"target\":\"ES2020\"},\"include\":[\"service.ts\",\"ignored.ts\"]}",
    )
    .unwrap();
    let schema = fs::read_to_string(root.join(SCHEMA_FILE)).unwrap()
        + "  covered_by_code:\n    description: Implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: code_coverage\n";
    fs::write(root.join(SCHEMA_FILE), schema).unwrap();
    let old_reference = "code:ignored.ts::typescript::`ignored.ts`/hidden().";
    let source = fs::read_to_string(root.join("a.mara.md")).unwrap().replace(
        ":title: Real",
        &format!(":title: Real\n:code_coverage: {old_reference}"),
    );
    fs::write(root.join("a.mara.md"), source).unwrap();
    assert!(r.project_validate(&[]).unwrap().valid);
    r.get(decode(json!({"reference":old_reference}))).unwrap();
    let before = runs();
    assert!(r.project_validate(&[]).unwrap().valid);
    assert_eq!(runs(), before);
    fs::write(
        root.join("ignored.ts"),
        "export function renamed(): number { return 4; }\n",
    )
    .unwrap();
    assert!(r.get(decode(json!({"reference":old_reference}))).is_err());
    assert!(runs() > before);
    assert!(!r.project_validate(&[]).unwrap().valid);
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        // Isolate dependency retirement from first-discovery SCIP verification.
        fs::write(
            root.join("tsconfig.json"),
            "{\"compilerOptions\":{\"target\":\"ES2020\"},\"include\":[\"service.ts\"]}",
        )
        .unwrap();
        fs::write(
            root.join(".gitignore"),
            "assets/\nignored.ts\nretired.txt\n",
        )
        .unwrap();
        fs::write(root.join("retired.txt"), "former dependency").unwrap();
        let original = document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Real");
        fs::write(
            root.join("a.mara.md"),
            original.replace(
                ":title: Real",
                ":title: Real\n:code_coverage: code:retired.txt",
            ),
        )
        .unwrap();
        get_a(&r);
        let before = runs();
        get_a(&r);
        assert_eq!(runs(), before);
        fs::write(root.join("a.mara.md"), original).unwrap();
        fs::remove_file(root.join("retired.txt")).unwrap();
        symlink("retired.txt", root.join("retired.txt")).unwrap();
        get_a(&r);
        assert_eq!(runs(), before + 1);
        get_a(&r);
        assert_eq!(runs(), before + 1);
        r.force_refresh().unwrap();
        get_a(&r);
        assert_eq!(runs(), before + 2);
        get_a(&r);
        assert_eq!(runs(), before + 2);
    }
    // An indexer may omit source text and discover an ignored file for the first
    // time. A concurrent edit after indexing must not become a reusable stale hit.
    fs::write(
        root.join("tsconfig.json"),
        "{\"compilerOptions\":{\"target\":\"ES2020\"},\"include\":[\"service.ts\",\"ignored.ts\"]}",
    )
    .unwrap();
    let old_reference = "code:ignored.ts::typescript::`ignored.ts`/oldname().";
    fs::write(
        root.join("ignored.ts"),
        "export function oldname(): number { return 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("a.mara.md"),
        document("REQ-A", "01ARZ3NDEKTSV4RRFFQ69G5F00", "Real").replace(
            ":title: Real",
            &format!(":title: Real\n:code_coverage: {old_reference}"),
        ),
    )
    .unwrap();
    let racing_command = format!(
        "{command}printf 'export function newname(): number {{ return 2; }}\\n' > ignored.ts\n"
    );
    fs::write(&script, &racing_command).unwrap();
    r.force_refresh().unwrap();
    let _ = r.project_validate(&[]).unwrap(); // No promise of an atomic initial read.
    let indexed =
        scip::types::Index::parse_from_bytes(&fs::read(outer.join("index.scip")).unwrap()).unwrap();
    assert!(
        indexed
            .documents
            .iter()
            .find(|d| d.relative_path == "ignored.ts")
            .unwrap()
            .text
            .is_empty()
    );
    let before = runs();
    assert!(!r.project_validate(&[]).unwrap().valid);
    assert_eq!(runs(), before + 1);
    assert!(!r.project_validate(&[]).unwrap().valid);
    assert_eq!(runs(), before + 1); // The stable second execution is now eligible.
    assert!(r.get(decode(json!({"reference":old_reference}))).is_err());
    r.get(decode(
        json!({"reference":"code:ignored.ts::typescript::`ignored.ts`/newname()."}),
    ))
    .unwrap();
    println!(
        "real indexer invocations={} with freshness, assets, activation, external refresh, failures, retired dependencies, concurrent ignored-source edits and active eviction checked",
        runs()
    );
    if let Some(evidence) = std::env::var_os("MARA_SESSION_PROBE_EVIDENCE") {
        let evidence = PathBuf::from(evidence);
        fs::create_dir_all(&evidence).unwrap();
        for name in [
            "package.json",
            "tsconfig.json",
            "service.ts",
            "ignored.ts",
            ".gitignore",
            "a.mara.md",
        ] {
            fs::copy(root.join(name), evidence.join(name)).unwrap();
        }
        fs::copy(&config_path, evidence.join("project.toml")).unwrap();
        fs::copy(root.join(SCHEMA_FILE), evidence.join("schema.yaml")).unwrap();
        fs::write(evidence.join("index.sh"), command).unwrap();
        fs::write(evidence.join("racing-index.sh"), racing_command).unwrap();
        fs::copy(&count, evidence.join("count")).unwrap();
        fs::copy(outer.join("index.scip"), evidence.join("index.scip")).unwrap();
        fs::copy(&query_path, evidence.join("typescript.scm")).unwrap();
        fs::copy(&grammar_path, evidence.join("typescript.wasm")).unwrap();
    }
}
