use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
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

fn get_pages_with_cli_mcp_parity(root: &Path, id: &str) -> Vec<Value> {
    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        assert!(pages.len() < 60, "get continuation must finish");
        let mut args = vec!["--format", "json", "get", id];
        let mut params = json!({"reference": id});
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor]);
            params["cursor"] = json!(cursor);
        }
        let output = mara(root, &args);
        assert!(output.status.success(), "{}", stdout(&output));
        assert!(output.stdout.len() - 1 <= 65_536);
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        let responses = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "get", params),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"]["structuredContent"];
        assert_eq!(result, &page);
        assert!(serde_json::to_vec(result).unwrap().len() <= 65_536);
        assert_eq!(page["has_more"], !page["next_cursor"].is_null());
        let next = page["next_cursor"].as_str().map(ToOwned::to_owned);
        if next.is_some() {
            assert_ne!(cursor, next);
        }
        cursor = next;
        pages.push(page);
        if cursor.is_none() {
            break;
        }
    }
    pages
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn get_returns_item_content_and_authored_metadata_without_neighbours() {
    let fixture = retrieval_fixture();

    let get = mara(fixture.path(), &["get", "REQ-ALPHA"]);

    assert!(get.status.success(), "{}", stderr(&get));
    let text = stdout(&get);
    assert!(text.contains("REQ-ALPHA\tItem\tAlpha requirement"));
    assert!(text.contains("source\tdocs/a.mara.md\tstart_byte=69\tend_byte=202"));
    assert!(text.contains("content\nNeed searchable Zebra knowledge.\n"));
    assert!(text.contains("derives_from\tSCN-BASE"));
    assert!(text.contains("id\tREQ-ALPHA"));
    assert!(text.contains("flavour\trequirement"));
    assert!(!text.contains("incoming\t"));
    assert!(!text.contains("outgoing\t"));
    assert!(text.contains("page\thas_more=false"));

    let missing = mara(fixture.path(), &["get", "REQ-MISSING"]);
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
    let ambiguous = mara(fixture.path(), &["get", "REQ-ALPHA"]);
    assert!(!ambiguous.status.success());
    assert!(stderr(&ambiguous).contains("item ID 'REQ-ALPHA' is ambiguous"));
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
// @mara implements VER-BOUNDED-NODE-READ
fn get_fragments_reconstruct_unicode_content_and_metadata_via_cli_mcp() {
    let fixture = retrieval_fixture();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap();
    fs::write(schema_path, schema.replace("        values: [draft, accepted]", "        values: [draft, accepted]\n      tag:\n        type: string\n        repeatable: true")).unwrap();
    let title = "界\"\\🦀".repeat(16_000);
    let tag = "cafe\u{301}\"\\🦀".repeat(9_000);
    // One enormous line plus its structural final newline, with JSON escaping
    // and combining sequences. Expected bytes come directly from authored text.
    let body = format!(
        "Start {} intervening text {} end.\n",
        "界\"\\🦀".repeat(20_000),
        "e\u{301}".repeat(25_000)
    );
    let mut source = format!(
        ":::mara requirement REQ-FRAGMENTS\n:title: {title}\n:tag: {tag}\n:tag: \n:tag: last\n"
    );
    source.push_str(":depends_on: REQ-FRAGMENTS\n");
    for i in (0..43).rev() {
        source.push_str(&format!(":depends_on: REQ-PART-{i}\n"));
    }
    source.push_str(&format!("\n{body}:::\n\n"));
    for i in 0..43 {
        source.push_str(&format!(":::mara requirement REQ-PART-{i}\n:title: Part {i}\n:depends_on: REQ-FRAGMENTS\n:supersedes: REQ-FRAGMENTS\n\nPart.\n:::\n\n"));
    }
    let path = fixture.path().join("docs/fragments.mara.md");
    fs::write(&path, &source).unwrap();
    let pages = get_pages_with_cli_mcp_parity(fixture.path(), "REQ-FRAGMENTS");
    assert!(pages.len() > 5);
    assert_eq!(pages[0]["content_range"]["partial"], true);
    assert_eq!(pages[0]["node"]["title_truncated"], true);
    let mut actual_body = String::new();
    let mut metadata = vec![String::new(); 48];
    let mut keys = vec![String::new(); 48];
    let mut fragmented_metadata = BTreeSet::new();
    for page in &pages {
        let text = page["content"].as_str().unwrap();
        let range = &page["content_range"];
        assert_eq!(range["start_byte"], actual_body.len());
        actual_body.push_str(text);
        assert_eq!(range["end_byte"], actual_body.len());
        assert_eq!(range["total_bytes"], body.len());
        assert_eq!(range["partial"], text != body);
        for entry in page["metadata"].as_array().unwrap() {
            let index = entry["index"].as_u64().unwrap() as usize;
            let key = entry["key"].as_str().unwrap();
            if !keys[index].is_empty() {
                assert_eq!(keys[index], key);
            }
            keys[index] = key.to_owned();
            assert_eq!(entry["range"]["start_byte"], metadata[index].len());
            metadata[index].push_str(entry["value"].as_str().unwrap());
            assert_eq!(entry["range"]["end_byte"], metadata[index].len());
            if entry["range"]["partial"] == true {
                fragmented_metadata.insert(index);
            }
            assert!(
                entry["range"]["end_byte"].as_u64().unwrap()
                    <= entry["range"]["total_bytes"].as_u64().unwrap()
            );
        }
        assert!(page.get("outgoing_relations").is_none());
        assert!(page.get("incoming_relations").is_none());
    }
    assert_eq!(actual_body, body);
    let mut expected_metadata = vec![
        title,
        tag,
        String::new(),
        "last".to_owned(),
        "REQ-FRAGMENTS".to_owned(),
    ];
    expected_metadata.extend((0..43).rev().map(|i| format!("REQ-PART-{i}")));
    assert_eq!(metadata, expected_metadata);
    assert_eq!(keys[..4], ["title", "tag", "tag", "tag"]);
    assert!(keys[4..].iter().all(|key| key == "depends_on"));
    assert!(fragmented_metadata.contains(&0) && fragmented_metadata.contains(&1));
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    let human = mara(fixture.path(), &["get", "REQ-FRAGMENTS"]);
    assert!(stdout(&human).contains("[title truncated]"));
    assert!(stdout(&human).contains("partial=true"));
    assert!(stdout(&human).contains("page\thas_more=true\tnext_cursor="));
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn get_returns_complete_fitting_content_before_paging_metadata() {
    let fixture = retrieval_fixture();
    let body = format!("{}\n", "body 🦀\"\\ ".repeat(3_000));
    let mut source = format!(
        ":::mara requirement REQ-COMPLETE\n:title: {}\n",
        "title".repeat(20_000)
    );
    for _ in 0..23 {
        source.push_str(":depends_on: REQ-ALPHA\n");
    }
    source.push_str(&format!("\n{body}:::\n"));
    fs::write(fixture.path().join("docs/complete.mara.md"), source).unwrap();
    let pages = get_pages_with_cli_mcp_parity(fixture.path(), "REQ-COMPLETE");
    assert_eq!(pages[0]["content"], body);
    assert_eq!(pages[0]["content_range"]["partial"], false);
    assert_eq!(pages[0]["metadata_range"]["partial"], true);
    assert!(
        pages
            .iter()
            .all(|page| page.get("outgoing_relations").is_none())
    );
    let small = get_pages_with_cli_mcp_parity(fixture.path(), "REQ-BETA");
    assert_eq!(small.len(), 1);
    assert_eq!(small[0]["content"], "Second requirement body.\n");
    for name in ["content_range", "metadata_range"] {
        assert_eq!(small[0][name]["partial"], false);
    }
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn get_rejects_changed_inputs_and_invalid_fragment_cursors() {
    let fixture = retrieval_fixture();
    fs::write(
        fixture.path().join("docs/long.mara.md"),
        format!(
            ":::mara requirement REQ-LONG\n:title: Long\n\n{}\n:::\n",
            "🦀".repeat(30_000)
        ),
    )
    .unwrap();
    let first = mara(fixture.path(), &["--format", "json", "get", "REQ-LONG"]);
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    for (id, token) in [("REQ-ALPHA", cursor), ("REQ-LONG", "malformed")] {
        let output = mara(fixture.path(), &["get", id, "--cursor", token]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"));
    }
    for token in [
        format!(
            "{}0000000000000001-0000000000000000-0000000000000000",
            &cursor[..20]
        ),
        format!(
            "{}0000000000000000-0000000000000000-0000000000000000",
            &cursor[..20]
        ),
        format!(
            "{}ffffffffffffffff-ffffffffffffffff-ffffffffffffffff",
            &cursor[..20]
        ),
    ] {
        let output = mara(fixture.path(), &["get", "REQ-LONG", "--cursor", &token]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"));
    }
    let cross_operation = mara(fixture.path(), &["item", "list", "--cursor", cursor]);
    assert!(!cross_operation.status.success());
    for limit in ["0", "101"] {
        let output = mara(fixture.path(), &["get", "REQ-LONG", "--limit", limit]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("unexpected argument"));
    }
    for relative in ["docs/long.mara.md", "docs/a.mara.md", ".mara/schema.yaml"] {
        let path = fixture.path().join(relative);
        let original = fs::read_to_string(&path).unwrap();
        let changed = if relative.ends_with("yaml") {
            original.replace("[draft, accepted]", "[draft, accepted, reviewed]")
        } else {
            format!("Narrative edit.\n{original}")
        };
        fs::write(&path, changed).unwrap();
        let output = mara(fixture.path(), &["get", "REQ-LONG", "--cursor", cursor]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"));
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "get", json!({"reference":"REQ-LONG", "cursor":cursor})),
                mcp_call(3, "get", json!({"reference":"REQ-LONG", "limit":101})),
            ],
        );
        for id in [2, 3] {
            assert_eq!(mcp_response(&responses, id)["result"]["isError"], true);
        }
        fs::write(&path, original).unwrap();
        let restored = mara(fixture.path(), &["get", "REQ-LONG", "--cursor", cursor]);
        assert!(restored.status.success(), "{}", stderr(&restored));
    }
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn get_ignores_neighbours_and_fails_on_unpageable_identity() {
    let fixture = retrieval_fixture();
    let title = "🦀\"\\".repeat(300);
    let source = (0..100).map(|i| format!(":::mara design DES-GET-{i}\n:title: {title}\n:satisfies: REQ-ALPHA\n\nBody.\n:::\n\n")).collect::<String>();
    fs::write(fixture.path().join("docs/relations.mara.md"), source).unwrap();
    let pages = get_pages_with_cli_mcp_parity(fixture.path(), "REQ-ALPHA");
    assert_eq!(pages.len(), 1);
    assert!(pages[0].get("incoming_relations").is_none());
    let long_id = format!("DES-{}", "A".repeat(66_000));
    fs::write(fixture.path().join("docs/oversized.mara.md"), format!(":::mara design {long_id}\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Oversized\n:satisfies: REQ-ALPHA\n\nBody.\n:::\n")).unwrap();
    let output = mara(fixture.path(), &["get", "01ARZ3NDEKTSV4RRFFQ69G5F01"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("shorten oversized identity/location fields"));
    assert!(output.stderr.len() < 1024);
    assert_eq!(
        get_pages_with_cli_mcp_parity(fixture.path(), "REQ-ALPHA").len(),
        1
    );
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn unified_get_reads_search_results_and_parent_documents_with_cli_mcp_parity() {
    let fixture = support::fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let heading = "界🦀".repeat(12_000);
    let paragraph = "Unicode e\u{301} \"quoted\" \\ text 🦀. ".repeat(4_000);
    let narrative = format!("# {heading}\r\n\r\n{paragraph}");
    let item_body = "## Inside\n\nItem body.\n";
    let mixed = format!(
        "# Mixed\n\nBefore.\n\n:::mara requirement REQ-READ\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: Read\n\n{item_body}:::\n\nAfter.\n"
    );
    fs::write(fixture.path().join("narrative.mara.md"), &narrative).unwrap();
    fs::write(fixture.path().join("mixed.mara.md"), &mixed).unwrap();
    let search = mara(fixture.path(), &["--format", "json", "search", ""]);
    assert!(search.status.success(), "{}", stderr(&search));
    let search: Value = serde_json::from_slice(&search.stdout).unwrap();
    assert_eq!(search["has_more"], false);
    let mut references = search["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["node"].clone())
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    while let Some(expected_node) = references.pop() {
        let reference = expected_node["reference"].as_str().unwrap();
        if !seen.insert(reference.to_owned()) {
            continue;
        }
        let pages = get_pages_with_cli_mcp_parity(fixture.path(), reference);
        let node = &pages[0]["node"];
        if !expected_node["kind"].is_null() {
            assert_eq!(node, &expected_node);
        }
        let kind = node["kind"].as_str().unwrap();
        kinds.insert(kind.to_owned());
        let source = if node["source"]["path"] == "narrative.mara.md" {
            &narrative
        } else {
            &mixed
        };
        let expected = if kind == "item" {
            item_body
        } else {
            &source[node["source"]["start_byte"].as_u64().unwrap() as usize
                ..node["source"]["end_byte"].as_u64().unwrap() as usize]
        };
        let mut content = String::new();
        for page in &pages {
            assert_eq!(page["format_version"], 2);
            assert_eq!(&page["node"], node);
            assert_eq!(page["content_range"]["start_byte"], content.len());
            content.push_str(page["content"].as_str().unwrap());
            assert_eq!(page["content_range"]["end_byte"], content.len());
            assert_eq!(page["content_range"]["total_bytes"], expected.len());
            if kind != "item" {
                assert_eq!(page["metadata"], json!([]));
                assert_eq!(
                    page["metadata_range"],
                    json!({"start_index":0,"end_index":0,"total":0,"partial":false})
                );
            }
            assert!(page.get("incoming_relations").is_none());
            assert!(page.get("outgoing_relations").is_none());
        }
        assert_eq!(content, expected);
        if expected.len() > 65_536 {
            assert!(pages.len() > 1);
        }
        if let Some(parent) = node["context"]["parent"].as_str() {
            references.push(json!({"reference": parent}));
        }
    }
    assert_eq!(
        kinds,
        BTreeSet::from([
            "item".into(),
            "section".into(),
            "block".into(),
            "document".into()
        ])
    );
    let by_id = get_pages_with_cli_mcp_parity(fixture.path(), "REQ-READ");
    let by_mid = get_pages_with_cli_mcp_parity(fixture.path(), "01ARZ3NDEKTSV4RRFFQ69G5F01");
    assert_eq!(by_id, by_mid);
}

// @mara checks REQ-PARTIAL-ITEM-READ
// @mara checks REQ-DOCUMENT-CONTEXT-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn unified_get_rejects_stale_handles_cursors_and_removed_interface() {
    let fixture = support::fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let path = fixture.path().join("narrative.mara.md");
    let source = "🦀".repeat(30_000);
    fs::write(&path, &source).unwrap();
    let result = mara(fixture.path(), &["--format", "json", "search", ""]);
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    let reference = result["results"][0]["node"]["reference"].as_str().unwrap();
    let pages = get_pages_with_cli_mcp_parity(fixture.path(), reference);
    let cursor = pages[0]["next_cursor"].as_str().unwrap();
    // Other documents do not invalidate a handle, but do invalidate a cursor.
    fs::write(fixture.path().join("other.mara.md"), "New narrative.").unwrap();
    assert!(mara(fixture.path(), &["get", reference]).status.success());
    assert!(
        !mara(fixture.path(), &["get", reference, "--cursor", cursor])
            .status
            .success()
    );
    fs::write(&path, format!("Changed {source}")).unwrap();
    let stale = mara(fixture.path(), &["get", reference]);
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("search again"));
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_request(2, "tools/list", json!({})),
            mcp_call(3, "get", json!({"reference":reference})),
            mcp_call(4, "get", json!({"reference":reference, "cursor":cursor})),
            mcp_call(5, "get", json!({"reference":reference, "limit":1})),
            mcp_call(6, "get", json!({"id":reference})),
            mcp_call(7, "item_get", json!({"id":reference})),
        ],
    );
    let tools = mcp_response(&responses, 2)["result"]["tools"]
        .as_array()
        .unwrap();
    assert!(!tools.iter().any(|tool| tool["name"] == "item_get"));
    let get = tools.iter().find(|tool| tool["name"] == "get").unwrap();
    let props = &get["inputSchema"]["properties"];
    assert!(props.get("reference").is_some());
    assert!(props.get("id").is_none() && props.get("limit").is_none());
    for id in 3..=7 {
        let response = mcp_response(&responses, id);
        assert!(
            response["result"]["isError"] == true || !response["error"].is_null(),
            "{response}"
        );
    }
    fs::write(&path, &source).unwrap();
    for args in [
        vec!["item", "get", reference],
        vec!["get", reference, "--limit", "1"],
    ] {
        assert!(!mara(fixture.path(), &args).status.success());
    }
}

fn code_fixture(language: &str, extension: &str) -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    code_index::configure(fixture.path(), language, &[extension], true);
    fixture
}

// @mara checks DES-CODE-READ
#[test]
fn get_reads_indexed_symbols_with_modifiers_and_preserves_sources() {
    for (language, ext, source, descriptor, expected) in [
        (
            "rust",
            "rs",
            "mod outer {\n#[test]\nfn run() {}\n}\n",
            "outer/run().",
            "#[test]\nfn run() {}",
        ),
        (
            "python",
            "py",
            "class Outer:\n    @decorator\n    def run(self): pass\n",
            "Outer#run().",
            "@decorator\n    def run(self): pass",
        ),
        (
            "javascript",
            "js",
            "export function run() {}\n",
            "run().",
            "export function run() {}",
        ),
        (
            "typescript",
            "ts",
            "export function run(): void {}\n",
            "run().",
            "export function run(): void {}",
        ),
    ] {
        let fixture = code_fixture(language, ext);
        let file = format!("sample.{ext}");
        fs::write(fixture.path().join(&file), source).unwrap();
        code_index::write_single(fixture.path(), language, &file, "run", descriptor);
        let selector = format!("{language}::{descriptor}");
        let reference = format!("code:{file}::{selector}");
        let pages = get_pages_with_cli_mcp_parity(fixture.path(), &reference);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0]["node"]["kind"], "code");
        assert_eq!(pages[0]["node"]["reference"], reference);
        assert_eq!(pages[0]["node"]["title"], selector);
        assert_eq!(pages[0]["content"], expected);
        assert_eq!(pages[0]["metadata"], json!([]));
        assert_eq!(pages[0]["node"]["context"], json!({}));
        assert!(pages[0]["node"].get("mid").is_none());
        assert_eq!(
            fs::read_to_string(fixture.path().join(&file)).unwrap(),
            source
        );
        let whole = get_pages_with_cli_mcp_parity(fixture.path(), &format!("code:{file}"));
        assert_eq!(whole[0]["content"], source);
    }
}

