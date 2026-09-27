use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;

fn inspect(
    root: &Path,
    source: &str,
    relation: &str,
    target: &str,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Value {
    let mut args = vec![
        "--format", "json", "relation", "get", source, relation, target,
    ];
    let limit_text = limit.map(|v| v.to_string());
    let mut params = json!({"source":source,"relation":relation,"target":target});
    if let Some(limit) = &limit_text {
        args.extend(["--limit", limit]);
        params["limit"] = json!(limit.parse::<usize>().unwrap());
    }
    if let Some(cursor) = cursor {
        args.extend(["--cursor", cursor]);
        params["cursor"] = json!(cursor);
    }
    let output = mara(root, &args);
    let result: Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{}", stderr(&output)));
    assert_eq!(
        output.status.success(),
        result.get("error").is_none(),
        "{result}"
    );
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "relation_get", params),
        ],
    );
    let response = &mcp_response(&replies, 2)["result"];
    assert_eq!(response["structuredContent"], result);
    assert_eq!(response["isError"], result.get("error").is_some());
    assert_eq!(result["format_version"], 1);
    assert!(serde_json::to_vec(&result).unwrap().len() <= 65536);
    result
}

// @mara implements VER-RELATION-INSPECTION
// @mara checks REQ-RELATION-INSPECTION
// @mara checks DES-RELATION-INTERFACES
#[test]
fn code_and_item_occurrences_are_globally_ordered_before_pagination() {
    let fixture = code_fixture("rust", "rs", "::");
    let root = fixture.path();
    let schema_path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap()
        + "\n  code_check:\n    description: Code checks requirement\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: checked_by_code\n";
    fs::write(schema_path, schema).unwrap();
    let item = ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:checked_by_code: code:aaa.rs::run\n\nAlso [[checked_by_code:code:aaa.rs::run]].\n:::\n";
    let code = "// @mara code_check REQ-A\nfn run() {}\n";
    fs::write(root.join("zzz.mara.md"), item).unwrap();
    fs::write(root.join("aaa.rs"), code).unwrap();
    let mut cursor = None::<String>;
    let mut occurrences = Vec::new();
    loop {
        let page = inspect(
            root,
            "code:aaa.rs::run",
            "code_check",
            "REQ-A",
            Some(1),
            cursor.as_deref(),
        );
        assert_eq!(page["occurrence_count"], 3);
        occurrences.extend(page["occurrences"].as_array().unwrap().iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        assert!(occurrences.len() < 4);
    }
    let locations = occurrences
        .iter()
        .map(|o| {
            (
                o["source"]["path"].as_str().unwrap(),
                o["source"]["start_byte"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut expected = locations.clone();
    expected.sort();
    assert_eq!(locations, expected);
    assert_eq!(occurrences[0]["kind"], "code_comment");
    assert_eq!(occurrences[1]["kind"], "metadata");
    assert_eq!(occurrences[2]["kind"], "inline");
    let alias = inspect(
        root,
        "REQ-A",
        "checked_by_code",
        "code:aaa.rs::run",
        None,
        None,
    );
    assert_eq!(alias["occurrences"], json!(occurrences));
    assert_eq!(fs::read_to_string(root.join("zzz.mara.md")).unwrap(), item);
    assert_eq!(fs::read_to_string(root.join("aaa.rs")).unwrap(), code);
}

fn code_fixture(language: &str, extension: &str, separator: &str) -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = if matches!(language, "python" | "typescript") {
        manifest.join("tests/fixtures/code")
    } else {
        manifest.join(".mara/code")
    };
    fs::create_dir(fixture.path().join(".mara/code")).unwrap();
    for suffix in ["wasm", "scm"] {
        let name = format!("{language}.{suffix}");
        fs::copy(
            assets.join(&name),
            fixture.path().join(".mara/code").join(name),
        )
        .unwrap();
    }
    let config = fixture.path().join(".mara/project.toml");
    let mut source = fs::read_to_string(&config).unwrap().replacen(
        "format_version = 1",
        "format_version = 3",
        1,
    );
    source.push_str(&format!("\n[[code.languages]]\nname = {language:?}\nextensions = [{extension:?}]\ngrammar = \".mara/code/{language}.wasm\"\nquery = \".mara/code/{language}.scm\"\nseparator = {separator:?}\n"));
    fs::write(config, source).unwrap();
    fixture
}

fn relation_fixture() -> TempDir {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    let path = root.join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap()
        + "\n  follows:\n    description: Dependency\n    source: [requirement]\n    target: [requirement]\n    inverse: followed_by\n  associated_with:\n    description: Association\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n  tracked_by:\n    description: External ticket\n    source: [requirement]\n    target: []\n    external: true\n";
    fs::write(path, schema).unwrap();
    fixture
}

fn check_source_spans(root: &Path, entries: &[Value]) {
    let mut selectors = BTreeSet::new();
    for entry in entries {
        assert!(selectors.insert(entry["reference"].as_str().unwrap()));
        let location = &entry["source"];
        let source = fs::read_to_string(root.join(location["path"].as_str().unwrap())).unwrap();
        let actual = &source[location["start_byte"].as_u64().unwrap() as usize
            ..location["end_byte"].as_u64().unwrap() as usize];
        let relation = entry["relation"].as_str().unwrap();
        let target = entry["target"].as_str().unwrap();
        let expected = match entry["kind"].as_str().unwrap() {
            "metadata" => format!(":{relation}: {target}"),
            "inline" => format!("[[{relation}:{target}]]"),
            other => panic!("{other}"),
        };
        assert_eq!(actual, expected);
    }
}

// @mara checks REQ-RELATION-INSPECTION
// @mara checks DES-RELATION-INTERFACES
#[test]
fn canonical_alias_symmetric_and_external_inspection_preserves_authored_spans() {
    let fixture = relation_fixture();
    let root = fixture.path();
    let first = ":::mara requirement REQ-A\r\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\r\n:title: A\r\n:follows: REQ-B\r\n:associated_with: REQ-B\r\n\r\nUnicode 🦀 [[follows:01ARZ3NDEKTSV4RRFFQ69G5F01]].\r\n:::\r\n";
    let second = ":::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n:followed_by: REQ-A\n:associated_with: REQ-A\n\nB.\n:::\n";
    fs::write(root.join("a.mara.md"), first).unwrap();
    fs::write(root.join("b.mara.md"), second).unwrap();
    let canonical = inspect(root, "REQ-A", "follows", "REQ-B", None, None);
    let alias = inspect(root, "REQ-B", "followed_by", "REQ-A", None, None);
    let mids = inspect(
        root,
        "01ARZ3NDEKTSV4RRFFQ69G5F00",
        "follows",
        "01ARZ3NDEKTSV4RRFFQ69G5F01",
        None,
        None,
    );
    assert_eq!(canonical, alias);
    assert_eq!(canonical, mids);
    assert_eq!(canonical["occurrence_count"], 3);
    check_source_spans(root, canonical["occurrences"].as_array().unwrap());
    let left = inspect(root, "REQ-A", "associated_with", "REQ-B", None, None);
    let right = inspect(root, "REQ-B", "associated_with", "REQ-A", None, None);
    assert_eq!(left, right);
    assert_eq!(left["edge"]["symmetric"], true);
    assert_eq!(left["occurrence_count"], 2);
    assert_eq!(fs::read_to_string(root.join("a.mara.md")).unwrap(), first);
    assert_eq!(fs::read_to_string(root.join("b.mara.md")).unwrap(), second);
    let url = "https://Example.invalid/Work/ABC-1?x=%5B#Part";
    let target = format!("external:{url}");
    let external = first.replace(
        "Unicode 🦀",
        &format!("[[tracked_by:{target}]] and [[tracked_by:{target}]] Unicode 🦀"),
    );
    fs::write(root.join("a.mara.md"), &external).unwrap();
    let page = inspect(root, "REQ-A", "tracked_by", &target, None, None);
    assert_eq!(
        page["edge"]["target"],
        json!({"kind":"external","address":url})
    );
    assert_eq!(page["occurrence_count"], 2);
    check_source_spans(root, page["occurrences"].as_array().unwrap());
    assert_eq!(
        fs::read_to_string(root.join("a.mara.md")).unwrap(),
        external
    );
}

// @mara checks REQ-RELATION-INSPECTION
// @mara checks DES-RELATION-INTERFACES
#[test]
fn occurrence_pages_preserve_total_count_and_reject_stale_or_changed_requests() {
    let fixture = relation_fixture();
    let root = fixture.path();
    let path = root.join("self.mara.md");
    let source = format!(
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n{}\n{}:::\n",
        ":followed_by: REQ-A\n".repeat(20),
        "Self [[followed_by:REQ-A]].\n".repeat(5)
    );
    fs::write(&path, &source).unwrap();
    let first = inspect(root, "REQ-A", "follows", "REQ-A", None, None);
    assert_eq!(first["occurrence_count"], 25);
    assert_eq!(first["occurrences"].as_array().unwrap().len(), 20);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = inspect(root, "REQ-A", "follows", "REQ-A", None, Some(cursor));
    assert_eq!(second["occurrence_count"], 25);
    assert_eq!(second["has_more"], false);
    let mut entries = first["occurrences"].as_array().unwrap().clone();
    entries.extend(second["occurrences"].as_array().unwrap().iter().cloned());
    assert_eq!(entries.len(), 25);
    check_source_spans(root, &entries);
    for (relation, limit, token) in [
        ("followed_by", None, cursor),
        ("follows", Some(1), cursor),
        ("follows", None, ""),
        ("follows", None, "malformed"),
    ] {
        let invalid = inspect(root, "REQ-A", relation, "REQ-A", limit, Some(token));
        assert!(invalid.get("error").is_some());
    }
    for position in ["0000000000000000", "ffffffffffffffff"] {
        let token = format!("{}{position}", &cursor[..19]);
        let invalid = inspect(root, "REQ-A", "follows", "REQ-A", None, Some(&token));
        assert_eq!(invalid["error"]["code"], "invalid_cursor");
    }
    for changed_path in [&path, &root.join(".mara/schema.yaml")] {
        let before = fs::read_to_string(changed_path).unwrap();
        let changed = if changed_path.extension().is_some_and(|e| e == "yaml") {
            before.replace("description: Dependency", "description: Updated dependency")
        } else {
            format!("{before}\n")
        };
        fs::write(changed_path, changed).unwrap();
        let stale = inspect(root, "REQ-A", "follows", "REQ-A", None, Some(cursor));
        assert!(stale.get("error").is_some());
        let fresh = inspect(root, "REQ-A", "follows", "REQ-A", None, None);
        assert_ne!(
            fresh["occurrences"][0]["reference"],
            first["occurrences"][0]["reference"]
        );
        fs::write(changed_path, before).unwrap();
    }
    assert_eq!(
        inspect(root, "REQ-A", "follows", "REQ-A", None, Some(cursor)),
        second
    );
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

// @mara checks REQ-RELATION-INSPECTION
// @mara checks DES-RELATION-INTERFACES
#[test]
fn inspection_byte_budget_pages_every_occurrence_and_fails_without_skipping() {
    let fixture = relation_fixture();
    let root = fixture.path();
    let path = root.join("item.mara.md");
    let target = format!("external:https://example.invalid/{}", "abc".repeat(1000));
    let source = format!(
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n{}\nBody.\n:::\n",
        format!(":tracked_by: {target}\n").repeat(35)
    );
    fs::write(&path, &source).unwrap();
    let mut cursor = None::<String>;
    let mut all = Vec::new();
    let mut pages = 0;
    loop {
        let page = inspect(
            root,
            "REQ-A",
            "tracked_by",
            &target,
            Some(100),
            cursor.as_deref(),
        );
        assert_eq!(page["occurrence_count"], 35);
        let entries = page["occurrences"].as_array().unwrap();
        assert!(!entries.is_empty());
        assert!(entries.len() < 35);
        all.extend(entries.iter().cloned());
        pages += 1;
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        assert!(pages < 10);
    }
    assert!(pages > 1);
    assert_eq!(all.len(), 35);
    check_source_spans(root, &all);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    let huge = format!("external:https://example.invalid/{}", "a".repeat(66000));
    fs::write(&path, source.replace(&target, &huge)).unwrap();
    let failure = inspect(root, "REQ-A", "tracked_by", &huge, None, None);
    assert_eq!(failure["error"]["code"], "page_limit");
    assert!(serde_json::to_vec(&failure).unwrap().len() < 1024);
}

// @mara checks REQ-RELATION-INSPECTION
// @mara checks DES-RELATION-INTERFACES
#[test]
fn inspection_reports_missing_edges_invalid_endpoints_and_bad_limits() {
    let fixture = relation_fixture();
    let root = fixture.path();
    fs::write(
        root.join("a.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n\nBody.\n:::\n",
    )
    .unwrap();
    let missing = inspect(root, "REQ-A", "follows", "REQ-A", None, None);
    assert_eq!(missing["error"]["code"], "relation_not_found");
    assert_eq!(missing["occurrence_count"], 0);
    assert_eq!(missing["edge"]["source"], missing["edge"]["target"]);
    for (source, relation, target, expected) in [
        ("REQ-MISSING", "follows", "REQ-A", "invalid_endpoint"),
        ("REQ-A", "unknown", "REQ-A", "invalid_relation"),
        ("REQ-A", "follows", "REQ-MISSING", "invalid_endpoint"),
        (
            "REQ-A",
            "tracked_by",
            "external:not-a-url",
            "invalid_endpoint",
        ),
    ] {
        assert_eq!(
            inspect(root, source, relation, target, None, None)["error"]["code"],
            expected
        );
    }
    for limit in [0, 101] {
        assert!(
            inspect(root, "REQ-A", "follows", "REQ-A", Some(limit), None)
                .get("error")
                .is_some()
        );
    }
    let replies = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "relation_get",
                json!({"source":"REQ-A","relation":"follows","target":"REQ-A","unexpected":true}),
            ),
        ],
    );
    let rejected = mcp_response(&replies, 2);
    assert!(rejected.get("error").is_some() || rejected["result"]["isError"] == true);
}
