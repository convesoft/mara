use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn retrieval_fixture() -> TempDir {
    let fixture = fixture();
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
fn directory_retrieval_fixture() -> TempDir {
    let fixture = retrieval_fixture();
    for (path, source) in [
        (
            "packages/query/docs/a.mara.md",
            ":::mara requirement REQ-PACK-A\n:title: Storage\n:status: draft\n:derives_from: SCN-BASE\n\nCache entries.\n:::\n\n:::mara requirement REQ-PACK-E\n:title: Cache\n:status: accepted\n:derives_from: SCN-BASE\n\nCache accepted.\n:::\n",
        ),
        (
            "packages/query/docs/nested/b.mara.md",
            ":::mara requirement REQ-PACK-B\n:title: Cache\n:status: draft\n:derives_from: SCN-BASE\n\nNested entry.\n:::\n\n:::mara requirement REQ-PACK-C\n:title: Cahce\n:status: draft\n:derives_from: SCN-BASE\n\nTypo entry.\n:::\n",
        ),
        (
            "packages/query/docs-extra/a.mara.md",
            ":::mara requirement REQ-DOCS-SIBLING\n:title: Cache\n\nSibling directory.\n:::\n",
        ),
        (
            "packages/query-extra/docs/a.mara.md",
            ":::mara requirement REQ-PACK-SIBLING\n:title: Cache\n\nSibling package.\n:::\n",
        ),
        (
            "packages/dicom-viewer/docs/a.mara.md",
            ":::mara requirement REQ-VIEWER\n:title: Cache\n:status: draft\n:derives_from: SCN-BASE\n\nOther package.\n:::\n",
        ),
    ] {
        let path = fixture.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    fixture
}

// @mara checks REQ-ITEM-LIST
#[test]
fn oversized_item_handles_fail_without_silent_omission_or_unbounded_diagnostics() {
    let fixture = retrieval_fixture();
    let long_id = format!("REQ-{}", "A".repeat(66_000));
    fs::write(
        fixture.path().join("docs/oversized.mara.md"),
        format!(
            ":::mara requirement {long_id}\n:title: Oversized identity\n\nNeed knowledge.\n:::\n"
        ),
    )
    .unwrap();
    let output = mara(
        fixture.path(),
        &["item", "list", "--path", "docs/oversized.mara.md"],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("shorten oversized identity/location fields"),
        "{}",
        stderr(&output)
    );
    assert!(output.stderr.len() < 1024);
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_list", json!({"paths":["docs/oversized.mara.md"]})),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert!(serde_json::to_vec(result).unwrap().len() < 1024);
}

// @mara checks REQ-ITEM-LIST
#[test]
fn item_list_returns_deterministic_compact_filtered_summaries() {
    let fixture = retrieval_fixture();

    let listed = mara(
        fixture.path(),
        &["item", "list", "--flavour", "requirement"],
    );
    assert!(listed.status.success(), "{}", stderr(&listed));
    assert_eq!(
        stdout(&listed),
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\nREQ-BETA\trequirement\tBeta requirement\tdocs/b.mara.md:1\npage\thas_more=false\n"
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
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\npage\thas_more=false\n"
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
        "SCN-BASE\tscenario\tBase scenario\tdocs/a.mara.md:1\nREQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\npage\thas_more=false\n"
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
}

fn page(root: &Path, arguments: &[&str], params: Value) -> Value {
    let mut args = vec!["--format", "json", "item", "list"];
    args.extend(arguments);
    let output = mara(root, &args);
    assert!(
        output.status.success(),
        "{} {}",
        stdout(&output),
        stderr(&output)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_list", params),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        value
    );
    assert!(serde_json::to_vec(&value).unwrap().len() <= 65_536);
    value
}

fn rejects(root: &Path, arguments: &[&str], params: Value, message: &str) {
    let mut args = vec!["item", "list"];
    args.extend(arguments);
    let output = mara(root, &args);
    assert!(!output.status.success());
    assert!(stderr(&output).contains(message), "{}", stderr(&output));
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_list", params),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
}

fn ids(value: &Value) -> Vec<String> {
    value["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

// @mara checks DES-ITEM-LIST
#[test]
fn directory_filters_preserve_component_boundaries_and_normalization() {
    let fixture = directory_retrieval_fixture();
    let selected = vec!["REQ-PACK-A", "REQ-PACK-E", "REQ-PACK-B", "REQ-PACK-C"];
    for (paths, expected) in [
        (vec!["packages/query/docs"], selected.clone()),
        (vec!["./packages//query/./docs/"], selected.clone()),
        (
            vec!["packages/query/docs", "packages/query/docs/nested"],
            selected,
        ),
        (
            vec!["packages/query/docs/a.mara.md"],
            vec!["REQ-PACK-A", "REQ-PACK-E"],
        ),
        (vec!["packages/query/docs/a.mara"], vec![]),
        (vec!["packages/missing"], vec![]),
        (
            vec!["packages/dicom-viewer", "packages/query/docs/nested"],
            vec!["REQ-VIEWER", "REQ-PACK-B", "REQ-PACK-C"],
        ),
    ] {
        let mut args = vec![];
        for path in &paths {
            args.extend(["--path", *path]);
        }
        let value = page(
            &fixture.path().join("packages/query"),
            &args,
            json!({"paths":paths}),
        );
        assert_eq!(ids(&value), expected);
        assert_eq!(value["has_more"], false);
    }
    for path in [
        ".",
        "./",
        "packages/query/../dicom-viewer",
        "/packages/query",
    ] {
        rejects(
            fixture.path(),
            &["--path", path],
            json!({"paths":[path]}),
            "path filter must be a project-relative path",
        );
    }
    rejects(
        fixture.path(),
        &["--path", ""],
        json!({"paths":[""]}),
        "a value is required",
    );
}

// @mara implements VER-ITEM-LIST
// @mara checks REQ-ITEM-LIST
#[test]
fn list_pages_continue_completely_with_cli_mcp_parity_and_no_source_writes() {
    let fixture = retrieval_fixture();
    let source = (0..45)
        .map(|i| {
            format!(
                ":::mara requirement REQ-PAGE-{i}\n:title: Page {i}\n\nBounded knowledge.\n:::\n\n"
            )
        })
        .collect::<String>();
    let file = fixture.path().join("docs/pages.mara.md");
    fs::write(&file, &source).unwrap();
    let mut cursor: Option<String> = None;
    let mut found = vec![];
    loop {
        assert!(found.len() < 50);
        let mut args = vec!["--path", "docs/pages.mara.md", "--limit", "7"];
        let mut params = json!({"paths":["docs/pages.mara.md"],"limit":7});
        if let Some(c) = &cursor {
            args.extend(["--cursor", c]);
            params["cursor"] = json!(c);
        }
        let value = page(fixture.path(), &args, params);
        let items = value["items"].as_array().unwrap();
        assert!(!items.is_empty() && items.len() <= 7);
        for item in items {
            assert!(item.get("body").is_none() && item.get("excerpts").is_none());
            assert_eq!(item["mid"], Value::Null);
        }
        found.extend(ids(&value));
        assert_eq!(value["has_more"], !value["next_cursor"].is_null());
        cursor = value["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        found,
        (0..45).map(|i| format!("REQ-PAGE-{i}")).collect::<Vec<_>>()
    );
    assert_eq!(fs::read_to_string(file).unwrap(), source);
    assert_eq!(
        page(fixture.path(), &[], json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        20
    );
}

// @mara checks DES-ITEM-LIST
#[test]
fn exact_filters_compose_and_reject_undeclared_names() {
    let fixture = retrieval_fixture();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap();
    // Minimal template has no inverse by default; add a project-owned alias.
    let schema = schema.replace(
        "  derives_from:\n",
        "  derives_from:\n    inverse: source_of\n",
    );
    fs::write(&schema_path, schema).unwrap();
    let value = page(
        fixture.path(),
        &[
            "--flavour",
            "requirement",
            "--field",
            "status=draft",
            "--field",
            "status=accepted",
            "--relation",
            "source_of",
        ],
        json!({"flavours":["requirement"],"fields":[{"key":"status","value":"draft"},{"key":"status","value":"accepted"}],"relations":["source_of"]}),
    );
    assert_eq!(ids(&value), ["REQ-ALPHA", "REQ-BETA"]);
    assert!(
        ids(&page(
            fixture.path(),
            &["--field", "status= draft"],
            json!({"fields":[{"key":"status","value":" draft"}]})
        ))
        .is_empty()
    );
    for (args, params, message) in [
        (
            vec!["--flavour", "missing"],
            json!({"flavours":["missing"]}),
            "unknown flavour",
        ),
        (
            vec!["--field", "title=x"],
            json!({"fields":[{"key":"title","value":"x"}]}),
            "unknown field",
        ),
        (
            vec!["--relation", "missing"],
            json!({"relations":["missing"]}),
            "unknown relation",
        ),
        (vec!["--limit", "0"], json!({"limit":0}), "1 through 100"),
        (
            vec!["--limit", "101"],
            json!({"limit":101}),
            "1 through 100",
        ),
        (
            vec!["--cursor", "malformed"],
            json!({"cursor":"malformed"}),
            "restart",
        ),
    ] {
        rejects(fixture.path(), &args, params, message);
    }
}

// @mara checks DES-ITEM-LIST
#[test]
fn pages_bound_escaped_unicode_titles_and_never_skip_large_items() {
    let fixture = retrieval_fixture();
    let title = "\"界\\".repeat(100);
    let expected = (0..65)
        .map(|i| format!("REQ-{i}-{}", "A".repeat(1200)))
        .collect::<Vec<_>>();
    let source = expected
        .iter()
        .map(|id| format!(":::mara requirement {id}\n:title: {title}\n\nBody.\n:::\n"))
        .collect::<String>();
    fs::write(fixture.path().join("docs/large.mara.md"), source).unwrap();
    let mut cursor: Option<String> = None;
    let mut found = vec![];
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages <= 4);
        let mut args = vec!["--path", "docs/large.mara.md", "--limit", "100"];
        let mut params = json!({"paths":["docs/large.mara.md"],"limit":100});
        if let Some(c) = &cursor {
            args.extend(["--cursor", c]);
            params["cursor"] = json!(c);
        }
        let value = page(fixture.path(), &args, params);
        for item in value["items"].as_array().unwrap() {
            assert_eq!(item["title"].as_str().unwrap().chars().count(), 256);
            assert_eq!(item["title_truncated"], true);
        }
        found.extend(ids(&value));
        cursor = value["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(pages > 1);
    assert_eq!(found, expected);
    let human = mara(
        fixture.path(),
        &[
            "item",
            "list",
            "--path",
            "docs/large.mara.md",
            "--limit",
            "1",
        ],
    );
    assert!(stdout(&human).contains("[title truncated]"));
}

// @mara checks DES-ITEM-LIST
#[test]
fn cursors_bind_source_schema_and_request_and_reject_invalid_positions() {
    let fixture = retrieval_fixture();
    let root = fixture.path();
    let first = page(root, &["--limit", "1"], json!({"limit":1}));
    let cursor = first["next_cursor"].as_str().unwrap();
    for (args, params) in [
        (
            vec!["--limit", "2", "--cursor", cursor],
            json!({"limit":2,"cursor":cursor}),
        ),
        (
            vec![
                "--limit",
                "1",
                "--flavour",
                "requirement",
                "--cursor",
                cursor,
            ],
            json!({"limit":1,"flavours":["requirement"],"cursor":cursor}),
        ),
    ] {
        rejects(root, &args, params, "restart");
    }
    for (file, from, to) in [
        ("docs/a.mara.md", "Base workflow.", "Changed narrative."),
        (
            ".mara/schema.yaml",
            "[draft, accepted]",
            "[draft, accepted, reviewed]",
        ),
    ] {
        let path = root.join(file);
        let source = fs::read_to_string(&path).unwrap();
        fs::write(&path, source.replace(from, to)).unwrap();
        rejects(
            root,
            &["--limit", "1", "--cursor", cursor],
            json!({"limit":1,"cursor":cursor}),
            "restart",
        );
        fs::write(path, source).unwrap();
        page(
            root,
            &["--limit", "1", "--cursor", cursor],
            json!({"limit":1,"cursor":cursor}),
        );
    }
    for position in [0, 999] {
        let invalid = format!("{}-{position:016x}", cursor.rsplit_once('-').unwrap().0);
        rejects(
            root,
            &["--limit", "1", "--cursor", &invalid],
            json!({"limit":1,"cursor":invalid}),
            "invalid continuation position",
        );
    }
}

fn enable_rust(root: &Path) {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join(".mara/code");
    fs::create_dir(root.join(".mara/code")).unwrap();
    for name in ["rust.wasm", "rust.scm"] {
        fs::copy(assets.join(name), root.join(".mara/code").join(name)).unwrap();
    }
    let path = root.join(".mara/project.toml");
    let mut config =
        fs::read_to_string(&path)
            .unwrap()
            .replacen("format_version = 1", "format_version = 3", 1);
    config.push_str("\n[[code.languages]]\nname = \"rust\"\nextensions = [\"rs\"]\ngrammar = \".mara/code/rust.wasm\"\nquery = \".mara/code/rust.scm\"\nseparator = \"::\"\n");
    fs::write(path, config).unwrap();
}

// @mara checks DES-ITEM-LIST
#[test]
fn code_adapter_and_file_only_bytes_invalidate_list_cursors() {
    let fixture = retrieval_fixture();
    let root = fixture.path();
    enable_rust(root);
    let schema = root.join(".mara/schema.yaml");
    let mut text = fs::read_to_string(&schema).unwrap();
    text.push_str("\n  code_implements:\n    description: Code implements a requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n");
    fs::write(schema, text).unwrap();
    let doc = root.join("docs/b.mara.md");
    let text = fs::read_to_string(&doc).unwrap();
    fs::write(
        &doc,
        text.replace(
            ":title: Beta requirement",
            ":title: Beta requirement\n:implemented_by_code: code:data.bin",
        ),
    )
    .unwrap();
    fs::write(root.join("data.bin"), [0xff, 0]).unwrap();
    fs::write(root.join("source.rs"), "fn unmarked() {}\n").unwrap();
    let first = page(root, &["--limit", "1"], json!({"limit":1}));
    let cursor = first["next_cursor"].as_str().unwrap();
    for (file, extra) in [
        ("source.rs", b"\n// changed\n".as_slice()),
        (".mara/code/rust.scm", b"\n; changed\n"),
        (".mara/project.toml", b"\n# changed\n"),
        ("data.bin", b"\xff"),
    ] {
        let path = root.join(file);
        let original = fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed.extend(extra);
        fs::write(&path, changed).unwrap();
        rejects(
            root,
            &["--limit", "1", "--cursor", cursor],
            json!({"limit":1,"cursor":cursor}),
            "restart",
        );
        fs::write(path, original).unwrap();
    }
    fs::remove_file(root.join("data.bin")).unwrap();
    rejects(
        root,
        &["--limit", "1", "--cursor", cursor],
        json!({"limit":1,"cursor":cursor}),
        "restart",
    );
    // A missing explicit endpoint is not semantic validation during a fresh list.
    page(root, &[], json!({}));
    fs::remove_file(root.join(".mara/code/rust.wasm")).unwrap();
    rejects(root, &[], json!({}), "rust.wasm");
    assert_eq!(
        fs::read_to_string(root.join("source.rs")).unwrap(),
        "fn unmarked() {}\n"
    );
}

// @mara checks REQ-ITEM-LIST
#[test]
fn fresh_listing_rejects_malformed_documents_and_marker_syntax_without_backfill() {
    let fixture = retrieval_fixture();
    let root = fixture.path();
    enable_rust(root);
    let doc = root.join("docs/a.mara.md");
    let original = fs::read(&doc).unwrap();
    fs::write(root.join("source.rs"), "// @mara implements\nfn run() {}\n").unwrap();
    rejects(root, &[], json!({}), "invalid code marker");
    fs::write(root.join("source.rs"), "fn run() {}\n").unwrap();
    let invalid = ":::mara requirement REQ-INCOMPLETE\n:title: Missing close\n";
    fs::write(&doc, invalid).unwrap();
    rejects(root, &[], json!({}), "closing delimiter");
    assert_eq!(fs::read_to_string(&doc).unwrap(), invalid);
    fs::write(&doc, &original).unwrap();
    page(root, &[], json!({}));
    assert_eq!(fs::read(&doc).unwrap(), original);
}

// @mara checks DES-ITEM-LIST
#[cfg(unix)]
#[test]
fn file_only_snapshot_never_follows_an_outside_symlink() {
    let fixture = retrieval_fixture();
    let root = fixture.path();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("data.bin");
    fs::write(&target, b"outside original").unwrap();
    std::os::unix::fs::symlink(&target, root.join("data.bin")).unwrap();
    let schema = root.join(".mara/schema.yaml");
    let mut s = fs::read_to_string(&schema).unwrap();
    s.push_str("\n  code_implements:\n    description: Code implements a requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n");
    fs::write(schema, s).unwrap();
    let doc = root.join("docs/b.mara.md");
    let s = fs::read_to_string(&doc).unwrap();
    fs::write(
        &doc,
        s.replace(
            ":title: Beta requirement",
            ":title: Beta requirement\n:implemented_by_code: code:data.bin",
        ),
    )
    .unwrap();
    let first = page(root, &["--limit", "1"], json!({"limit":1}));
    let c = first["next_cursor"].as_str().unwrap();
    let second = page(
        root,
        &["--limit", "1", "--cursor", c],
        json!({"limit":1,"cursor":c}),
    );
    fs::write(&target, b"outside changed").unwrap();
    assert_eq!(
        page(
            root,
            &["--limit", "1", "--cursor", c],
            json!({"limit":1,"cursor":c})
        ),
        second
    );
}
