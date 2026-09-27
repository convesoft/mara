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
fn add_fixture_mids(root: &Path) {
    for entry in ignore::Walk::new(root) {
        let entry = entry.unwrap();
        if !entry.file_type().is_some_and(|kind| kind.is_file())
            || !entry.path().to_string_lossy().ends_with(".mara.md")
        {
            continue;
        }
        let source = fs::read_to_string(entry.path()).unwrap();
        let mut result = String::new();
        for line in source.split_inclusive('\n') {
            result.push_str(line);
            if line.starts_with(":::mara ") {
                result.push_str(&format!(":mid: {}\n", ulid::Ulid::new()));
            }
        }
        fs::write(entry.path(), result).unwrap();
    }
}
fn related_cli_mcp(root: &Path, reference: &str, filters: &[(&str, &str)]) -> Value {
    let mut args = vec!["--format", "json", "related", reference];
    let mut params = json!({"reference":reference});
    for (flag, value) in filters {
        args.extend([*flag, *value]);
        match *flag {
            "--limit" => params["limit"] = json!(value.parse::<usize>().unwrap()),
            "--relation" => {
                if params.get("relations").is_none() {
                    params["relations"] = json!([]);
                }
                params["relations"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(value));
            }
            "--flavour" => params["flavours"] = json!([value]),
            flag => params[flag.trim_start_matches("--")] = json!(value),
        }
    }
    let output = mara(root, &args);
    assert!(output.status.success(), "{}", stdout(&output));
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(serde_json::to_vec(&page).unwrap().len() <= 65_536);
    assert_eq!(page["format_version"], 2);
    let responses = mcp_exchange(
        root,
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "related", params),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        page
    );
    page
}

