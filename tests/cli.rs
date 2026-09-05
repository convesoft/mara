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
    let backfill = mara(fixture.path(), &["project", "mid", "backfill"]);
    assert!(backfill.status.success(), "{}", stderr(&backfill));
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
                "--format", "json", "item", "related", "REQ-HUB", "--limit", "7",
            ];
            let mut params = json!({"id":"REQ-HUB", "limit":7});
            if filtered {
                args.extend([
                    "--direction",
                    "incoming",
                    "--relation",
                    "supersedes",
                    "--flavour",
                    "requirement",
                ]);
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
            let items = page["items"].as_array().unwrap();
            assert!(!items.is_empty() && items.len() <= 7);
            for entry in items {
                assert!(is_mid(entry["item"]["mid"].as_str().unwrap()));
                assert!(entry["item"].get("body").is_none());
                assert!(entry["item"].get("excerpts").is_none());
                actual.push((
                    entry["direction"].as_str().unwrap().to_owned(),
                    entry["relation"].as_str().unwrap().to_owned(),
                    entry["item"]["id"].as_str().unwrap().to_owned(),
                ));
            }
            requests.push(mcp_call(pages.len() as u64 + 2, "item_related", params));
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
            for i in (0..43).rev() {
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
    let output = mara(
        fixture.path(),
        &["--format", "json", "item", "related", "REQ-HUB"],
    );
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 20);
    assert_eq!(page["has_more"], true);
}

#[test]
fn related_pages_reject_changed_inputs_and_invalid_continuation() {
    let fixture = retrieval_fixture();
    let first = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "related",
            "REQ-ALPHA",
            "--limit",
            "1",
        ],
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    for args in [
        vec![
            "item",
            "related",
            "REQ-ALPHA",
            "--limit",
            "2",
            "--cursor",
            cursor,
        ],
        vec![
            "item", "related", "SCN-BASE", "--limit", "1", "--cursor", cursor,
        ],
        vec![
            "item",
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
            "item",
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
            "item",
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
        vec!["item", "related", "REQ-ALPHA", "--cursor", "malformed"],
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
                "item",
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
        let output = mara(
            fixture.path(),
            &["item", "related", "REQ-ALPHA", "--limit", limit],
        );
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
            &[
                "item",
                "related",
                "REQ-ALPHA",
                "--limit",
                "1",
                "--cursor",
                cursor,
            ],
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
                    "item_related",
                    json!({"id":"REQ-ALPHA","limit":1,"cursor":cursor}),
                ),
                mcp_call(
                    3,
                    "item_related",
                    json!({"id":"REQ-ALPHA","cursor":"malformed"}),
                ),
                mcp_call(4, "item_related", json!({"id":"REQ-ALPHA","limit":101})),
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
                "item",
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
        assert_eq!(page["items"][0]["direction"], "incoming");
        assert_eq!(page["has_more"], false);
        assert!(page["next_cursor"].is_null());
    }
}

#[test]
fn related_pages_bound_escaped_unicode_titles_and_preserve_every_entry() {
    let fixture = retrieval_fixture();
    let title = "界\"\\".repeat(200);
    let source = (0..100).map(|i| format!(":::mara design DES-BUDGET-{i}\n:title: {title}\n:satisfies: REQ-ALPHA\n\nBody.\n:::\n\n")).collect::<String>();
    fs::write(fixture.path().join("docs/budget.mara.md"), source).unwrap();
    let mut cursor: Option<String> = None;
    let mut ids = Vec::new();
    let mut pages = 0;
    loop {
        assert!(pages < 10);
        let mut args = vec![
            "--format",
            "json",
            "item",
            "related",
            "REQ-ALPHA",
            "--direction",
            "incoming",
            "--limit",
            "100",
        ];
        let mut params = json!({"id":"REQ-ALPHA", "direction":"incoming", "limit":100});
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
                mcp_call(2, "item_related", params),
            ],
        );
        assert_eq!(
            mcp_response(&responses, 2)["result"]["structuredContent"],
            page
        );
        if pages == 0 {
            assert!(page["items"].as_array().unwrap().len() < 100);
        }
        for entry in page["items"].as_array().unwrap() {
            let item = &entry["item"];
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
        &["item", "related", "REQ-ALPHA", "--direction", "incoming"],
    );
    assert!(stdout(&human).contains(" [title truncated]"));
    assert!(stdout(&human).contains("page\thas_more=true\tnext_cursor="));
    let full = mara(
        fixture.path(),
        &["--format", "json", "item", "get", "DES-BUDGET-0"],
    );
    let full: Value = serde_json::from_slice(&full.stdout).unwrap();
    assert_eq!(full["summary"]["title"], title);
}

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
    let args = [
        "--format",
        "json",
        "item",
        "related",
        "REQ-ALPHA",
        "--direction",
        "incoming",
    ];
    let output = mara(fixture.path(), &args);
    assert!(output.status.success(), "{}", stdout(&output));
    let first: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["items"][0]["item"]["id"], "DES-ALPHA");
    let cursor = first["next_cursor"].as_str().unwrap();
    let output = mara(
        fixture.path(),
        &[
            "item",
            "related",
            "REQ-ALPHA",
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
                "item_related",
                json!({"id":"REQ-ALPHA", "direction":"incoming", "cursor":cursor}),
            ),
        ],
    );
    let result = &mcp_response(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert!(serde_json::to_vec(result).unwrap().len() < 1024);
}