// @mara checks DES-CODE-READ
// @mara checks DES-BOUNDED-NODE-READ
#[test]
fn get_pages_ignored_file_only_content_and_binds_it_to_cursors() {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    fs::write(fixture.path().join(".gitignore"), "notes.txt\n").unwrap();
    let path = fixture.path().join("notes.txt");
    let source = "Unicode 🦀 e\u{301} \"escaped\" \\ text\r\n".repeat(5000);
    fs::write(&path, &source).unwrap();
    let pages = get_pages_with_cli_mcp_parity(fixture.path(), "code:notes.txt");
    assert!(pages.len() > 1);
    let reconstructed: String = pages
        .iter()
        .map(|p| p["content"].as_str().unwrap())
        .collect();
    assert_eq!(reconstructed, source);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    let cursor = pages[0]["next_cursor"].as_str().unwrap();
    fs::write(&path, format!("Changed {source}")).unwrap();
    let stale = mara(
        fixture.path(),
        &["get", "code:notes.txt", "--cursor", cursor],
    );
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("restart"));
    let replies = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "get",
                json!({"reference":"code:notes.txt","cursor":cursor}),
            ),
        ],
    );
    assert_eq!(mcp_response(&replies, 2)["result"]["isError"], true);
    fs::write(&path, &source).unwrap();
    assert!(
        mara(
            fixture.path(),
            &["get", "code:notes.txt", "--cursor", cursor]
        )
        .status
        .success()
    );
}