// @mara checks REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
// @mara implements VER-DOCUMENT-NAVIGATION
fn unified_related_navigates_mentions_relations_and_structure_with_exact_evidence() {
    let fixture = retrieval_fixture();
    let source = "# Portal\r\n\r\nStart here [[REQ-HUB]] and [[REQ-HUB]].\r\n\r\n:::mara requirement REQ-HUB\r\n:title: Hub\r\n:depends_on: REQ-TARGET\r\n:depends_on: REQ-HUB\r\n\r\n## Inner\r\n\r\nSee [[REQ-TARGET]].\r\n:::\r\n\r\n:::mara requirement REQ-TARGET\r\n:title: Target\r\n\r\nTarget body.\r\n:::\r\n\r\n[Inner](#inner) and [document](a.mara.md).\r\n";
    fs::write(fixture.path().join("docs/portal.mara.md"), source).unwrap();
    add_fixture_mids(fixture.path());
    let output = mara(
        fixture.path(),
        &["--format", "json", "search", "Start here"],
    );
    let search: Value = serde_json::from_slice(&output.stdout).unwrap();
    let hit = &search["results"][0]["node"];
    assert_eq!(hit["kind"], "block");
    let reference = hit["reference"].as_str().unwrap();
    let mut cursor = None::<String>;
    let mut connections = Vec::new();
    loop {
        let mut filters = vec![("--limit", "1")];
        if let Some(cursor) = &cursor {
            filters.push(("--cursor", cursor));
        }
        let page = related_cli_mcp(fixture.path(), reference, &filters);
        assert_eq!(page["node"], *hit);
        let entries = page["connections"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        connections.extend(entries.iter().cloned());
        assert!(
            connections.len() <= 3,
            "direct navigation must not expand another hop"
        );
        cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(connections.len(), 3);
    for edge in &connections[..2] {
        assert_eq!(edge["relation"], "mentions");
        assert_eq!(edge["direction"], "outgoing");
        assert_eq!(edge["neighbour"]["id"], "REQ-HUB");
        let source = fs::read_to_string(
            fixture
                .path()
                .join(edge["source"]["path"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            &source[edge["source"]["start_byte"].as_u64().unwrap() as usize
                ..edge["source"]["end_byte"].as_u64().unwrap() as usize],
            "[[REQ-HUB]]"
        );
    }
    assert_ne!(connections[0]["source"], connections[1]["source"]);
    assert_eq!(connections[2]["relation"], "contains");
    assert_eq!(connections[2]["direction"], "incoming");
    let hub = connections[0]["neighbour"]["reference"].as_str().unwrap();
    let hub_page = related_cli_mcp(fixture.path(), hub, &[]);
    let edges = hub_page["connections"].as_array().unwrap();
    for edge in &connections[..2] {
        assert!(edges.iter().any(|back| back["direction"] == "incoming"
            && back["relation"] == "mentions"
            && back["neighbour"]["reference"] == reference
            && back["source"] == edge["source"]));
    }
    // Separate typed and mention connections; a directed self-edge appears once.
    let target_edges = edges
        .iter()
        .filter(|edge| edge["neighbour"]["id"] == "REQ-TARGET")
        .collect::<Vec<_>>();
    assert_eq!(target_edges.len(), 2);
    assert_eq!(
        target_edges
            .iter()
            .map(|edge| edge["relation"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["depends_on", "mentions"])
    );
    assert!(edges.iter().any(|edge| edge["neighbour"]["id"] == "REQ-HUB"
        && edge["direction"] == "outgoing"
        && edge["relation"] == "depends_on"));
    assert!(
        !edges.iter().any(|edge| edge["neighbour"]["id"] == "REQ-HUB"
            && edge["direction"] == "incoming"
            && edge["relation"] == "depends_on")
    );
    let target = target_edges[0]["neighbour"]["reference"].as_str().unwrap();
    let target_page = related_cli_mcp(
        fixture.path(),
        target,
        &[("--direction", "incoming"), ("--flavour", "requirement")],
    );
    assert_eq!(target_page["connections"].as_array().unwrap().len(), 2);
    // Parent -> children is a second explicit call, exposing sibling context and reusable handles.
    let section = connections[2]["neighbour"]["reference"].as_str().unwrap();
    let section_page = related_cli_mcp(
        fixture.path(),
        section,
        &[("--direction", "outgoing"), ("--relation", "contains")],
    );
    assert_eq!(section_page["node"]["kind"], "section");
    assert!(
        section_page["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["neighbour"]["reference"] == reference)
    );
    let document = section_page["node"]["context"]["parent"].as_str().unwrap();
    let document_page = related_cli_mcp(fixture.path(), document, &[]);
    assert_eq!(document_page["node"]["kind"], "document");
    let read = mara(fixture.path(), &["--format", "json", "get", target]);
    let read: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert!(read["content"].as_str().unwrap().contains("Target body."));
    // Links can target a section within an item, without losing that precise destination.
    let inner = edges
        .iter()
        .find(|edge| edge["relation"] == "contains")
        .unwrap()["neighbour"]
        .clone();
    let inner_page = related_cli_mcp(
        fixture.path(),
        inner["reference"].as_str().unwrap(),
        &[("--relation", "mentions"), ("--direction", "incoming")],
    );
    assert_eq!(inner_page["node"]["kind"], "section");
    assert_eq!(inner_page["connections"].as_array().unwrap().len(), 1);
    let filtered = related_cli_mcp(fixture.path(), reference, &[("--flavour", "requirement")]);
    assert_eq!(filtered["connections"].as_array().unwrap().len(), 2);
    let invalidated =
        fs::read_to_string(fixture.path().join("docs/portal.mara.md")).unwrap() + "\nChanged.\n";
    fs::write(fixture.path().join("docs/portal.mara.md"), invalidated).unwrap();
    let stale = mara(fixture.path(), &["related", reference]);
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("rediscover"));
}

// @mara checks REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn unified_related_qualifies_collisions_against_vocabulary_and_rejects_old_interface() {
    let fixture = retrieval_fixture();
    add_fixture_mids(fixture.path());
    let path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{schema}\n  contains:\n    description: Authored containment\n    source: [requirement]\n    target: [scenario]\n  mentions:\n    description: Authored mention\n    source: [requirement]\n    target: [scenario]\n")).unwrap();
    let path = fixture.path().join("docs/a.mara.md");
    let source = fs::read_to_string(&path).unwrap().replace(
        ":derives_from: SCN-BASE",
        ":contains: SCN-BASE\n:mentions: SCN-BASE",
    );
    fs::write(path, source).unwrap();
    let page = related_cli_mcp(fixture.path(), "REQ-ALPHA", &[]);
    let relations = page["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["relation"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(relations.contains("schema:contains"));
    assert!(relations.contains("schema:mentions"));
    assert!(relations.contains("builtin:contains"));
    let typed = related_cli_mcp(
        fixture.path(),
        "REQ-ALPHA",
        &[("--relation", "schema:contains")],
    );
    assert_eq!(typed["connections"].as_array().unwrap().len(), 1);
    let structural = related_cli_mcp(
        fixture.path(),
        "REQ-ALPHA",
        &[("--relation", "builtin:contains")],
    );
    assert_eq!(structural["connections"].as_array().unwrap().len(), 2);
    let human = mara(
        fixture.path(),
        &[
            "related",
            "REQ-ALPHA",
            "--relation",
            "builtin:contains",
            "--direction",
            "incoming",
        ],
    );
    assert!(stdout(&human).contains("incoming\tbuiltin:contained_by\t"));
    // Even a node without either edge must reject ambiguous shorthand.
    for name in ["contains", "mentions"] {
        let output = mara(fixture.path(), &["related", "REQ-ZULU", "--relation", name]);
        assert!(!output.status.success());
        let error = stderr(&output);
        assert!(
            error.contains(&format!("schema:{name}")) && error.contains(&format!("builtin:{name}"))
        );
    }
    for args in [
        vec!["item", "related", "REQ-ALPHA"],
        vec!["related", "REQ-ALPHA", "--hops", "2"],
        vec!["related", "REQ-ALPHA", "--relation", "builtin:missing"],
        vec!["related", "REQ-ALPHA", "--flavour", "missing"],
    ] {
        assert!(!mara(fixture.path(), &args).status.success());
    }
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_related", json!({"id":"REQ-ALPHA"})),
            mcp_call(3, "related", json!({"id":"REQ-ALPHA"})),
            mcp_call(4, "related", json!({"reference":"REQ-ALPHA", "hops":2})),
            mcp_call(
                5,
                "related",
                json!({"reference":"REQ-ALPHA", "relations":["contains"]}),
            ),
        ],
    );
    for id in 2..=5 {
        let response = mcp_response(&responses, id);
        assert!(
            response.get("error").is_some() || response["result"]["isError"] == true,
            "{response}"
        );
    }
}

// @mara checks REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn unified_related_rejects_oversized_requested_node_even_without_connections() {
    let fixture = retrieval_fixture();
    let reference = format!("REQ-{}", "A".repeat(66_000));
    fs::write(
        fixture.path().join("docs/huge.mara.md"),
        format!(":::mara requirement {reference}\n:title: Huge\n\nBody.\n:::\n"),
    )
    .unwrap();
    let output = mara(
        fixture.path(),
        &["related", &reference, "--relation", "depends_on"],
    );
    assert!(!output.status.success());
    assert!(stderr(&output).contains("65536-byte"));
    assert!(output.stderr.len() < 1024);
}

// @mara checks REQ-RELATED-PAGINATION
// @mara checks REQ-ITEM-RELATED
#[test]
fn related_pages_continue_in_order_with_filters_and_cli_mcp_parity() {
    let fixture = retrieval_fixture();
    let mut source = String::from(":::mara requirement REQ-HUB\n:title: Hub\n");
    // Deliberately reverse outgoing order; each neighbour also links back twice.
    for i in (0..43).rev() {
        source.push_str(&format!(":depends_on: REQ-NEIGHBOUR-{i}\n"));
    }
    source.push_str("\nHub body.\n:::\n\n");
    for i in 0..43 {
        source.push_str(&format!(":::mara requirement REQ-NEIGHBOUR-{i}\n:title: Neighbour {i}\n:depends_on: REQ-HUB\n:supersedes: REQ-HUB\n\nNeighbour body.\n:::\n\n"));
    }
    fs::write(fixture.path().join("docs/neighbours.mara.md"), source).unwrap();
    add_fixture_mids(fixture.path());
    for filtered in [false, true] {
        let mut cursor: Option<String> = None;
        let mut actual = Vec::new();
        let mut requests = vec![
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        ];
        let mut pages = Vec::new();
        loop {
            assert!(pages.len() < 30, "continuation must make progress");
            let mut args = vec![
                "--format",
                "json",
                "related",
                "REQ-HUB",
                "--flavour",
                "requirement",
                "--limit",
                "7",
            ];
            let mut params = json!({"reference":"REQ-HUB", "flavours":["requirement"], "limit":7});
            if filtered {
                args.extend(["--direction", "incoming", "--relation", "supersedes"]);
                params["direction"] = json!("incoming");
                params["relations"] = json!(["supersedes"]);
                params["flavours"] = json!(["requirement"]);
            }
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
                params["cursor"] = json!(cursor);
            }
            let output = mara(fixture.path(), &args);
            assert!(output.status.success(), "{}", stdout(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            let items = page["connections"].as_array().unwrap();
            assert!(!items.is_empty() && items.len() <= 7);
            for entry in items {
                assert!(
                    entry["neighbour"]["mid"]
                        .as_str()
                        .unwrap()
                        .parse::<ulid::Ulid>()
                        .is_ok()
                );
                assert!(entry["neighbour"].get("body").is_none());
                assert!(entry["neighbour"].get("excerpts").is_none());
                actual.push((
                    entry["direction"].as_str().unwrap().to_owned(),
                    entry["relation"].as_str().unwrap().to_owned(),
                    entry["neighbour"]["id"].as_str().unwrap().to_owned(),
                ));
            }
            requests.push(mcp_call(pages.len() as u64 + 2, "related", params));
            assert_eq!(page["has_more"], !page["next_cursor"].is_null());
            cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
            pages.push(page);
            if cursor.is_none() {
                break;
            }
        }
        let responses = mcp_exchange(fixture.path(), &requests);
        for (i, page) in pages.iter().enumerate() {
            assert_eq!(
                &mcp_response(&responses, i as u64 + 2)["result"]["structuredContent"],
                page
            );
        }
        let mut expected = Vec::new();
        if !filtered {
            for i in 0..43 {
                expected.push((
                    "outgoing".to_owned(),
                    "depends_on".to_owned(),
                    format!("REQ-NEIGHBOUR-{i}"),
                ));
            }
        }
        for i in 0..43 {
            for relation in if filtered {
                vec!["supersedes"]
            } else {
                vec!["depends_on", "supersedes"]
            } {
                expected.push((
                    "incoming".to_owned(),
                    relation.to_owned(),
                    format!("REQ-NEIGHBOUR-{i}"),
                ));
            }
        }
        assert_eq!(actual, expected);
    }
    let output = mara(fixture.path(), &["--format", "json", "related", "REQ-HUB"]);
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page["connections"].as_array().unwrap().len(), 20);
    assert_eq!(page["has_more"], true);
}

// @mara checks REQ-RELATED-PAGINATION
// @mara checks REQ-ITEM-RELATED
#[test]
fn related_pages_reject_changed_inputs_and_invalid_continuation() {
    let fixture = retrieval_fixture();
    add_fixture_mids(fixture.path());
    let first = mara(
        fixture.path(),
        &["--format", "json", "related", "REQ-ALPHA", "--limit", "1"],
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    for args in [
        vec!["related", "REQ-ALPHA", "--limit", "2", "--cursor", cursor],
        vec!["related", "SCN-BASE", "--limit", "1", "--cursor", cursor],
        vec![
            "related",
            "REQ-ALPHA",
            "--limit",
            "1",
            "--direction",
            "incoming",
            "--cursor",
            cursor,
        ],
        vec![
            "related",
            "REQ-ALPHA",
            "--limit",
            "1",
            "--relation",
            "satisfies",
            "--cursor",
            cursor,
        ],
        vec![
            "related",
            "REQ-ALPHA",
            "--limit",
            "1",
            "--flavour",
            "design",
            "--cursor",
            cursor,
        ],
        vec!["item", "list", "--limit", "1", "--cursor", cursor],
        vec!["related", "REQ-ALPHA", "--cursor", "malformed"],
    ] {
        let output = mara(fixture.path(), &args);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"), "{}", stderr(&output));
    }
    for position in ["0000000000000000", "ffffffffffffffff"] {
        let invalid_cursor = format!("{}{position}", &cursor[..19]);
        let output = mara(
            fixture.path(),
            &[
                "related",
                "REQ-ALPHA",
                "--limit",
                "1",
                "--cursor",
                &invalid_cursor,
            ],
        );
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"));
    }
    for limit in ["0", "101"] {
        let output = mara(fixture.path(), &["related", "REQ-ALPHA", "--limit", limit]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("1 through 100"));
    }
    for relative in ["docs/a.mara.md", ".mara/schema.yaml"] {
        let path = fixture.path().join(relative);
        let original = fs::read_to_string(&path).unwrap();
        let changed = if relative.ends_with("yaml") {
            original.replace("[draft, accepted]", "[draft, accepted, reviewed]")
        } else {
            format!("Narrative edit.\n{original}")
        };
        fs::write(&path, changed).unwrap();
        let output = mara(
            fixture.path(),
            &["related", "REQ-ALPHA", "--limit", "1", "--cursor", cursor],
        );
        assert!(!output.status.success());
        assert!(stderr(&output).contains("restart"));
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "related",
                    json!({"reference":"REQ-ALPHA","limit":1,"cursor":cursor}),
                ),
                mcp_call(
                    3,
                    "related",
                    json!({"reference":"REQ-ALPHA","cursor":"malformed"}),
                ),
                mcp_call(4, "related", json!({"reference":"REQ-ALPHA","limit":101})),
            ],
        );
        for id in [2, 3, 4] {
            assert_eq!(mcp_response(&responses, id)["result"]["isError"], true);
        }
        fs::write(path, original).unwrap();
        let restored = mara(
            fixture.path(),
            &[
                "--format",
                "json",
                "related",
                "REQ-ALPHA",
                "--limit",
                "1",
                "--cursor",
                cursor,
            ],
        );
        assert!(restored.status.success(), "{}", stdout(&restored));
        let page: Value = serde_json::from_slice(&restored.stdout).unwrap();
        assert!(!page["connections"].as_array().unwrap().is_empty());
    }
}

