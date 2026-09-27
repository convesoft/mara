use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
use tempfile::TempDir;
mod support;
use support::*;
fn collection_nodes(page: &Value) -> Vec<Value> {
    if page.get("format_version").is_some() {
        assert_eq!(page["format_version"], 2);
        page["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["node"].clone())
            .collect()
    } else {
        page["items"].as_array().unwrap().clone()
    }
}
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

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn directory_path_filters_compose_with_ranking_and_cli_mcp_pagination() {
    let fixture = directory_retrieval_fixture();
    for operation in ["list", "search"] {
        let mut cursor: Option<String> = None;
        let mut ids = Vec::new();
        loop {
            assert!(ids.len() < 4, "continuation must make progress");
            let mut args = if operation == "search" {
                vec!["--format", "json", operation]
            } else {
                vec!["--format", "json", "item", operation]
            };
            let mut params = json!({
                "paths":["packages/query/docs/", "packages/query/docs/nested"],
                "flavours":["requirement"], "fields":[{"key":"status", "value":"draft"}],
                "relations":["derives_from"], "limit":1,
            });
            if operation == "search" {
                args.extend([
                    "cache",
                    "--id",
                    "REQ-PACK-A",
                    "--id",
                    "REQ-PACK-B",
                    "--id",
                    "REQ-PACK-C",
                    "--id",
                    "REQ-VIEWER",
                ]);
                params["query"] = json!("cache");
                params["ids"] = json!(["REQ-PACK-A", "REQ-PACK-B", "REQ-PACK-C", "REQ-VIEWER"]);
            }
            args.extend([
                "--path",
                "packages/query/docs/",
                "--path",
                "packages/query/docs/nested",
                "--flavour",
                "requirement",
                "--field",
                "status=draft",
                "--relation",
                "derives_from",
                "--limit",
                "1",
            ]);
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
                params["cursor"] = json!(cursor);
            }
            let output = mara(fixture.path(), &args);
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(
                        2,
                        &if operation == "search" {
                            "search".to_owned()
                        } else {
                            format!("item_{operation}")
                        },
                        params.clone(),
                    ),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
            assert!(serde_json::to_vec(&page).unwrap().len() <= 65_536);
            let items = collection_nodes(&page);
            assert_eq!(items.len(), 1);
            ids.push(items[0]["id"].as_str().unwrap().to_owned());
            if operation == "search" {
                assert!(page["results"][0]["excerpt"]["text"].is_string());
            }
            assert_eq!(page["has_more"], !page["next_cursor"].is_null());
            cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
            if let Some(cursor) = &cursor {
                // A different directory must not reuse this request's continuation.
                params["paths"] = json!(["packages/dicom-viewer"]);
                params["cursor"] = json!(cursor);
                let responses = mcp_exchange(
                    fixture.path(),
                    &[
                        mcp_initialize(1),
                        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                        mcp_call(
                            2,
                            &if operation == "search" {
                                "search".to_owned()
                            } else {
                                format!("item_{operation}")
                            },
                            params,
                        ),
                    ],
                );
                assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
            } else {
                break;
            }
        }
        let expected = if operation == "search" {
            ["REQ-PACK-B", "REQ-PACK-A", "REQ-PACK-C"]
        } else {
            ["REQ-PACK-A", "REQ-PACK-B", "REQ-PACK-C"]
        };
        assert_eq!(ids, expected);
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn search_excerpts_preserve_unicode_source_positions_and_exact_selection() {
    let fixture = retrieval_fixture();
    let title = "界".repeat(300);
    let source = format!(
        ":::mara requirement REQ-PASSAGE\n:title: {title}\n:status: draft\n\n{} Straße cafe\u{301} {}\n:::\n",
        "界 ".repeat(2000),
        "tail ".repeat(2000)
    );
    fs::write(fixture.path().join("docs/passage.mara.md"), &source).unwrap();
    add_fixture_mids(fixture.path());
    let source = fs::read_to_string(fixture.path().join("docs/passage.mara.md")).unwrap();
    let mid = source
        .lines()
        .find_map(|line| line.strip_prefix(":mid: "))
        .unwrap();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "search",
            "STRASSE CAFÉ",
            "--id",
            mid,
            "--id",
            "REQ-PASSAGE",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = collection_nodes(&page);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title_truncated"], true);
    assert_eq!(items[0]["title"].as_str().unwrap().chars().count(), 256);
    let excerpts = [page["results"][0]["excerpt"].clone()];
    assert!(!excerpts.is_empty() && excerpts.len() <= 3);
    let mut previous_end = 0;
    for excerpt in &excerpts {
        let start = excerpt["start_byte"].as_u64().unwrap() as usize;
        let end = excerpt["end_byte"].as_u64().unwrap() as usize;
        let text = excerpt["text"].as_str().unwrap();
        assert_eq!(&source[start..end], text);
        assert!(start >= previous_end);
        previous_end = end;
        assert!(text.chars().count() <= 240);
        assert_eq!(excerpt["partial"], true);
        assert_eq!(
            excerpt["start_line"],
            source[..start].bytes().filter(|b| *b == b'\n').count() + 1
        );
        assert_eq!(
            excerpt["end_line"],
            source.as_bytes()[..end - 1]
                .iter()
                .copied()
                .filter(|b| *b == b'\n')
                .count()
                + 1
        );
    }
    assert!(
        excerpts
            .iter()
            .any(|e| e["text"].as_str().unwrap().contains("Straße cafe\u{301}"))
    );
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "search",
                json!({"query":"STRASSE CAFÉ", "ids":[mid,"REQ-PASSAGE"]}),
            ),
            mcp_call(3, "search", json!({"query":"draft", "ids":["REQ-MISSING"]})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        page
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    let excluded = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "search",
            "draft",
            "--id",
            mid,
            "--flavour",
            "scenario",
        ],
    );
    let excluded: Value = serde_json::from_slice(&excluded.stdout).unwrap();
    assert_eq!(collection_nodes(&excluded), Vec::<Value>::new());
    let human = mara(fixture.path(), &["search", "STRASSE CAFÉ", "--id", mid]);
    assert!(human.status.success(), "{}", stderr(&human));
    assert!(stdout(&human).contains("[title truncated]"));
    assert!(stdout(&human).contains("excerpt\tpartial=true\tdocs/passage.mara.md:"));
    assert!(stdout(&human).ends_with("page\thas_more=false\n"));
    let empty = mara(
        fixture.path(),
        &["--format", "json", "search", "", "--id", mid],
    );
    let empty: Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert!(empty["results"][0]["excerpt"]["text"].is_string());
    // Duplicate source IDs make exact selection ambiguous, even with a filter
    // that would otherwise remove every candidate.
    fs::write(fixture.path().join("docs/duplicate.mara.md"), &source).unwrap();
    let ambiguous = mara(
        fixture.path(),
        &[
            "search",
            "draft",
            "--id",
            "REQ-PASSAGE",
            "--flavour",
            "scenario",
        ],
    );
    assert!(!ambiguous.status.success());
    assert!(
        stderr(&ambiguous).contains("ambiguous"),
        "{}",
        stderr(&ambiguous)
    );
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn pagination_rejects_changed_inputs_and_invalid_limits() {
    let fixture = retrieval_fixture();
    let first = mara(
        fixture.path(),
        &["--format", "json", "search", "alpha", "--limit", "1"],
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    for (query, limit) in [("alpha", "2"), ("beta", "1")] {
        let changed = mara(
            fixture.path(),
            &["search", query, "--limit", limit, "--cursor", cursor],
        );
        assert!(!changed.status.success());
        assert!(stderr(&changed).contains("restart"), "{}", stderr(&changed));
    }
    let path = fixture.path().join("docs/a.mara.md");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("Narrative edit.\n{original}")).unwrap();
    let changed = mara(
        fixture.path(),
        &["search", "alpha", "--limit", "1", "--cursor", cursor],
    );
    assert!(!changed.status.success());
    assert!(stderr(&changed).contains("restart"), "{}", stderr(&changed));
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "search",
                json!({"query":"alpha","limit":1,"cursor":cursor}),
            ),
            mcp_call(3, "item_list", json!({"cursor":"malformed"})),
            mcp_call(4, "item_list", json!({"limit":101})),
        ],
    );
    for id in [2, 3, 4] {
        assert_eq!(mcp_response(&responses, id)["result"]["isError"], true);
    }
    fs::write(&path, original).unwrap();
    let restored = mara(
        fixture.path(),
        &["search", "alpha", "--limit", "1", "--cursor", cursor],
    );
    assert!(restored.status.success(), "{}", stderr(&restored));
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let schema = fs::read_to_string(&schema_path).unwrap();
    fs::write(
        &schema_path,
        schema.replace("[draft, accepted]", "[draft, accepted, reviewed]"),
    )
    .unwrap();
    let changed = mara(
        fixture.path(),
        &["search", "alpha", "--limit", "1", "--cursor", cursor],
    );
    assert!(!changed.status.success());
    assert!(stderr(&changed).contains("restart"), "{}", stderr(&changed));
    fs::write(schema_path, schema).unwrap();
    for limit in ["0", "101"] {
        let invalid = mara(fixture.path(), &["item", "list", "--limit", limit]);
        assert!(!invalid.status.success());
        assert!(
            stderr(&invalid).contains("1 through 100"),
            "{}",
            stderr(&invalid)
        );
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn search_pages_obey_serialized_byte_budget_without_losing_large_results() {
    let fixture = retrieval_fixture();
    let title = "界\"\\".repeat(200);
    let passage = format!("needle {} ", "界\"\\ ".repeat(400));
    let source = (0..100)
        .map(|i| {
            format!(
                ":::mara requirement REQ-BUDGET-{i}\n:title: {title}\n\n{}\n:::\n\n",
                passage.repeat(3)
            )
        })
        .collect::<String>();
    fs::write(fixture.path().join("docs/budget.mara.md"), source).unwrap();
    let mut cursor: Option<String> = None;
    let mut ids = Vec::new();
    let mut pages = 0;
    loop {
        let mut args = vec!["--format", "json", "search", "needle", "--limit", "100"];
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor]);
        }
        let output = mara(fixture.path(), &args);
        assert!(output.status.success(), "{}", stderr(&output));
        // Exclude the CLI's framing newline, which is not domain JSON.
        assert!(output.stdout.len() - 1 <= 65_536);
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        if pages == 0 {
            assert_eq!(page["has_more"], true);
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(2, "search", json!({"query":"needle","limit":100})),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
        }
        for item in collection_nodes(&page) {
            assert_eq!(item["title_truncated"], true);
            assert!(item.get("excerpts").is_none());
            ids.push(item["id"].as_str().unwrap().to_owned());
        }
        pages += 1;
        cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(pages > 1);
    assert_eq!(
        ids,
        (0..100)
            .map(|i| format!("REQ-BUDGET-{i}"))
            .collect::<Vec<_>>()
    );
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
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
        "projects validation", // A word-form variation within the edit budget.
    ] {
        let searched = mara(fixture.path(), &["search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert!(
            stdout(&searched)
                .contains("REQ-CROSS-FIELD\tItem\tdocs/search.mara.md:1\tProject knowledge"),
            "query: {query}"
        );
    }

    for query in ["project missing", "prj validation", "project valid"] {
        let searched = mara(fixture.path(), &["search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(
            stdout(&searched),
            "page\thas_more=false\n",
            "query: {query}"
        );
    }

    let unicode_equivalent = mara(fixture.path(), &["search", "CAFE\u{301} WORKFLOW"]);
    assert!(
        unicode_equivalent.status.success(),
        "{}",
        stderr(&unicode_equivalent)
    );
    assert!(
        stdout(&unicode_equivalent)
            .contains("SCN-UNICODE\tItem\tdocs/search.mara.md:7\tCafé workflow")
    );
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn search_relevance_ranks_before_pagination_with_cli_mcp_parity() {
    let fixture = retrieval_fixture();
    let entries = [
        ("REQ-APPROX-BODY", "Catalog", "Project validaton."),
        ("REQ-PROJCET-VALIDATON", "Projcet validaton", "Details."),
        (
            "REQ-BODY",
            "Catalog",
            "Project validation. Project validation.",
        ),
        ("REQ-MIXED", "Project", "Validation."),
        ("REQ-PROJECT-VALIDATION", "Catalog", "Details."),
        ("REQ-TITLE", "Project validation", "Details."),
        (
            "REQ-REPEATED",
            "Project project project",
            "Validation validation.",
        ),
        ("REQ-BOTH", "Projcet validaton", "Project validation."),
    ];
    let mut source = entries.iter().map(|(id, title, body)| {
        format!(":::mara requirement {id}\n:title: {title}\n:status: draft\n:derives_from: SCN-BASE\n\n{body}\n:::\n\n")
    }).collect::<String>();
    source.push_str(":::mara requirement REQ-EXCLUDED\n:title: Project validation\n:status: accepted\n:derives_from: SCN-BASE\n\nDetails.\n:::\n");
    fs::write(fixture.path().join("docs/ranking.mara.md"), source).unwrap();
    add_fixture_mids(fixture.path());
    let expected = [
        "REQ-PROJECT-VALIDATION",
        "REQ-TITLE",
        "REQ-BOTH", // Exact group, weight 6.
        "REQ-MIXED",
        "REQ-REPEATED", // Exact group, weight 4; repetition adds nothing.
        "REQ-BODY",     // Exact body beats even approximate ID/title words.
        "REQ-PROJCET-VALIDATON",
        "REQ-APPROX-BODY", // Approximate group, weights 6 and 2.
    ];
    for (query, select_ids) in [
        ("project validation", false),
        ("VALIDATION project project", true),
    ] {
        let mut cursor: Option<String> = None;
        let mut ids = Vec::new();
        loop {
            let mut args = vec![
                "--format",
                "json",
                "search",
                query,
                "--path",
                "docs/ranking.mara.md",
                "--flavour",
                "requirement",
                "--field",
                "status=draft",
                "--relation",
                "derives_from",
                "--limit",
                "2",
            ];
            let mut params = json!({"query":query, "paths":["docs/ranking.mara.md"],
                "flavours":["requirement"], "fields":[{"key":"status","value":"draft"}],
                "relations":["derives_from"], "limit":2});
            if select_ids {
                // Reversed selected handles must not control ranking.
                for (id, _, _) in entries.iter().rev() {
                    args.extend(["--id", id]);
                }
                params["ids"] = json!(
                    entries
                        .iter()
                        .rev()
                        .map(|entry| entry.0)
                        .collect::<Vec<_>>()
                );
            }
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
                params["cursor"] = json!(cursor);
            }
            let output = mara(fixture.path(), &args);
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            let repeated = mara(fixture.path(), &args);
            assert!(repeated.status.success(), "{}", stderr(&repeated));
            assert_eq!(output.stdout, repeated.stdout);
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(2, "search", params),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
            let items = collection_nodes(&page);
            assert_eq!(items.len(), 2);
            for item in items {
                assert!(item.get("score").is_none());
                ids.push(item["id"].as_str().unwrap().to_owned());
            }
            assert_eq!(ids, expected[..ids.len()]);
            cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
            assert_eq!(page["has_more"], cursor.is_some());
            if cursor.is_none() {
                break;
            }
            assert!(ids.len() < expected.len(), "continuation must finish");
        }
        assert_eq!(ids, expected);
    }
    // Zero-term queries and item list retain source order, including the filtered-out item.
    for operation in ["list", "search"] {
        let mut args = if operation == "search" {
            vec!["--format", "json", operation]
        } else {
            vec!["--format", "json", "item", operation]
        };
        if operation == "search" {
            args.push("...");
        }
        args.extend(["--path", "docs/ranking.mara.md"]);
        let output = mara(fixture.path(), &args);
        assert!(output.status.success(), "{}", stderr(&output));
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        let actual = collection_nodes(&page)
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let mut corpus_order = entries.iter().map(|entry| entry.0).collect::<Vec<_>>();
        corpus_order.push("REQ-EXCLUDED");
        assert_eq!(actual, corpus_order);
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn search_identity_fields_and_their_excerpts_match_exactly() {
    let fixture = retrieval_fixture();
    let path = fixture.path().join("docs/identity.mara.md");
    fs::write(
        &path,
        ":::mara requirement REQ-IDENTITY\n:title: Catalog\n\nDetails.\n:::\n",
    )
    .unwrap();
    add_fixture_mids(fixture.path());
    let original = fs::read_to_string(&path).unwrap();
    let mid = original
        .lines()
        .find_map(|line| line.strip_prefix(":mid: "))
        .unwrap();
    let lower_mid = mid.to_lowercase();
    let mut mistyped_mid = mid.to_owned();
    mistyped_mid.replace_range(25.., if mid.ends_with('0') { "1" } else { "0" });

    for with_title_match in [false, true] {
        let source = if with_title_match {
            original.replace(
                ":title: Catalog",
                &format!(":title: Identity {mistyped_mid}"),
            )
        } else {
            original.clone()
        };
        fs::write(&path, &source).unwrap();
        for (query, exact) in [
            ("identity", true),
            ("identtiy", false),
            (lower_mid.as_str(), true),
            (mistyped_mid.as_str(), false),
        ] {
            let output = mara(
                fixture.path(),
                &[
                    "--format",
                    "json",
                    "search",
                    query,
                    "--path",
                    "docs/identity.mara.md",
                ],
            );
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                collection_nodes(&page).len(),
                usize::from(exact || with_title_match),
                "{query}, title={with_title_match}"
            );
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(
                        2,
                        "search",
                        json!({"query":query, "paths":["docs/identity.mara.md"]}),
                    ),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
            if with_title_match && !exact {
                let excerpts = [page["results"][0]["excerpt"].clone()];
                assert!(!excerpts.is_empty());
                for excerpt in &excerpts {
                    let start = excerpt["start_byte"].as_u64().unwrap() as usize;
                    let end = excerpt["end_byte"].as_u64().unwrap() as usize;
                    assert_eq!(excerpt["text"], source[start..end]);
                    assert!(
                        start >= source.find(":title:").unwrap(),
                        "identity fields must not produce approximate excerpts"
                    );
                }
            }
        }
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn typo_tolerant_search_uses_normalized_word_lengths_and_bounded_edits() {
    let fixture = retrieval_fixture();
    let path = fixture.path().join("docs/word.mara.md");
    for (query, word, expected) in [
        ("cat", "cat", true),
        ("cat", "cut", false),
        ("cát", "cåt", false), // Three scalars, despite the UTF-8 byte count.
        ("cats", "cat", true),
        ("cafe", "cafes", true),
        ("cafe", "case", true),
        ("cafe", "caef", true), // An adjacent swap is one edit.
        ("cafe", "cxfx", false),
        ("project", "projecx", true),
        ("project", "projexx", false),
        ("projects", "projexxs", true),
        ("projects", "projxxxs", false),
        ("STRASSE", "Straße", true),
        ("STRASXE", "Straße", true),
        ("ßabcdef", "ssabcdxx", true), // Case folding expands to eight scalars.
        ("CAFÈ", "cafe\u{301}", true),
        ("ПРОЕКТ", "проетк", true),
        ("prj", "project", false), // No subsequence or substring mode.
        ("validate", "validation", false), // No stemming.
        ("project missing", "project", false),
    ] {
        let source =
            format!(":::mara requirement REQ-WORD\n:title: Catalog entry\n\n{word}\n:::\n");
        fs::write(&path, &source).unwrap();
        let output = mara(
            fixture.path(),
            &[
                "--format",
                "json",
                "search",
                query,
                "--path",
                "docs/word.mara.md",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            collection_nodes(&page).len(),
            usize::from(expected),
            "{query} -> {word}"
        );
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "search",
                    json!({"query":query, "paths":["docs/word.mara.md"]}),
                ),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            page
        );
        if expected {
            let excerpts = [page["results"][0]["excerpt"].clone()];
            assert!(
                excerpts
                    .iter()
                    .any(|e| e["text"].as_str().unwrap().contains(word)),
                "{query} -> {word}"
            );
            for excerpt in &excerpts {
                let start = excerpt["start_byte"].as_u64().unwrap() as usize;
                let end = excerpt["end_byte"].as_u64().unwrap() as usize;
                assert_eq!(excerpt["text"], source[start..end]);
            }
        }
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn typo_tolerant_search_preserves_exact_matches_filters_excerpts_and_pages() {
    let fixture = retrieval_fixture();
    let path = fixture.path().join("docs/typos.mara.md");
    fs::write(
        &path,
        ":::mara requirement REQ-APPROXIMATE\n:title: Project knowledge\n:status: accepted\n:derives_from: SCN-BASE\n\nValidaton Straße cafe\u{301}.\n:::\n\n:::mara requirement REQ-EXACT\n:title: Project knowledge\n:status: draft\n:derives_from: SCN-BASE\n\nValidation Straße cafe\u{301}.\n:::\n",
    )
    .unwrap();
    add_fixture_mids(fixture.path());
    let source = fs::read_to_string(&path).unwrap();

    // Exact results lead when present; otherwise equal field weights retain corpus order.
    for query in ["project validation", "projcet validation projcet"] {
        let mut cursor: Option<String> = None;
        let mut ids = Vec::new();
        loop {
            let mut args = vec![
                "--format",
                "json",
                "search",
                query,
                "--path",
                "docs/typos.mara.md",
                "--flavour",
                "requirement",
                "--relation",
                "derives_from",
                "--limit",
                "1",
            ];
            let mut params = json!({
                "query": query, "paths": ["docs/typos.mara.md"],
                "flavours": ["requirement"], "relations": ["derives_from"],
                "limit": 1,
            });
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
                params["cursor"] = json!(cursor);
            }
            let output = mara(fixture.path(), &args);
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(2, "search", params),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
            let items = collection_nodes(&page);
            assert_eq!(items.len(), 1, "query: {query}");
            ids.push(items[0]["id"].as_str().unwrap().to_owned());
            let excerpts = [page["results"][0]["excerpt"].clone()];
            assert!(
                excerpts
                    .iter()
                    .any(|e| e["text"].as_str().unwrap().contains("Project"))
            );
            for excerpt in &excerpts {
                let start = excerpt["start_byte"].as_u64().unwrap() as usize;
                let end = excerpt["end_byte"].as_u64().unwrap() as usize;
                assert_eq!(excerpt["text"], source[start..end]);
                assert_eq!(excerpt["partial"], true);
            }
            cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
            assert_eq!(page["has_more"], cursor.is_some());
            if cursor.is_none() {
                break;
            }
            assert!(ids.len() < 2, "continuation must finish");
        }
        if query == "project validation" {
            assert_eq!(ids, ["REQ-EXACT", "REQ-APPROXIMATE"]);
        } else {
            assert_eq!(ids, ["REQ-APPROXIMATE", "REQ-EXACT"]);
        }
    }

    // Filter values and selected handles must never use typo tolerance.
    for (option, value, params, expected) in [
        (
            "--field",
            "status=draft",
            json!({"fields":[{"key":"status","value":"draft"}]}),
            Some(1),
        ),
        (
            "--field",
            "status=drafx",
            json!({"fields":[{"key":"status","value":"drafx"}]}),
            Some(0),
        ),
        (
            "--path",
            "docs/typoz.mara.md",
            json!({"paths":["docs/typoz.mara.md"]}),
            Some(0),
        ),
        (
            "--flavour",
            "requiremenx",
            json!({"flavours":["requiremenx"]}),
            None,
        ),
        (
            "--relation",
            "derives_fron",
            json!({"relations":["derives_fron"]}),
            None,
        ),
        ("--id", "REQ-EXACT", json!({"ids":["REQ-EXACT"]}), Some(1)),
        ("--id", "REQ-EXACX", json!({"ids":["REQ-EXACX"]}), None),
    ] {
        let mut params = params;
        params["query"] = json!("projcet validation");
        let output = mara(
            fixture.path(),
            &[
                "--format",
                "json",
                "search",
                "projcet validation",
                option,
                value,
            ],
        );
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "search", params),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        if let Some(count) = expected {
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(collection_nodes(&page).len(), count, "{option} {value}");
            assert_eq!(result["structuredContent"], page);
        } else {
            assert!(!output.status.success(), "{option} {value}");
            assert_eq!(result["isError"], true);
        }
    }

    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
// @mara implements VER-UNIFIED-SEARCH
fn unified_search_owns_blocks_ranks_mixed_hits_and_pages_with_mcp_parity() {
    let fixture = retrieval_fixture();
    let source = "# Needle\n\nUnrelated prose.\n\n:::mara requirement REQ-OWNED\n:title: Owner\n\n## Needle\n\nNeedle inside an item.\n:::\n\nNeedle paragraph.\n\n- Needle list\n  - Needle nested\n\n> Needle quote\n> - Needle nested list\n\n| Header |\n| --- |\n| Needle cell |\n\n```text\nNeedle code\n```\n\n# Other\n\nNeedle needle needle repeated.\n\n# Needel\n";
    fs::write(fixture.path().join("docs/mixed.mara.md"), source).unwrap();
    fs::write(
        fixture.path().join("docs/narrative.mara.md"),
        "# Needle\n\nPlain narrative.\n",
    )
    .unwrap();
    let mut cursor: Option<String> = None;
    let mut hits = Vec::new();
    loop {
        let mut args = vec!["--format", "json", "search", "needle", "--limit", "2"];
        let mut params = json!({"query":"needle", "limit":2});
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor]);
            params["cursor"] = json!(cursor);
        }
        let output = mara(fixture.path(), &args);
        assert!(output.status.success(), "{}", stderr(&output));
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(page["format_version"], 2);
        assert!(page.get("items").is_none());
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "search", params),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            page
        );
        for hit in page["results"].as_array().unwrap() {
            let path = hit["node"]["source"]["path"].as_str().unwrap();
            let source = fs::read_to_string(fixture.path().join(path)).unwrap();
            let excerpt = &hit["excerpt"];
            let start = excerpt["start_byte"].as_u64().unwrap() as usize;
            let end = excerpt["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(&source[start..end], excerpt["text"].as_str().unwrap());
            assert!(excerpt["text"].as_str().unwrap().chars().count() <= 240);
            hits.push(hit.clone());
        }
        assert!(hits.len() <= 10);
        cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(hits.len(), 10);
    assert_eq!(hits[0]["node"]["kind"], "section");
    assert_eq!(hits[1]["node"]["id"], "REQ-OWNED");
    assert_eq!(hits[2]["node"]["source"]["path"], "docs/narrative.mara.md");
    assert_eq!(hits[9]["node"]["title"], "Needel");
    let kinds: Vec<_> = hits[3..9]
        .iter()
        .map(|h| h["node"]["block_kind"].clone())
        .collect();
    assert_eq!(
        kinds,
        json!([
            "paragraph",
            "list",
            "blockquote",
            "table",
            "code_block",
            "paragraph"
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    assert_eq!(
        hits.iter()
            .map(|h| h["node"]["reference"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        10
    );
    // The parent heading supplies context, never an inherited query term.
    let output = mara(
        fixture.path(),
        &["--format", "json", "search", "needle unrelated"],
    );
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page["results"], json!([]));
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "search",
            "needle",
            "--flavour",
            "requirement",
        ],
    );
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page["results"].as_array().unwrap().len(), 1);
    assert_eq!(page["results"][0]["node"]["id"], "REQ-OWNED");
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn unified_search_rejects_removed_names_and_options_and_narrative_stale_cursors() {
    let fixture = retrieval_fixture();
    fs::write(
        fixture.path().join("docs/narrative.mara.md"),
        "# Needle\n\nNeedle paragraph.\n",
    )
    .unwrap();
    for args in [
        vec!["item", "search", "needle"],
        vec!["search", "needle", "--excerpts"],
        vec!["search", "needle", "--kind", "block"],
    ] {
        assert!(!mara(fixture.path(), &args).status.success());
    }
    let first = mara(
        fixture.path(),
        &["--format", "json", "search", "needle", "--limit", "1"],
    );
    let page: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = page["next_cursor"].as_str().unwrap();
    fs::write(
        fixture.path().join("docs/unrelated.mara.md"),
        "Unrelated content.\n",
    )
    .unwrap();
    let stale = mara(
        fixture.path(),
        &["search", "needle", "--limit", "1", "--cursor", cursor],
    );
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("stale cursor"));
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(2, "item_search", json!({"query":"needle"})),
            mcp_call(3, "search", json!({"query":"needle","excerpts":true})),
            mcp_call(4, "search", json!({"query":"needle","kind":"block"})),
            mcp_call(
                5,
                "search",
                json!({"query":"needle","limit":1,"cursor":cursor}),
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

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn unified_search_keeps_large_blocks_whole_and_never_skips_oversized_identities() {
    let fixture = retrieval_fixture();
    let path = fixture.path().join("docs/large-search.mara.md");
    let source = format!("{}needle\n\nneedle tail.\n", "界\"\\ ".repeat(20_000));
    fs::write(&path, &source).unwrap();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "search",
            "needle",
            "--path",
            "docs/large-search.mara.md",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    let hits = page["results"].as_array().unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["node"]["source"]["start_byte"], 0);
    assert!(hits[0]["node"]["source"]["end_byte"].as_u64().unwrap() > 65_536);
    assert_eq!(hits[0]["excerpt"]["partial"], true);
    assert!(
        hits[0]["excerpt"]["text"]
            .as_str()
            .unwrap()
            .contains("needle")
    );
    assert!(hits[0]["excerpt"]["text"].as_str().unwrap().chars().count() <= 240);
    assert!(output.stdout.len() - 1 <= 65_536);

    fs::write(&path, format!(":::mara requirement REQ-FIRST\n:title: Needle\n\nSmall.\n:::\n\n:::mara requirement REQ-{}\n:title: Needle\n\nLarge identity.\n:::\n\n:::mara requirement REQ-LAST\n:title: Needle\n\nMust not skip to here.\n:::\n", "X".repeat(70_000))).unwrap();
    let first = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "search",
            "needle",
            "--path",
            "docs/large-search.mara.md",
            "--limit",
            "100",
        ],
    );
    assert!(first.status.success(), "{}", stderr(&first));
    let page: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(page["results"].as_array().unwrap().len(), 1);
    assert_eq!(page["results"][0]["node"]["id"], "REQ-FIRST");
    let cursor = page["next_cursor"].as_str().unwrap();
    let next = mara(
        fixture.path(),
        &[
            "search",
            "needle",
            "--path",
            "docs/large-search.mara.md",
            "--limit",
            "100",
            "--cursor",
            cursor,
        ],
    );
    assert!(!next.status.success());
    assert!(stderr(&next).contains("cannot fit the 65536-byte page budget"));
    assert!(next.stderr.len() < 1024);
    assert!(next.stdout.is_empty());
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "search",
                json!({"query":"needle", "paths":["docs/large-search.mara.md"],"limit":100,"cursor":cursor}),
            ),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn unified_search_resolves_schema_relation_filters_against_vocabulary() {
    for name in ["contains", "mentions"] {
        let fixture = retrieval_fixture();
        let schema_path = fixture.path().join(".mara/schema.yaml");
        let schema = fs::read_to_string(&schema_path).unwrap();
        fs::write(&schema_path, format!("{schema}\n  {name}:\n    description: Authored relation\n    source: [requirement]\n    target: [scenario]\n")).unwrap();
        let source_path = fixture.path().join("docs/a.mara.md");
        let source = fs::read_to_string(&source_path)
            .unwrap()
            .replace(":derives_from: SCN-BASE", &format!(":{name}: SCN-BASE"));
        fs::write(source_path, source).unwrap();
        let qualified = format!("schema:{name}");
        let builtin = format!("builtin:{name}");
        let output = mara(
            fixture.path(),
            &["--format", "json", "search", "", "--relation", &qualified],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        let page: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(page["results"].as_array().unwrap().len(), 1);
        assert_eq!(page["results"][0]["node"]["id"], "REQ-ALPHA");
        let rejected = mara(fixture.path(), &["search", "", "--relation", name]);
        assert!(!rejected.status.success());
        let expected = format!(
            "ambiguous relation '{name}'; search accepts schema relations only; use {qualified}"
        );
        assert!(
            stderr(&rejected).contains(&expected),
            "{}",
            stderr(&rejected)
        );
        assert!(!stderr(&rejected).contains(&builtin));
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                mcp_call(2, "search", json!({"query":"", "relations":[qualified]})),
                mcp_call(3, "search", json!({"query":"", "relations":[name]})),
                mcp_call(4, "search", json!({"query":"", "relations":[builtin]})),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            page
        );
        for id in [3, 4] {
            assert_eq!(mcp_response(&responses, id)["result"]["isError"], true);
        }
        let message = mcp_response(&responses, 3)["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(message.contains(&expected), "{message}");
        assert!(!message.contains(&builtin));
    }
}

// @mara checks DES-UNIFIED-KNOWLEDGE-DISCOVERY
#[test]
fn unified_search_excerpts_locate_decoded_headings_inside_owning_nodes() {
    let fixture = retrieval_fixture();
    let path = fixture.path().join("docs/decoded-search.mara.md");
    for (query, encoded) in [
        ("discovery", "disco&#118;ery"),
        ("discovery", "disco**very**"),
        ("discovery", "disco&#x76;ery"),
        ("strasse", "Stra&szlig;e"),
        ("CAFÉ", "caf&#x65;&#x301;"),
        ("discovery", "`disco`**very**"),
    ] {
        for prefix in [String::new(), "Background 界 &amp; ".repeat(30)] {
            let heading = format!("{prefix}{encoded}");
            for newline in ["\n", "\r\n"] {
                for kind in ["item", "block", "section"] {
                    let introduction = "Unrelated introduction. ".repeat(30);
                    let source = if kind == "item" {
                    format!(":::mara requirement REQ-OWNER\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00\n:title: Owner\n\n{introduction}\n\n## {heading}\n\nDetails.\n:::\n")
                } else if kind == "block" {
                    format!("> {introduction}\n>\n> ## {heading}\n>\n> Details.\n")
                } else {
                    format!("## {heading}\n\nDetails.\n")
                }.replace('\n', newline);
                    fs::write(&path, &source).unwrap();
                    let output = mara(
                        fixture.path(),
                        &[
                            "--format",
                            "json",
                            "search",
                            query,
                            "--path",
                            "docs/decoded-search.mara.md",
                        ],
                    );
                    assert!(output.status.success(), "{}", stderr(&output));
                    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
                    let hits = page["results"].as_array().unwrap();
                    assert_eq!(hits.len(), 1, "{heading} {kind}");
                    assert_eq!(hits[0]["node"]["kind"], kind);
                    assert_eq!(hits[0]["node"]["source"]["start_byte"], 0);
                    let excerpt = &hits[0]["excerpt"];
                    let start = excerpt["start_byte"].as_u64().unwrap() as usize;
                    let end = excerpt["end_byte"].as_u64().unwrap() as usize;
                    let heading_start = source.find(encoded).unwrap();
                    assert!(
                        start <= heading_start && end >= heading_start + encoded.len(),
                        "excerpt {start}..{end} misses heading at {heading_start}: {heading} {kind}"
                    );
                    assert_eq!(excerpt["text"], source[start..end]);
                    assert_eq!(excerpt["partial"], true);
                    assert!(excerpt["text"].as_str().unwrap().chars().count() <= 240);
                    assert_eq!(
                        excerpt["start_line"],
                        source[..start].bytes().filter(|b| *b == b'\n').count() + 1
                    );
                    assert_eq!(
                        excerpt["end_line"],
                        source[..end - 1].bytes().filter(|b| *b == b'\n').count() + 1
                    );
                    let responses = mcp_exchange(
                        fixture.path(),
                        &[
                            mcp_initialize(1),
                            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                            mcp_call(
                                2,
                                "search",
                                json!({"query":query, "paths":["docs/decoded-search.mara.md"]}),
                            ),
                        ],
                    );
                    assert_eq!(
                        mcp_response(&responses, 2)["result"]["structuredContent"],
                        page
                    );
                }
            }
        }
    }
}

// @mara checks DES-ITEM-LIST
#[test]
fn shared_library_filters_bind_selected_ids_to_list_continuation() {
    let fixture = retrieval_fixture();
    let project = mara::resolve_project(Some(fixture.path()), fixture.path()).unwrap();
    let schema = mara::load_schema(&project).unwrap();
    let corpus = mara::load_corpus(&project, &schema).unwrap();
    let filters = mara::ItemFilters::new(vec![], vec![], vec![], vec![], Some(1))
        .with_ids(vec!["REQ-ALPHA".into(), "REQ-BETA".into()]);
    let first = mara::list_items(&corpus, &schema, &filters).unwrap();
    assert!(first.has_more);
    let changed = mara::ItemFilters::new(vec![], vec![], vec![], vec![], Some(1))
        .with_ids(vec!["REQ-ALPHA".into(), "DES-ALPHA".into()])
        .with_cursor(first.next_cursor);
    assert!(matches!(
        mara::list_items(&corpus, &schema, &changed),
        Err(mara::QueryError::InvalidPage { .. })
    ));
}
