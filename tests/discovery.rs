use std::{fs, path::Path, process::Command};

use mara::{
    ConnectionKind as Connection, DiscoveryGraph, DiscoveryNode, DiscoveryNodeKind as Node,
    MarkdownBlockKind as Block, RelationDirection as Direction, Template, initialize_project,
    load_corpus, load_schema,
};
use tempfile::TempDir;

fn text<'a>(node: DiscoveryNode<'_, '_>, source: &'a str) -> &'a str {
    let span = node.source().span();
    &source[span.start_byte()..span.end_byte()]
}

fn section<'g, 'c>(graph: &'g DiscoveryGraph<'c>, title: &str) -> DiscoveryNode<'g, 'c> {
    graph
        .nodes()
        .find(|node| {
            matches!(node.kind(), Node::Section { heading }
        if heading.heading_text() == Some(title))
        })
        .unwrap()
}

fn item<'g, 'c>(graph: &'g DiscoveryGraph<'c>, id: &str) -> DiscoveryNode<'g, 'c> {
    graph
        .nodes()
        .find(|node| matches!(node.kind(), Node::Item(item) if item.id() == id))
        .unwrap()
}

#[test]
fn derives_scoped_sections_and_navigates_interleaved_content_in_source_order() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let lf = "Prelude.\n\n### Early\n\nEarly body.\n\n# Outer\n\nBefore.\n:::mara requirement REQ-ONE\n:title: One\n\nItem prelude.\n\n# Local\n\nLocal body.\n\n#### Deep\n\nDeep body.\n:::\nAfter.\n\n### Skipped\n\nNested.\n\n## Peer\n\nPeer body.\n\n# End\n\nFinal.\n";
    for source in [lf.to_owned(), lf.replace('\n', "\r\n")] {
        fs::write(fixture.path().join("structure.mara.md"), &source).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        let root = graph.nodes().next().unwrap();
        assert!(matches!(root.kind(), Node::Document(_)));
        assert!(root.parent().is_none());
        let children = root.children();
        assert_eq!(children.len(), 4);
        assert_eq!(text(children[0], &source).trim(), "Prelude.");
        assert_eq!(
            text(children[1], &source),
            &source[source.find("### Early").unwrap()..source.find("# Outer").unwrap()]
        );
        let outer = section(&graph, "Outer");
        assert_eq!(outer.parent().unwrap().source(), root.source());
        assert_eq!(
            text(outer, &source),
            &source[source.find("# Outer").unwrap()..source.find("# End").unwrap()]
        );
        let siblings = outer.children();
        assert_eq!(siblings.len(), 5); // Before, item, after, skipped, peer; no duplicate heading node.
        assert_eq!(text(siblings[0], &source).trim(), "Before.");
        assert!(matches!(siblings[1].kind(), Node::Item(_)));
        assert_eq!(text(siblings[2], &source).trim(), "After.");
        assert_eq!(
            section(&graph, "Skipped").parent().unwrap().source(),
            outer.source()
        );
        assert_eq!(
            section(&graph, "Peer").parent().unwrap().source(),
            outer.source()
        );
        let one = item(&graph, "REQ-ONE");
        assert_eq!(one.parent().unwrap().source(), outer.source());
        let local = section(&graph, "Local");
        assert_eq!(local.parent().unwrap().source(), one.source());
        let deep = section(&graph, "Deep");
        assert_eq!(deep.parent().unwrap().source(), local.source());
        assert_eq!(
            text(deep, &source).trim(),
            "#### Deep\n\nDeep body."
                .replace('\n', if source.contains('\r') { "\r\n" } else { "\n" })
        );
        assert_eq!(
            one.parent().unwrap().children()[2].source(),
            siblings[2].source()
        );
        assert_eq!(outer.source().span().start_line(), 7);
        assert_eq!(outer.source().span().end_line(), 32);
        assert_eq!(
            fs::read_to_string(fixture.path().join("structure.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn preserves_ordinary_containers_and_their_local_heading_scopes() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "# Outside\n\n> Intro.\n>\n> ### Quoted\n>\n> - First\n>   - Nested\n>\n> # Quote peer\n>\n> Last.\n\n- ### Listed\n\n  List body.\n\n  | A | B |\n  |---|---|\n  | é | x |\n\nAfter containers.\n\n```md\n# Example only\n```\n".replace('\n', "\r\n");
    fs::write(fixture.path().join("containers.mara.md"), &source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let outside = section(&graph, "Outside");
    let children = outside.children();
    assert_eq!(children.len(), 4);
    assert!(
        matches!(children[0].kind(), Node::MarkdownBlock(block) if block.kind() == Block::Blockquote)
    );
    assert!(
        matches!(children[1].kind(), Node::MarkdownBlock(block) if block.kind() == Block::List)
    );
    assert!(
        matches!(children[3].kind(), Node::MarkdownBlock(block) if block.kind() == Block::CodeBlock)
    );
    let quoted = section(&graph, "Quoted");
    let quote_peer = section(&graph, "Quote peer");
    assert_eq!(quoted.parent().unwrap().source(), children[0].source());
    assert_eq!(quote_peer.parent().unwrap().source(), children[0].source());
    assert_eq!(
        quoted.source().span().end_byte(),
        quote_peer.source().span().start_byte()
    );
    let listed = section(&graph, "Listed");
    assert!(
        matches!(listed.parent().unwrap().kind(), Node::MarkdownBlock(block) if block.kind() == Block::ListItem)
    );
    let table = listed.children()[1];
    assert!(matches!(table.kind(), Node::MarkdownBlock(block) if block.kind() == Block::Table));
    assert_eq!(table.children().len(), 2);
    assert!(
        matches!(table.children()[1].children()[0].children()[0].kind(), Node::MarkdownBlock(block) if block.kind() == Block::TableCell)
    );
    assert_eq!(
        text(table.children()[1].children()[0].children()[0], &source),
        "é"
    );
    for node in graph.nodes() {
        assert!(
            source
                .get(node.source().span().start_byte()..node.source().span().end_byte())
                .is_some()
        );
        if let Some(parent) = node.parent() {
            assert!(node.source().span().start_byte() >= parent.source().span().start_byte());
            assert!(node.source().span().end_byte() <= parent.source().span().end_byte());
            let incoming = node.connections(Direction::Incoming);
            let inverse = incoming
                .iter()
                .filter(|edge| edge.kind == Connection::ContainedBy)
                .collect::<Vec<_>>();
            assert_eq!(inverse.len(), 1);
            assert_eq!(inverse[0].source, node.source());
        }
    }
}

#[test]
fn heading_free_documents_and_empty_documents_have_direct_content() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    fs::write(fixture.path().join("a.mara.md"), "").unwrap();
    fs::write(
        fixture.path().join("b.mara.md"),
        "Plain.\n\n- List\n\n:::mara requirement REQ-ONE\n:title: One\n\nBody.\n:::\n",
    )
    .unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let documents = graph
        .nodes()
        .filter(|node| matches!(node.kind(), Node::Document(_)))
        .collect::<Vec<_>>();
    assert_eq!(documents.len(), 2);
    assert!(documents[0].children().is_empty());
    assert_eq!(documents[0].source().span().start_byte(), 0);
    assert_eq!(documents[0].source().span().end_byte(), 0);
    assert_eq!(documents[1].children().len(), 3);
    assert_eq!(
        item(&graph, "REQ-ONE").parent().unwrap().source(),
        documents[1].source()
    );
    assert!(
        !graph
            .nodes()
            .any(|node| matches!(node.kind(), Node::Section { .. }))
    );
}

#[test]
fn distinguishes_containment_mentions_and_schema_names_between_direct_neighbours() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema_path = fixture.path().join(".mara/schema.yaml");
    let mut schema_source = fs::read_to_string(&schema_path).unwrap();
    schema_source.push_str("  contains:\n    description: Authored semantic relation.\n    source: [requirement]\n    target: [requirement]\n  mentions:\n    description: Another authored relation.\n    source: [requirement]\n    target: [requirement]\n");
    fs::write(schema_path, schema_source).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "# Context\n\n:::mara requirement REQ-ONE\n:title: One\n:contains: REQ-TWO\n:mentions: REQ-TWO\n\n[[REQ-TWO]]\n:::\n\n:::mara requirement REQ-TWO\n:title: Two\n:depends_on: REQ-THREE\n\nTwo.\n:::\n\n:::mara requirement REQ-THREE\n:title: Three\n\nThree.\n:::\n";
    fs::write(fixture.path().join("relations.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let one = item(&graph, "REQ-ONE");
    let two = item(&graph, "REQ-TWO");
    let outgoing = one.connections(Direction::Outgoing);
    let kinds = outgoing.iter().map(|edge| edge.kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            Connection::Contains,
            Connection::Schema("contains"),
            Connection::Schema("mentions"),
            Connection::Mentions
        ]
    );
    assert!(
        !outgoing.iter().any(
            |edge| matches!(edge.neighbour.kind(), Node::Item(item) if item.id() == "REQ-THREE")
        )
    );
    let incoming = two.connections(Direction::Incoming);
    assert_eq!(incoming.len(), 4);
    for edge in outgoing
        .iter()
        .filter(|edge| edge.kind != Connection::Contains)
    {
        assert_eq!(edge.neighbour.source(), two.source());
        assert!(incoming.iter().any(|backlink| backlink.kind == edge.kind
            && backlink.source == edge.source
            && backlink.neighbour.source() == one.source()));
    }
    assert_eq!(
        one.parent().unwrap().source(),
        two.parent().unwrap().source()
    );
    assert_eq!(one.children().len(), 1); // A schema relation named contains is not structural.
    assert!(
        two.connections(Direction::Outgoing).iter().any(
            |edge| matches!(edge.neighbour.kind(), Node::Item(item) if item.id() == "REQ-THREE")
        )
    );
}

#[test]
fn reloads_real_cli_authored_and_updated_items_into_the_graph() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let path = fixture.path().join("model.mara.md");
    fs::write(&path, "# Model\n\nNarrative.\n").unwrap();
    let invoke = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .current_dir(fixture.path())
            .args(["--format", "json"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    invoke(&[
        "item",
        "create",
        "requirement",
        "REQ-REAL",
        "model.mara.md",
        "--title",
        "Real",
        "--body",
        "# First\n\nInitial body.",
    ]);
    let first = load_corpus(&project, &schema).unwrap();
    let mid = first.items().next().unwrap().mid().unwrap().to_owned();
    let graph = first.discovery();
    assert_eq!(
        section(&graph, "First").parent().unwrap().source(),
        item(&graph, "REQ-REAL").source()
    );
    invoke(&[
        "item",
        "update",
        "REQ-REAL",
        "--body",
        "Prelude.\n\n### Updated\n\n- Child",
    ]);
    invoke(&["project", "validate"]);
    let second = load_corpus(&project, &schema).unwrap();
    assert_eq!(second.items().next().unwrap().mid(), Some(mid.as_str()));
    let graph = second.discovery();
    let real = item(&graph, "REQ-REAL");
    assert_eq!(
        real.parent().unwrap().source(),
        section(&graph, "Model").source()
    );
    assert_eq!(real.children().len(), 2);
    assert_eq!(
        section(&graph, "Updated").parent().unwrap().source(),
        real.source()
    );
    assert!(!graph.nodes().any(|node| matches!(node.kind(), Node::Section { heading } if heading.heading_text() == Some("First"))));
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        second.documents()[0].source()
    );
}

#[test]
fn projects_repository_structure_deterministically_without_writing_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = mara::resolve_project(Some(root), root).unwrap();
    let schema = load_schema(&project).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    let snapshot = |graph: &DiscoveryGraph<'_>| {
        graph
            .nodes()
            .map(|node| {
                (
                    format!("{:?}", std::mem::discriminant(&node.kind())),
                    node.source().clone(),
                    node.parent().map(|parent| parent.source().clone()),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        snapshot(&graph),
        snapshot(&load_corpus(&project, &schema).unwrap().discovery())
    );
    for node in graph.nodes() {
        let children = node.children();
        for child in &children {
            assert_eq!(child.parent().unwrap().source(), node.source());
            assert!(child.source().span().start_byte() >= node.source().span().start_byte());
            assert!(child.source().span().end_byte() <= node.source().span().end_byte());
        }
        assert!(
            children
                .windows(2)
                .all(|pair| pair[0].source().span().end_byte()
                    <= pair[1].source().span().start_byte())
        );
    }
    for document in corpus.documents() {
        assert_eq!(
            fs::read_to_string(root.join(document.path())).unwrap(),
            document.source()
        );
    }
}

#[test]
fn retains_heading_text_and_exact_heading_spans_in_document_context() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let lf = "# **Café** and `code` [guide][ref]\n\n:::mara requirement REQ-ONE\n:title: One\n\nBody.\n:::\n\n[ref]: https://example.com\n\n多行\nHeading\n=======\n\n## <https://example.com>\n";
    for source in [lf.to_owned(), lf.replace('\n', "\r\n")] {
        fs::write(fixture.path().join("heading.mara.md"), &source).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        let titles = graph
            .nodes()
            .filter_map(|node| match node.kind() {
                Node::Section { heading } => Some(heading.heading_text().unwrap()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            titles,
            ["Café and code guide", "多行 Heading", "https://example.com"]
        );
        let multiline = section(&graph, "多行 Heading");
        let Node::Section { heading } = multiline.kind() else {
            unreachable!()
        };
        let span = heading.source().span();
        assert_eq!(
            &source[span.start_byte()..span.end_byte()],
            if source.contains('\r') {
                "多行\r\nHeading\r\n=======\r\n"
            } else {
                "多行\nHeading\n=======\n"
            }
        );
        assert_eq!(span.start_line(), 11);
        assert_eq!(span.end_line(), 13);
    }
}