// @mara checks REQ-RELATED-PAGINATION
// @mara checks REQ-ITEM-RELATED
#[test]
fn related_pages_bound_escaped_unicode_titles_and_preserve_every_entry() {
    let fixture = retrieval_fixture();
    let title = "界\"\\".repeat(200);
    let source = (0..100).map(|i| format!(":::mara design DES-BUDGET-{i}\n:title: {title}\n:satisfies: REQ-ALPHA\n\nBody.\n:::\n\n")).collect::<String>();
    fs::write(fixture.path().join("docs/budget.mara.md"), source).unwrap();
    add_fixture_mids(fixture.path());
    let mut cursor: Option<String> = None;
    let mut ids = Vec::new();
    let mut pages = 0;
    loop {
        assert!(pages < 10);
        let mut args = vec![
            "--format",
            "json",
            "related",
            "REQ-ALPHA",
            "--relation",
            "satisfies",
            "--direction",
            "incoming",
            "--limit",
            "100",
        ];
        let mut params = json!({"reference":"REQ-ALPHA", "relations":["satisfies"], "direction":"incoming", "limit":100});
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor]);
            params["cursor"] = json!(cursor);
        }
        let output = mara(fixture.path(), &args);
        assert!(output.status.success(), "{}", stdout(&output));
        assert!(output.stdout.len() - 1 <= 65_536);
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "related", params),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            page
        );
        if pages == 0 {
            assert!(page["connections"].as_array().unwrap().len() < 100);
        }
        for entry in page["connections"].as_array().unwrap() {
            let item = &entry["neighbour"];
            if item["id"] != "DES-ALPHA" {
                assert_eq!(item["title_truncated"], true);
                assert_eq!(item["title"], title.chars().take(256).collect::<String>());
            }
            ids.push(item["id"].as_str().unwrap().to_owned());
        }
        pages += 1;
        cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    let mut expected = vec!["DES-ALPHA".to_owned()];
    expected.extend((0..100).map(|i| format!("DES-BUDGET-{i}")));
    assert_eq!(ids, expected);
    let human = mara(
        fixture.path(),
        &["related", "REQ-ALPHA", "--direction", "incoming"],
    );
    assert!(stdout(&human).contains(" [title truncated]"));
    assert!(stdout(&human).contains("page\thas_more=true\tnext_cursor="));
    let full = mara(fixture.path(), &["--format", "json", "get", "DES-BUDGET-0"]);
    let full: Value = serde_json::from_slice(&full.stdout).unwrap();
    assert_eq!(full["metadata"][1]["value"], title);
}

