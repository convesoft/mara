use std::{collections::BTreeSet, fs, path::Path, process::Command};

use mara::{
    ConnectionKind, DiscoveryGraph, DiscoveryNodeKind, RelationDirection, Template,
    initialize_project, load_corpus, load_schema, resolve_project,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn cli(root: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .current_dir(root)
        .args(["--format", "json"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn summaries(graph: &DiscoveryGraph<'_>) -> Vec<Value> {
    graph
        .nodes()
        .map(|node| serde_json::to_value(node.summary()).unwrap())
        .collect()
}

#[test]
fn every_repository_and_padded_table_node_has_a_reusable_source_reference() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    fs::write(
        fixture.path().join("padded.mara.md"),
        "| A | B | C |\n|---|---|---|\n| One |\n",
    )
    .unwrap();
    for project in [
        project,
        resolve_project(Some(Path::new(env!("CARGO_MANIFEST_DIR"))), ".").unwrap(),
    ] {
        let schema = load_schema(&project).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        for node in graph.nodes() {
            let resolved = graph.resolve(node.reference()).unwrap();
            assert_eq!(node.summary(), resolved.summary());
        }
    }
}

#[test]
fn shared_summaries_preserve_full_sources_and_bounded_context() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let title = "é🙂".repeat(160);
    let filename = format!("{}.mara.md", "n".repeat(200));
    fs::write(
        fixture.path().join(&filename),
        format!("# {title}\n\n> ### Local\n>\n> - é🙂\n\n").replace('\n', "\r\n"),
    )
    .unwrap();
    cli(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-ONE",
            &filename,
            "--title",
            &title,
            "--body",
            "## Inside\n\nBody.",
        ],
    );
    fs::write(fixture.path().join("empty.mara.md"), "").unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let mut references = BTreeSet::new();
    for node in graph.nodes() {
        let value = serde_json::to_value(node.summary()).unwrap();
        assert!(references.insert(node.reference().to_owned()));
        assert_eq!(value["reference"], node.reference());
        assert_eq!(
            graph.resolve(node.reference()).unwrap().source(),
            node.source()
        );
        assert_eq!(
            value["source"]["path"],
            node.source().path().to_str().unwrap()
        );
        let span = node.source().span();
        assert_eq!(value["source"]["start_byte"], span.start_byte());
        assert_eq!(value["source"]["end_byte"], span.end_byte());
        assert_eq!(value["source"]["start_line"], span.start_line());
        assert_eq!(value["source"]["end_line"], span.end_line());
        let source = corpus
            .documents()
            .iter()
            .find(|doc| doc.path() == node.source().path())
            .unwrap()
            .source();
        assert!(source.get(span.start_byte()..span.end_byte()).is_some());
        let mut ancestor = node.parent();
        let mut nearest_section = None;
        while let Some(parent) = ancestor {
            if matches!(parent.kind(), DiscoveryNodeKind::Section { .. }) {
                nearest_section = Some(parent.reference());
                break;
            }
            ancestor = parent.parent();
        }
        assert_eq!(
            value["context"]["parent"].as_str(),
            node.parent().map(|p| p.reference())
        );
        assert_eq!(value["context"]["section"].as_str(), nearest_section);
        assert!(value["context"].as_object().unwrap().len() <= 2);
        for reference in value["context"].as_object().unwrap().values() {
            assert!(graph.resolve(reference.as_str().unwrap()).is_ok());
        }
        match node.kind() {
            DiscoveryNodeKind::Item(item) => {
                assert_eq!(value["kind"], "item");
                assert_eq!(value["reference"], item.mid().unwrap());
                assert_eq!(value["id"], item.id());
                assert_eq!(value["mid"], item.mid().unwrap());
                assert_eq!(value["flavour"], "requirement");
                assert_eq!(value["title"], "é🙂".repeat(128));
                assert_eq!(value["title_truncated"], true);
            }
            DiscoveryNodeKind::Section { heading } => {
                assert_eq!(value["kind"], "section");
                assert!(value["heading_level"].is_number());
                let full = heading.heading_text().unwrap();
                assert_eq!(value["title"], full.chars().take(256).collect::<String>());
                assert_eq!(value["title_truncated"], full.chars().count() > 256);
            }
            DiscoveryNodeKind::MarkdownBlock(_) => {
                assert_eq!(value["kind"], "block");
                assert!(value["block_kind"].is_string());
                assert!(value.get("title").is_none());
            }
            DiscoveryNodeKind::Document(_) => {
                assert_eq!(value["kind"], "document");
                assert!(value.get("title").is_none());
                if node.source().path() == Path::new("empty.mara.md") {
                    // Fixed v1 vector, independently calculated from the documented
                    // framing in the implementation; catches accidental encoding drift.
                    assert_eq!(
                        node.reference(),
                        "mara:node:1:3a162694bbd212454d23e7cfa61fd034096a907ac18b3650525fc3f2d9d2cf3d"
                    );
                }
            }
        }
        if !matches!(node.kind(), DiscoveryNodeKind::Item(_)) {
            for key in ["id", "mid", "flavour"] {
                assert!(value.get(key).is_none());
            }
        }
        if !matches!(node.kind(), DiscoveryNodeKind::Section { .. }) {
            assert!(value.get("heading_level").is_none());
        }
        if !matches!(node.kind(), DiscoveryNodeKind::MarkdownBlock(_)) {
            assert!(value.get("block_kind").is_none());
        }
    }
    // List, list item and paragraph can occupy identical spans; their kinds distinguish them.
    assert_eq!(references.len(), graph.nodes().count());
}

#[test]
fn handles_survive_unrelated_edits_but_reject_containing_edits_and_moves() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "# Target\n\nNarrative.\n";
    fs::write(fixture.path().join("target.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let original = summaries(&graph);
    let root_reference = graph.nodes().next().unwrap().reference().to_owned();
    // A preceding document changes private graph indexes and adds a live backlink.
    fs::write(
        fixture.path().join("a.mara.md"),
        "[Target](target.mara.md)\n",
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    for value in &original {
        assert_eq!(
            serde_json::to_value(
                graph
                    .resolve(value["reference"].as_str().unwrap())
                    .unwrap()
                    .summary()
            )
            .unwrap(),
            *value
        );
    }
    assert!(
        graph
            .resolve(&root_reference)
            .unwrap()
            .connections(RelationDirection::Incoming)
            .iter()
            .any(|edge| edge.kind == ConnectionKind::Mentions)
    );
    for change in [
        source.replace("Narrative", "Different"),
        source.replace('\n', "\r\n"),
    ] {
        fs::write(fixture.path().join("target.mara.md"), change).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        for value in &original {
            let error = graph
                .resolve(value["reference"].as_str().unwrap())
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains("search again"), "{error}");
        }
    }
    fs::write(fixture.path().join("target.mara.md"), source).unwrap();
    fs::rename(
        fixture.path().join("target.mara.md"),
        fixture.path().join("moved.mara.md"),
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    for value in &original {
        assert!(graph.resolve(value["reference"].as_str().unwrap()).is_err());
    }
    for reference in [
        "mara:node:2:unsupported",
        "mara:node:1:malformed",
        &format!("{root_reference}0"),
    ] {
        assert!(
            graph
                .resolve(reference)
                .err()
                .unwrap()
                .to_string()
                .contains("search again")
        );
    }
}

#[test]
fn item_mids_resolve_after_real_cli_update_rename_and_move() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    cli(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-ONE",
            "item.mara.md",
            "--title",
            "One",
            "--body",
            "# Local\n\nBody.",
        ],
    );
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let item = graph.resolve("REQ-ONE").unwrap();
    let mid = item.reference().to_owned();
    assert_eq!(mid, corpus.items().next().unwrap().mid().unwrap());
    let section_handle = item.children()[0].reference().to_owned();
    cli(
        fixture.path(),
        &["item", "update", &mid, "--title", "Changed"],
    );
    cli(fixture.path(), &["item", "rename", &mid, "REQ-NEW"]);
    cli(fixture.path(), &["item", "move", &mid, "moved.mara.md"]);
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let value = serde_json::to_value(graph.resolve(&mid).unwrap().summary()).unwrap();
    assert_eq!(value["reference"], mid);
    assert_eq!(value["id"], "REQ-NEW");
    assert_eq!(value["title"], "Changed");
    assert_eq!(value["source"]["path"], "moved.mara.md");
    assert_eq!(graph.resolve("REQ-NEW").unwrap().reference(), mid);
    assert!(graph.resolve("REQ-ONE").is_err());
    assert!(graph.resolve(&section_handle).is_err());
    assert_eq!(cli(fixture.path(), &["project", "validate"])["valid"], true);
}

#[test]
fn summaries_round_trip_across_process_restarts() {
    const ROOT_ENV: &str = "MARA_HANDLE_RESTART_FIXTURE";
    if let Some(root) = std::env::var_os(ROOT_ENV) {
        let root = Path::new(&root);
        let project = resolve_project(Some(root), root).unwrap();
        let schema = load_schema(&project).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        let expected: Vec<Value> =
            serde_json::from_slice(&fs::read(root.join("expected.json")).unwrap()).unwrap();
        assert_eq!(summaries(&graph), expected);
        for value in expected {
            assert_eq!(
                serde_json::to_value(
                    graph
                        .resolve(value["reference"].as_str().unwrap())
                        .unwrap()
                        .summary()
                )
                .unwrap(),
                value
            );
        }
        return;
    }
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    fs::write(
        fixture.path().join("restart.mara.md"),
        "# Heading\n\n- One\n  - Two\n\n| A | B |\n|---|---|\n| é | 🙂 |\n",
    )
    .unwrap();
    cli(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-ONE",
            "restart.mara.md",
            "--title",
            "One",
            "--body",
            "Body.",
        ],
    );
    let corpus = load_corpus(&project, &schema).unwrap();
    fs::write(
        fixture.path().join("expected.json"),
        serde_json::to_vec(&summaries(&corpus.discovery())).unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "summaries_round_trip_across_process_restarts",
                "--nocapture",
            ])
            .env(ROOT_ENV, fixture.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        cli(fixture.path(), &["project", "validate"])["valid"],
        json!(true)
    );
}