#[test]
fn bounded_search_and_list_continue_completely_with_cli_mcp_parity() {
    let fixture = retrieval_fixture();
    let source = (0..45)
        .map(|index| format!(":::mara requirement REQ-PAGE-{index}\n:title: Page {index}\n\nNeed bounded knowledge.\n:::\n\n"))
        .collect::<String>();
    fs::write(fixture.path().join("docs/pages.mara.md"), source).unwrap();
    for operation in ["search", "list"] {
        let mut cursor: Option<String> = None;
        let mut ids = Vec::new();
        loop {
            let mut args = vec!["--format", "json", "item", operation];
            if operation == "search" {
                args.push("bounded knowledge");
            }
            args.extend(["--path", "docs/pages.mara.md", "--limit", "7"]);
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
            }
            let output = mara(fixture.path(), &args);
            assert!(output.status.success(), "{}", stderr(&output));
            let page: Value = serde_json::from_slice(&output.stdout).unwrap();
            let mut params = json!({"paths": ["docs/pages.mara.md"], "limit": 7});
            if operation == "search" {
                params["query"] = json!("bounded knowledge");
            }
            if let Some(cursor) = &cursor {
                params["cursor"] = json!(cursor);
            }
            let responses = mcp_exchange(
                fixture.path(),
                &[
                    mcp_initialize(1),
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    mcp_call(2, &format!("item_{operation}"), params),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
            let items = page["items"].as_array().unwrap();
            assert!(!items.is_empty() && items.len() <= 7);
            for item in items {
                assert!(item.get("body").is_none());
                assert!(item.get("excerpts").is_none());
                ids.push(item["id"].as_str().unwrap().to_owned());
            }
            assert_eq!(page["has_more"], !page["next_cursor"].is_null());
            cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(
            ids,
            (0..45).map(|i| format!("REQ-PAGE-{i}")).collect::<Vec<_>>()
        );
    }
    let default = mara(fixture.path(), &["--format", "json", "item", "list"]);
    let page: Value = serde_json::from_slice(&default.stdout).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 20);
    assert_eq!(page["has_more"], true);
}

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
    let backfill = mara(fixture.path(), &["project", "mid", "backfill"]);
    assert!(backfill.status.success(), "{}", stderr(&backfill));
    let source = fs::read_to_string(fixture.path().join("docs/passage.mara.md")).unwrap();
    let got = mara(
        fixture.path(),
        &["--format", "json", "item", "get", "REQ-PASSAGE"],
    );
    let got: Value = serde_json::from_slice(&got.stdout).unwrap();
    let mid = got["summary"]["mid"].as_str().unwrap();
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "search",
            "draft STRASSE CAFÉ",
            "--id",
            mid,
            "--id",
            "REQ-PASSAGE",
            "--excerpts",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let page: Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title_truncated"], true);
    assert_eq!(items[0]["title"].as_str().unwrap().chars().count(), 256);
    assert_eq!(got["summary"]["title"], title);
    let excerpts = items[0]["excerpts"].as_array().unwrap();
    assert!(!excerpts.is_empty() && excerpts.len() <= 3);
    let mut previous_end = 0;
    for excerpt in excerpts {
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
                "item_search",
                json!({"query":"draft STRASSE CAFÉ", "ids":[mid,"REQ-PASSAGE"], "excerpts":true}),
            ),
            mcp_call(
                3,
                "item_search",
                json!({"query":"draft", "ids":["REQ-MISSING"]}),
            ),
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
            "item",
            "search",
            "draft",
            "--id",
            mid,
            "--flavour",
            "scenario",
        ],
    );
    let excluded: Value = serde_json::from_slice(&excluded.stdout).unwrap();
    assert_eq!(excluded["items"], json!([]));
    let human = mara(
        fixture.path(),
        &[
            "item",
            "search",
            "draft STRASSE CAFÉ",
            "--id",
            mid,
            "--excerpts",
        ],
    );
    assert!(human.status.success(), "{}", stderr(&human));
    assert!(stdout(&human).contains("[title truncated]"));
    assert!(stdout(&human).contains("excerpt\tpartial=true\tdocs/passage.mara.md:"));
    assert!(stdout(&human).ends_with("page\thas_more=false\n"));
    let empty = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "search",
            "",
            "--id",
            mid,
            "--excerpts",
        ],
    );
    let empty: Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(empty["items"][0]["excerpts"], json!([]));
    // Duplicate source IDs make exact selection ambiguous, even with a filter
    // that would otherwise remove every candidate.
    fs::write(fixture.path().join("docs/duplicate.mara.md"), &source).unwrap();
    let ambiguous = mara(
        fixture.path(),
        &[
            "item",
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

#[test]
fn pagination_rejects_changed_inputs_and_invalid_limits() {
    let fixture = retrieval_fixture();
    let first = mara(
        fixture.path(),
        &[
            "--format", "json", "item", "search", "alpha", "--limit", "1",
        ],
    );
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["next_cursor"].as_str().unwrap();
    for (query, limit) in [("alpha", "2"), ("beta", "1")] {
        let changed = mara(
            fixture.path(),
            &[
                "item", "search", query, "--limit", limit, "--cursor", cursor,
            ],
        );
        assert!(!changed.status.success());
        assert!(stderr(&changed).contains("restart"), "{}", stderr(&changed));
    }
    let path = fixture.path().join("docs/a.mara.md");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("Narrative edit.\n{original}")).unwrap();
    let changed = mara(
        fixture.path(),
        &[
            "item", "search", "alpha", "--limit", "1", "--cursor", cursor,
        ],
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
                "item_search",
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
        &[
            "item", "search", "alpha", "--limit", "1", "--cursor", cursor,
        ],
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
        &[
            "item", "search", "alpha", "--limit", "1", "--cursor", cursor,
        ],
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
        let mut args = vec![
            "--format",
            "json",
            "item",
            "search",
            "needle",
            "--excerpts",
            "--limit",
            "100",
        ];
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
                    mcp_call(
                        2,
                        "item_search",
                        json!({"query":"needle","excerpts":true,"limit":100}),
                    ),
                ],
            );
            assert_eq!(
                mcp_response(&responses, 2)["result"]["structuredContent"],
                page
            );
        }
        for item in page["items"].as_array().unwrap() {
            assert_eq!(item["title_truncated"], true);
            assert_eq!(item["excerpts"].as_array().unwrap().len(), 3);
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

    for query in ["zEbRa", "accepted", "DES-ALPHA"] {
        let searched = mara(fixture.path(), &["item", "search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(stdout(&searched).lines().count(), 2, "query: {query}");
    }
    let searched = mara(fixture.path(), &["item", "search", "alpha"]);
    assert!(searched.status.success(), "{}", stderr(&searched));
    assert_eq!(
        stdout(&searched),
        "REQ-ALPHA\trequirement\tAlpha requirement\tdocs/a.mara.md:7\nDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\npage\thas_more=false\n"
    );

    let case_folded = mara(fixture.path(), &["item", "search", "STRASSE"]);
    assert!(case_folded.status.success(), "{}", stderr(&case_folded));
    assert_eq!(
        stdout(&case_folded),
        "SCN-GERMAN\tscenario\tStraße\tdocs/b.mara.md:16\npage\thas_more=false\n"
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
            "REQ-CROSS-FIELD\trequirement\tProject knowledge\tdocs/search.mara.md:1\npage\thas_more=false\n",
            "query: {query}"
        );
    }

    for query in ["project missing", "projects validation", "project valid"] {
        let searched = mara(fixture.path(), &["item", "search", query]);
        assert!(searched.status.success(), "{}", stderr(&searched));
        assert_eq!(
            stdout(&searched),
            "page\thas_more=false\n",
            "query: {query}"
        );
    }

    let unicode_equivalent = mara(fixture.path(), &["item", "search", "CAFE\u{301} WORKFLOW"]);
    assert!(
        unicode_equivalent.status.success(),
        "{}",
        stderr(&unicode_equivalent)
    );
    assert_eq!(
        stdout(&unicode_equivalent),
        "SCN-UNICODE\tscenario\tCafé workflow\tdocs/search.mara.md:7\npage\thas_more=false\n"
    );
}

#[test]
fn item_related_returns_filtered_direct_neighbours_with_relation_and_direction() {
    let fixture = retrieval_fixture();

    let related = mara(fixture.path(), &["item", "related", "REQ-ALPHA"]);
    assert!(related.status.success(), "{}", stderr(&related));
    assert_eq!(
        stdout(&related),
        "outgoing\tderives_from\tSCN-BASE\tscenario\tBase scenario\tdocs/a.mara.md:1\nincoming\tsatisfies\tDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\npage\thas_more=false\n"
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
        "incoming\tsatisfies\tDES-ALPHA\tdesign\tAlpha design\tdocs/b.mara.md:9\npage\thas_more=false\n"
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
    assert_eq!(stdout(&no_match), "page\thas_more=false\n");
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
            "item_delete",
            "item_rename",
            "item_move",
            "item_update",
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
        vec!["item", "delete", "REQ-MOVE"],
        vec!["item", "rename", "REQ-MOVE", "REQ-NEW"],
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
            mcp_call(
                4,
                "item_rename",
                json!({"reference":"REQ-MOVE","new_id":"REQ-NEW"}),
            ),
        ],
    );
    assert_eq!(mcp_response(&responses, 2)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 3)
            .to_string()
            .contains("unrecoverable transaction")
    );
    assert_eq!(mcp_response(&responses, 4)["result"]["isError"], true);
    assert!(
        mcp_response(&responses, 4)
            .to_string()
            .contains("pending transaction")
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

fn update_fixture() -> (TempDir, String, String) {
    let fixture = TempDir::new().unwrap();
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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    (fixture, source, mid)
}

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
    let cleared = mara(
        fixture.path(),
        &["item", "update", "REQ-EDIT", "--clear-field", "tag"],
    );
    assert!(cleared.status.success(), "{}", stderr(&cleared));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        expected.replace(":tag:\tonly  \r\n", "")
    );
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    assert!(!fixture.path().join(".mara/transaction.json").exists());
}

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
    for command in [
        vec!["project", "validate"],
        vec!["item", "validate", "REQ-DRAFT"],
    ] {
        let validation = mara(fixture.path(), &command);
        assert!(!validation.status.success());
        assert!(stderr(&validation).contains("required body is empty"));
    }
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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
}

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