// @mara checks REQ-RELATED-PAGINATION
// @mara checks REQ-ITEM-RELATED
#[test]
fn related_pages_fail_on_an_oversized_entry_without_skipping_it() {
    let fixture = retrieval_fixture();
    let long_id = format!("DES-{}", "A".repeat(66_000));
    fs::write(
        fixture.path().join("docs/oversized.mara.md"),
        format!(
            ":::mara design {long_id}\n:title: Huge identity\n:satisfies: REQ-ALPHA\n\nBody.\n:::\n"
        ),
    )
    .unwrap();
    add_fixture_mids(fixture.path());
    let args = [
        "--format",
        "json",
        "related",
        "REQ-ALPHA",
        "--relation",
        "satisfies",
        "--direction",
        "incoming",
    ];
    let output = mara(fixture.path(), &args);
    assert!(output.status.success(), "{}", stdout(&output));
    let first: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(first["connections"].as_array().unwrap().len(), 1);
    assert_eq!(first["connections"][0]["neighbour"]["id"], "DES-ALPHA");
    let cursor = first["next_cursor"].as_str().unwrap();
    let output = mara(
        fixture.path(),
        &[
            "related",
            "REQ-ALPHA",
            "--relation",
            "satisfies",
            "--direction",
            "incoming",
            "--cursor",
            cursor,
        ],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("shorten oversized identity/location fields"));
    assert!(output.stderr.len() < 1024);
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "related",
                json!({"reference":"REQ-ALPHA", "relations":["satisfies"], "direction":"incoming", "cursor":cursor}),
            ),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert!(serde_json::to_vec(result).unwrap().len() < 1024);
}

