use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
mod support;
use support::*;

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
#[test]
// @mara implements VER-DOCUMENT-NAVIGATION
fn unified_related_navigates_mentions_relations_and_structure_with_exact_evidence() {
    let fixture = retrieval_fixture();
    let source = "# Portal\r\n\r\nStart here [[REQ-HUB]] and [[REQ-HUB]].\r\n\r\n:::mara requirement REQ-HUB\r\n:title: Hub\r\n:depends_on: REQ-TARGET\r\n:depends_on: REQ-HUB\r\n\r\n## Inner\r\n\r\nSee [[REQ-TARGET]].\r\n:::\r\n\r\n:::mara requirement REQ-TARGET\r\n:title: Target\r\n\r\nTarget body.\r\n:::\r\n\r\n[Inner](#inner) and [document](a.mara.md).\r\n";
    fs::write(fixture.path().join("docs/portal.mara.md"), source).unwrap();
    assert!(
        mara(fixture.path(), &["project", "mid", "backfill"])
            .status
            .success()
    );
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
#[test]
fn unified_related_qualifies_collisions_against_vocabulary_and_rejects_old_interface() {
    let fixture = retrieval_fixture();
    assert!(
        mara(fixture.path(), &["project", "mid", "backfill"])
            .status
            .success()
    );
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