fn delete_fixture() -> (TempDir, String, String, String) {
    let fixture = TempDir::new().unwrap();
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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    assert!(
        !mara(fixture.path(), &["item", "get", &mid])
            .status
            .success()
    );
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
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    let cli = mara(
        fixture.path(),
        &["--format", "json", "item", "delete", &mid],
    );
    assert!(!cli.status.success());
    let result: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let error = result["error"]["message"].as_str().unwrap();
    assert_eq!(error.matches("(bytes ").count(), 10, "{error}");
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
    ] {
        assert_eq!(
            fs::read_to_string(fixture.path().join(file)).unwrap(),
            expected
        );
    }
}

#[test]
fn item_delete_ignores_outgoing_self_references_and_code_examples() {
    let (fixture, source, other, mid) = delete_fixture();
    let source = source.replace(":title: REQ-DELETE", &format!(":title: REQ-DELETE\n:depends_on: REQ-KEEP\n:depends_on: REQ-DELETE\n:depends_on: {mid}"))
        .replace("Exact Unicode body: żółć.", &format!("[[REQ-KEEP]] [[REQ-DELETE]] [[{mid}]]"));
    let other = other.replace("Exact Unicode body: żółć.", &format!("`[[REQ-DELETE]] [[{mid}]]`\n\n```text\n[[REQ-DELETE]] [[{mid}]]\n```\n\n\\[[REQ-DELETE]] \\[[{mid}]]"));
    let other = format!("Narrative [[REQ-DELETE]] [[{mid}]].\n\n{other}");
    fs::write(fixture.path().join("delete.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("keep.mara.md"), &other).unwrap();
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
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
            mcp_call(3, "project_validate", json!({})),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        serde_json::from_slice::<Value>(&cli.stdout).unwrap()
    );
    assert_eq!(
        mcp_response(&responses, 3)["result"]["structuredContent"]["valid"],
        true
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("delete.mara.md")).unwrap(),
        ""
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("keep.mara.md")).unwrap(),
        other
    );
}

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