// @mara checks REQ-RELATED-PAGINATION
// @mara checks REQ-ITEM-RELATED
#[test]
fn item_related_returns_filtered_direct_neighbours_with_relation_and_direction() {
    let fixture = retrieval_fixture();
    add_fixture_mids(fixture.path());

    let related = mara(fixture.path(), &["related", "REQ-ALPHA"]);
    assert!(related.status.success(), "{}", stderr(&related));
    let human = stdout(&related);
    assert!(human.contains("derives_from → SCN-BASE\tBase scenario"));
    assert!(human.contains("incoming\tcontained_by\t"));
    assert!(human.contains("occurrences=1"));

    let incoming = mara(
        fixture.path(),
        &[
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
    assert!(stdout(&incoming).starts_with(
        "incoming satisfies → DES-ALPHA\tAlpha design\tdocs/b.mara.md:10\toccurrences=1\treference="
    ));
    assert!(stdout(&incoming).ends_with("page\thas_more=false\n"));

    let no_match = mara(
        fixture.path(),
        &[
            "related",
            "REQ-ALPHA",
            "--direction",
            "outgoing",
            "--flavour",
            "design",
        ],
    );
    assert!(no_match.status.success(), "{}", stderr(&no_match));
    assert_eq!(stdout(&no_match), "page\thas_more=false\n");
}

fn extend_schema(root: &Path, extra: &str) {
    let path = root.join(".mara/schema.yaml");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(path, format!("{source}\n{extra}")).unwrap();
}

// @mara checks DES-CANONICAL-TRACE-RELATIONS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn canonical_alias_symmetric_and_self_edges_deduplicate_before_paging() {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    extend_schema(
        root,
        "  follows:\n    description: Directed dependency\n    source: [requirement]\n    target: [requirement]\n    inverse: followed_by\n  associated_with:\n    description: Association\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n",
    );
    let source = ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:follows: REQ-B\n:associated_with: REQ-B\n\nAlso [[follows:REQ-B]].\n:::\n\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n:followed_by: REQ-A\n:associated_with: REQ-A\n\nB.\n:::\n";
    let path = root.join("items.mara.md");
    fs::write(&path, source).unwrap();
    let outgoing = related_cli_mcp(
        root,
        "REQ-A",
        &[("--relation", "follows"), ("--direction", "outgoing")],
    );
    let incoming = related_cli_mcp(
        root,
        "REQ-B",
        &[("--relation", "followed_by"), ("--direction", "incoming")],
    );
    assert_eq!(outgoing["connections"].as_array().unwrap().len(), 1);
    assert_eq!(
        incoming["connections"][0]["edge"],
        outgoing["connections"][0]["edge"]
    );
    assert_eq!(incoming["connections"][0]["label"], "followed_by");
    assert_eq!(incoming["connections"][0]["occurrence_count"], 3);
    let human = mara(
        root,
        &[
            "related",
            "REQ-B",
            "--relation",
            "followed_by",
            "--direction",
            "incoming",
        ],
    );
    assert!(stdout(&human).starts_with("followed_by → REQ-A"));
    let mut edge = None;
    for id in ["REQ-A", "REQ-B"] {
        let page = related_cli_mcp(root, id, &[("--direction", "symmetric")]);
        assert_eq!(page["connections"].as_array().unwrap().len(), 1);
        assert_eq!(page["connections"][0]["occurrence_count"], 2);
        if let Some(expected) = &edge {
            assert_eq!(&page["connections"][0]["edge"], expected);
        }
        edge = Some(page["connections"][0]["edge"].clone());
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    fs::write(&path,":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:follows: REQ-A\n:followed_by: REQ-A\n:associated_with: REQ-A\n\nSelf [[followed_by:REQ-A]].\n:::\n").unwrap();
    for (direction, expected) in [
        (None, vec!["outgoing", "symmetric"]),
        (Some("outgoing"), vec!["outgoing"]),
        (Some("incoming"), vec!["incoming"]),
        (Some("symmetric"), vec!["symmetric"]),
    ] {
        let mut cursor = None::<String>;
        let mut actual = Vec::new();
        loop {
            let mut filters = vec![
                ("--relation", "follows"),
                ("--relation", "associated_with"),
                ("--limit", "1"),
            ];
            if let Some(direction) = direction {
                filters.push(("--direction", direction));
            }
            if let Some(cursor) = &cursor {
                filters.push(("--cursor", cursor));
            }
            let page = related_cli_mcp(root, "REQ-A", &filters);
            actual.extend(
                page["connections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c["direction"].as_str().unwrap().to_owned()),
            );
            cursor = page["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
            assert!(actual.len() < 3);
        }
        assert_eq!(actual, expected);
    }
}

// @mara checks DES-CANONICAL-TRACE-RELATIONS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn external_neighbours_are_exact_terminal_endpoints() {
    let fixture = support::fixture();
    let root = fixture.path();
    mara::initialize_project(root, mara::Template::Minimal).unwrap();
    extend_schema(
        root,
        "  tracked_by:\n    description: Local external reference\n    source: [requirement]\n    target: []\n    external: true\n",
    );
    let address = "https://Example.invalid/ENG-7?view=full#notes";
    let source = format!(
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:tracked_by: external:{address}\n\nAlso [[tracked_by:external:{address}]].\n:::\n"
    );
    let path = root.join("item.mara.md");
    fs::write(&path, &source).unwrap();
    let page = related_cli_mcp(root, "REQ-A", &[("--relation", "tracked_by")]);
    assert_eq!(page["connections"].as_array().unwrap().len(), 1);
    assert_eq!(
        page["connections"][0]["neighbour"],
        json!({"kind":"external","address":address})
    );
    assert_eq!(page["connections"][0]["occurrence_count"], 2);
    assert_eq!(
        page["connections"][0]["edge"]["target"],
        page["connections"][0]["neighbour"]
    );
    assert!(page["connections"][0].get("source").is_none());
    let filtered = related_cli_mcp(
        root,
        "REQ-A",
        &[("--relation", "tracked_by"), ("--flavour", "requirement")],
    );
    assert_eq!(filtered["connections"], json!([]));
    for operation in ["get", "related"] {
        assert!(
            !mara(root, &[operation, &format!("external:{address}")])
                .status
                .success()
        );
    }
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

fn code_fixture(language: &str, extension: &str) -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    code_index::configure(fixture.path(), language, &[extension], true);
    fixture
}

// @mara checks DES-CANONICAL-TRACE-RELATIONS
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn code_markers_and_inverse_assertions_share_edges_and_binary_endpoints() {
    let fixture = code_fixture("rust", "rs");
    let root = fixture.path();
    extend_schema(
        root,
        "  code_implements:\n    description: Code implements requirement\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n",
    );
    let source = ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:implemented_by_code: code:run.rs::rust::run().\n:implemented_by_code: code:blob.bin\n\nA.\n:::\n";
    fs::write(root.join("item.mara.md"), source).unwrap();
    let code = "// @mara code_implements REQ-A\nfn run() {}\n";
    fs::write(root.join("run.rs"), code).unwrap();
    code_index::write_single(root, "rust", "run.rs", "run", "run().");
    fs::write(root.join("blob.bin"), [0xff, 0xfe]).unwrap();
    let incoming = related_cli_mcp(
        root,
        "REQ-A",
        &[("--relation", "implemented_by_code"), ("--limit", "1")],
    );
    assert_eq!(
        incoming["connections"][0]["neighbour"]["reference"],
        "code:blob.bin"
    );
    let cursor = incoming["next_cursor"].as_str().unwrap();
    let next = related_cli_mcp(
        root,
        "REQ-A",
        &[
            ("--relation", "implemented_by_code"),
            ("--limit", "1"),
            ("--cursor", cursor),
        ],
    );
    assert_eq!(next["connections"][0]["occurrence_count"], 2);
    let outgoing = related_cli_mcp(root, "code:run.rs::rust::run().", &[]);
    assert_eq!(
        outgoing["connections"][0]["edge"],
        next["connections"][0]["edge"]
    );
    let binary = related_cli_mcp(root, "code:blob.bin", &[]);
    assert_eq!(binary["connections"][0]["neighbour"]["id"], "REQ-A");
    assert!(!mara(root, &["get", "code:blob.bin"]).status.success());
    fs::write(root.join("blob.bin"), [0xfe, 0xff]).unwrap();
    let stale = mara(
        root,
        &[
            "related",
            "REQ-A",
            "--relation",
            "implemented_by_code",
            "--limit",
            "1",
            "--cursor",
            cursor,
        ],
    );
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("restart"));
    for (marker, error) in [
        ("code_implements REQ-MISSING", "missing item"),
        ("unknown REQ-A", "unknown relation"),
        ("implemented_by_code REQ-A", "does not allow a code source"),
    ] {
        fs::write(
            root.join("run.rs"),
            format!("// @mara {marker}\nfn run() {{}}\n"),
        )
        .unwrap();
        code_index::write_single(root, "rust", "run.rs", "run", "run().");
        let failure = mara(root, &["related", "code:run.rs::rust::run()."]);
        assert!(!failure.status.success());
        assert!(stderr(&failure).contains(error), "{}", stderr(&failure));
    }
    fs::write(root.join("run.rs"), code).unwrap();
    code_index::write_single(root, "rust", "run.rs", "run", "run().");
    assert_eq!(
        fs::read_to_string(root.join("item.mara.md")).unwrap(),
        source
    );
    fs::remove_file(root.join("blob.bin")).unwrap();
    let missing = mara(root, &["related", "REQ-A", "--relation", "code_implements"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("unavailable code target"));
    let excluded = related_cli_mcp(
        root,
        "REQ-A",
        &[
            ("--relation", "code_implements"),
            ("--flavour", "requirement"),
        ],
    );
    assert_eq!(excluded["connections"], json!([]));
}

// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn selected_missing_and_ambiguous_item_targets_fail_without_affecting_other_relations() {
    let fixture = retrieval_fixture();
    let root = fixture.path();
    let path = root.join("docs/missing.mara.md");
    fs::write(
        &path,
        ":::mara requirement REQ-SOURCE\n:title: Source\n:depends_on: REQ-TARGET\n\nSource.\n:::\n",
    )
    .unwrap();
    add_fixture_mids(root);
    for (target, error) in [
        ("", "missing item"),
        (
            ":::mara requirement REQ-TARGET\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: One\n\nOne.\n:::\n\n:::mara requirement REQ-TARGET\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: Two\n\nTwo.\n:::\n",
            "ambiguous item",
        ),
    ] {
        fs::write(root.join("docs/targets.mara.md"), target).unwrap();
        let result = mara(root, &["related", "REQ-SOURCE", "--relation", "depends_on"]);
        assert!(!result.status.success());
        assert!(stderr(&result).contains(error));
        let replies = mcp_exchange(
            root,
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "related",
                    json!({"reference":"REQ-SOURCE","relations":["depends_on"]}),
                ),
            ],
        );
        assert_eq!(mcp_response(&replies, 2)["result"]["isError"], true);
        let excluded = related_cli_mcp(root, "REQ-SOURCE", &[("--relation", "supersedes")]);
        assert_eq!(excluded["connections"], json!([]));
    }
}