// @mara checks DES-CODE-READ
#[test]
fn get_code_rejects_missing_unsupported_and_binary_targets() {
    let fixture = code_fixture("rust", "rs");
    fs::write(
        fixture.path().join("sample.rs"),
        "fn duplicate() {}\nfn duplicate() {}\n",
    )
    .unwrap();
    fs::write(fixture.path().join("notes.txt"), "Text\n").unwrap();
    fs::write(fixture.path().join("binary.bin"), [0xff, 0xfe]).unwrap();
    for (reference, error) in [
        ("code:missing.txt", "MissingFile"),
        ("code:sample.rs::rust::missing().", "MissingSymbol"),
        ("code:notes.txt::thing", "Unsupported"),
        ("code:./notes.txt", "Unsupported"),
        ("code:../notes.txt", "Unsupported"),
        ("code:binary.bin", "not UTF-8"),
    ] {
        let result = mara(fixture.path(), &["get", reference]);
        assert!(!result.status.success(), "{reference}");
        assert!(stderr(&result).contains(error), "{}", stderr(&result));
        let replies = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "get", json!({"reference":reference})),
            ],
        );
        assert_eq!(mcp_response(&replies, 2)["result"]["isError"], true);
        assert!(
            mcp_response(&replies, 2)["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(error)
        );
    }
    #[cfg(unix)]
    {
        let outside = support::fixture();
        fs::write(outside.path().join("secret.txt"), "Outside source").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            fixture.path().join("outside.txt"),
        )
        .unwrap();
        let result = mara(fixture.path(), &["get", "code:outside.txt"]);
        assert!(!result.status.success());
        assert!(stderr(&result).contains("Unsupported"));
        assert!(!stdout(&result).contains("Outside source"));
    }
}