fn rename_fixture() -> (TempDir, String, String, String) {
    let (fixture, source, _) = move_fixture();
    let original: Value = serde_json::from_slice(
        &mara(
            fixture.path(),
            &["--format", "json", "item", "get", "REQ-MOVE"],
        )
        .stdout,
    )
    .unwrap();
    let mid = original["summary"]["mid"].as_str().unwrap().to_owned();
    let source = source
        .replace(
            ":title: REQ-MOVE\r\n",
            ":title: REQ-MOVE\r\n:depends_on: REQ-MOVE\r\n",
        )
        .replace(
            "Exact Unicode body:",
            "Self [[REQ-MOVE]]. Exact Unicode body:",
        );
    let destination = fs::read_to_string(fixture.path().join("destination.mara.md"))
        .unwrap()
        .replace(
            ":depends_on: REQ-MOVE",
            &format!(":depends_on:\tREQ-MOVE  \n:depends_on: {mid}"),
        )
        .replace(
            "Reference [[REQ-MOVE]].",
            &format!(
                r#"Reference [[REQ-MOVE]], [[REQ-MOVE]] and [[{mid}]].
Unicode żółć REQ-MOVE prose; [REQ-MOVE](https://example.com/REQ-MOVE).
Literal `[[REQ-MOVE]]` and escaped \[[REQ-MOVE]].
Unsupported labelled syntax [[REQ-MOVE|REQ-MOVE]].

```markdown
[[REQ-MOVE]]
```

<!-- [[REQ-MOVE]] -->

<div>
[[REQ-MOVE]]
</div>
"#
            ),
        );
    let destination = format!("Narrative [[REQ-MOVE]].\n\n{destination}\nTail REQ-MOVE");
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("destination.mara.md"), &destination).unwrap();
    fs::write(
        fixture.path().join("untouched.mara.md"),
        "REQ-MOVE [[REQ-MOVE]]\r\n",
    )
    .unwrap();
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    (fixture, source, destination, mid)
}