// @mara implements VER-DOCUMENT-NAVIGATION
// @mara checks DES-DIRECT-NAVIGATION
#[test]
fn related_orders_incoming_code_before_symmetric_connections() {
    let fixture = fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap()
        + "\n  code_implements:\n    description: Code implements the requirement.\n    source: []\n    target: [requirement]\n    code_source: true\n    inverse: implemented_by_code\n  peer:\n    description: Requirements are peers.\n    source: [requirement]\n    target: [requirement]\n    symmetric: true\n";
    fs::write(&schema_path, schema).unwrap();
    fs::write(fixture.path().join("a.rs"), "fn run() {}\n").unwrap();
    fs::write(
        fixture.path().join("requirements.mara.md"),
        ":::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: A\n:implemented_by_code: code:a.rs\n:peer: REQ-B\n\nA.\n:::\n\n:::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01\n:title: B\n\nB.\n:::\n\n:::mara requirement REQ-C\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02\n:title: C\n:depends_on: REQ-A\n\nC.\n:::\n",
    )
    .unwrap();

    let result = related_cli_mcp(fixture.path(), "REQ-A", &[]);
    assert_eq!(result["has_more"], false);
    let connections = result["connections"].as_array().unwrap();
    let code = connections
        .iter()
        .position(|c| c["neighbour"]["reference"] == "code:a.rs")
        .unwrap();
    let incoming = connections
        .iter()
        .position(|c| c["direction"] == "incoming" && c["neighbour"]["id"] == "REQ-C")
        .unwrap();
    let symmetric = connections
        .iter()
        .position(|c| c["direction"] == "symmetric")
        .unwrap();
    assert_eq!(connections[code]["direction"], "incoming");
    assert!(code < incoming && incoming < symmetric);
}