#[test]
fn item_rename_preserves_bytes_and_mid_graph_across_documents() {
    let (fixture, source, destination, mid) = rename_fixture();
    #[cfg(unix)]
    for (path, mode) in [("source.mara.md", 0o640), ("destination.mara.md", 0o604)] {
        fs::set_permissions(fixture.path().join(path), fs::Permissions::from_mode(mode)).unwrap();
    }
    let before: Value = serde_json::from_slice(
        &mara(fixture.path(), &["--format", "json", "item", "get", &mid]).stdout,
    )
    .unwrap();
    let git = Command::new("git")
        .arg("init")
        .arg(fixture.path())
        .output()
        .unwrap();
    assert!(git.status.success());
    let output = mara(
        fixture.path(),
        &[
            "--format",
            "json",
            "item",
            "rename",
            &mid,
            "REQ-RENAMED-LONGER",
        ],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result,
        json!({"mid":mid, "old_id":"REQ-MOVE", "new_id":"REQ-RENAMED-LONGER", "paths":["destination.mara.md","source.mara.md"]})
    );
    let expected_source = source
        .replace("requirement REQ-MOVE", "requirement REQ-RENAMED-LONGER")
        .replace(":depends_on: REQ-MOVE", ":depends_on: REQ-RENAMED-LONGER")
        .replace("Self [[REQ-MOVE]]", "Self [[REQ-RENAMED-LONGER]]");
    let expected_destination = destination
        .replace(":depends_on:\tREQ-MOVE", ":depends_on:\tREQ-RENAMED-LONGER")
        .replace(
            "Reference [[REQ-MOVE]], [[REQ-MOVE]]",
            "Reference [[REQ-RENAMED-LONGER]], [[REQ-RENAMED-LONGER]]",
        );
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        expected_source
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        expected_destination
    );
    assert_eq!(
        fs::read(fixture.path().join("untouched.mara.md")).unwrap(),
        b"REQ-MOVE [[REQ-MOVE]]\r\n"
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
    let after: Value = serde_json::from_slice(
        &mara(
            fixture.path(),
            &["--format", "json", "item", "get", "REQ-RENAMED-LONGER"],
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(after["summary"]["mid"], before["summary"]["mid"]);
    for direction in ["incoming_relations", "outgoing_relations"] {
        let endpoints = |value: &Value| {
            value[direction]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| (entry["relation"].clone(), entry["item"]["mid"].clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(endpoints(&after), endpoints(&before));
    }
    assert!(
        !mara(fixture.path(), &["item", "get", "REQ-MOVE"])
            .status
            .success()
    );
    assert!(
        mara(fixture.path(), &["item", "get", &mid])
            .status
            .success()
    );
    assert!(
        mara(fixture.path(), &["project", "validate"])
            .status
            .success()
    );
    assert!(!fixture.path().join(".mara/transaction.json").exists());
    assert!(
        !Command::new("git")
            .current_dir(fixture.path())
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn item_rename_cli_mcp_and_human_results_agree() {
    let (fixture, source, destination, mid) = rename_fixture();
    let output = mara(
        fixture.path(),
        &["--format", "json", "item", "rename", "REQ-MOVE", "REQ-X"],
    );
    assert!(output.status.success(), "{}", stdout(&output));
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected_source = fs::read(fixture.path().join("source.mara.md")).unwrap();
    let expected_destination = fs::read(fixture.path().join("destination.mara.md")).unwrap();
    fs::write(fixture.path().join("source.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("destination.mara.md"), &destination).unwrap();
    let responses = mcp_exchange_with_arguments(
        fixture.path(),
        &["mcp", "--project", fixture.path().to_str().unwrap()],
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            mcp_call(2, "item_rename", json!({"reference":mid,"new_id":"REQ-X"})),
            mcp_call(
                3,
                "item_rename",
                json!({"reference":mid,"new_id":"REQ-X","project":fixture.path()}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        expected
    );
    assert_eq!(mcp_response(&responses, 3)["result"]["isError"], true);
    assert_eq!(
        fs::read(fixture.path().join("source.mara.md")).unwrap(),
        expected_source
    );
    assert_eq!(
        fs::read(fixture.path().join("destination.mara.md")).unwrap(),
        expected_destination
    );
    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            mcp_call(
                2,
                "item_rename",
                json!({"project":fixture.path(),"reference":"REQ-X","new_id":"REQ-X"}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        json!({"mid":mid,"old_id":"REQ-X","new_id":"REQ-X","paths":[]})
    );
    let output = mara(fixture.path(), &["item", "rename", "REQ-X", "REQ-MOVE"]);
    assert!(output.status.success());
    for value in [
        &mid,
        "REQ-X",
        "REQ-MOVE",
        "source.mara.md",
        "destination.mara.md",
    ] {
        assert!(stdout(&output).contains(value));
    }
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
fn item_rename_rejections_leave_source_unchanged_with_cli_mcp_parity() {
    let (fixture, source, destination, mid) = rename_fixture();
    for (reference, new_id) in [
        ("REQ-MOVE", "bad"),
        ("REQ-MOVE", "DES-WRONG"),
        ("REQ-MOVE", "REQ-STAY"),
        ("REQ-MOVE", &mid),
        ("REQ-MOVE", "REQ-A\nREQ-B"),
        ("REQ-UNKNOWN", "REQ-X"),
    ] {
        let output = mara(
            fixture.path(),
            &["--format", "json", "item", "rename", reference, new_id],
        );
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        let responses = mcp_exchange(
            fixture.path(),
            &[
                mcp_initialize(1),
                json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
                mcp_call(
                    2,
                    "item_rename",
                    json!({"reference":reference,"new_id":new_id}),
                ),
            ],
        );
        let result = &mcp_response(&responses, 2)["result"];
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(error["error"]["message"].as_str().unwrap())
        );
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
        fixture.path().join("destination.mara.md"),
        destination.replace("[[REQ-MOVE]]", "[[REQ-MISSING]]"),
    )
    .unwrap();
    let output = mara(fixture.path(), &["item", "rename", "REQ-MOVE", "REQ-X"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("validation fails"));
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        source
    );
}
