use mara::{
    ConnectionKind, DiscoveryNodeKind as Node, MarkdownBlockKind as Block, ReferenceKind,
    RelationDirection as Direction, Template, initialize_project, load_corpus, load_schema,
};
use std::{fs, process::Command};
use tempfile::TempDir;

const MID: &str = "01M1PXP2KG381MM1VNN6XC7S4M";

fn item(body: &str) -> String {
    format!(":::mara requirement REQ-ONE\n:mid: {MID}\n:title: One\n\n{body}\n:::\n")
}

#[test]
fn resolves_document_sections_and_items_with_precise_backlinks() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    fs::create_dir(fixture.path().join("docs")).unwrap();
    let target = format!(
        "# Retry policy\n\nBefore.\n\n{}\n# Retry policy\n\nAfter.\n",
        item("## Retry policy\n\nItem details.")
    );
    let source = format!(
        "# Source\n\n[document](../target.mara.md) [outer](.././target.mara.md#retry-policy) [inside][policy] [last](../target.mara.md#retry-policy-2) [[REQ-ONE]] [[{MID}]]\n\n[policy]: ../target.mara.md#retry-policy-1\n"
    );
    for newline in ["\n", "\r\n"] {
        fs::write(
            fixture.path().join("target.mara.md"),
            target.replace('\n', newline),
        )
        .unwrap();
        let source = source.replace('\n', newline);
        fs::write(fixture.path().join("docs/source.mara.md"), &source).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let graph = corpus.discovery();
        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let paragraph = graph
            .nodes()
            .find(|node| {
                node.source().path().to_str() == Some("docs/source.mara.md")
                    && matches!(node.kind(), Node::MarkdownBlock(b) if b.kind() == Block::Paragraph)
            })
            .unwrap();
        let links = paragraph
            .connections(Direction::Outgoing)
            .into_iter()
            .filter(|c| c.kind == ConnectionKind::Mentions)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 6);
        assert_eq!(
            links
                .iter()
                .filter(|c| matches!(c.neighbour.kind(), Node::Document(_)))
                .count(),
            1
        );
        assert_eq!(
            links
                .iter()
                .filter(|c| matches!(c.neighbour.kind(), Node::Item(_)))
                .count(),
            2
        );
        let inside = links
            .iter()
            .find(|c| {
                matches!(c.neighbour.kind(), Node::Section { .. })
                    && matches!(c.neighbour.parent().unwrap().kind(), Node::Item(_))
            })
            .unwrap();
        assert_eq!(
            &source[inside.source.span().start_byte()..inside.source.span().end_byte()],
            "[inside][policy]"
        );
        let backlink = inside
            .neighbour
            .connections(Direction::Incoming)
            .into_iter()
            .find(|c| c.kind == ConnectionKind::Mentions)
            .unwrap();
        assert_eq!(backlink.source, inside.source);
        assert_eq!(backlink.neighbour.source(), paragraph.source());
        let target_source = corpus
            .documents()
            .iter()
            .find(|d| d.path().to_str() == Some("target.mara.md"))
            .unwrap()
            .source();
        let span = inside.neighbour.source().span();
        assert_eq!(
            &target_source[span.start_byte()..span.end_byte()],
            format!("## Retry policy{newline}{newline}Item details.{newline}")
        );
    }
}

#[test]
fn explicit_anchors_keep_block_targets_inside_items_and_respect_boundaries() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = format!(
        r#"[heading](#stable-heading) [paragraph](#stable-paragraph) [inline](#inline) [list](#list) [boundary](#boundary) [tail](#tail)

<a name="stable-heading"></a>

# Heading

<a name='stable-paragraph'></a>

Target paragraph.

<a name="boundary"></a>

{}
After item.
"#,
        item(
            r#"## Local

An <a name="inline"></a> inline anchor.

- First
  - <a name="list"></a> nested content.

<a name="tail"></a>"#
        )
    );
    fs::write(fixture.path().join("anchors.mara.md"), &source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let paragraph = graph
        .nodes()
        .find(|node| {
            node.source().span().start_byte() == 0 && matches!(node.kind(), Node::MarkdownBlock(_))
        })
        .unwrap();
    let links = paragraph
        .connections(Direction::Outgoing)
        .into_iter()
        .filter(|c| c.kind == ConnectionKind::Mentions)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 6);
    for link in links {
        let evidence = &source[link.source.span().start_byte()..link.source.span().end_byte()];
        let target = link.neighbour;
        let text = &source[target.source().span().start_byte()..target.source().span().end_byte()];
        match evidence {
            "[heading](#stable-heading)" => assert!(matches!(target.kind(), Node::Section { .. })),
            "[paragraph](#stable-paragraph)" => assert_eq!(text.trim(), "Target paragraph."),
            "[inline](#inline)" => assert!(text.starts_with("An <a")),
            "[list](#list)" => {
                assert!(matches!(target.kind(), Node::MarkdownBlock(b) if b.kind() == Block::List))
            }
            "[boundary](#boundary)" => assert_eq!(text.trim(), "<a name=\"boundary\"></a>"),
            "[tail](#tail)" => assert_eq!(text.trim(), "<a name=\"tail\"></a>"),
            _ => panic!("unexpected evidence {evidence:?}"),
        }
    }
    let anchors = corpus.documents()[0]
        .references()
        .iter()
        .filter(|r| r.kind() == ReferenceKind::Anchor)
        .collect::<Vec<_>>();
    assert_eq!(anchors.len(), 6);
    for anchor in anchors {
        assert!(
            source[anchor.source().span().start_byte()..anchor.source().span().end_byte()]
                .starts_with("<a name=")
        );
    }
}

#[test]
fn generates_document_wide_github_anchors_and_decodes_fragment_urls_once() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "[one](#déjà--vu-code_x--ok) [two](#same-1) [three](#same-1-1) [four](#same-2) [encoded](#d%C3%A9j%C3%A0--vu-code_x--ok)\n\n# Déjà  *vu* `code_x` &amp; ok!\n\n# Same\n\n# Same-1\n\n# Same-1\n\n# Same\n";
    fs::write(fixture.path().join("unicode.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let links = graph
        .nodes()
        .flat_map(|n| n.connections(Direction::Outgoing))
        .filter(|c| c.kind == ConnectionKind::Mentions)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 5);
    assert!(
        links
            .iter()
            .all(|c| matches!(c.neighbour.kind(), Node::Section { .. }))
    );
}

#[test]
fn validation_reports_broken_and_ambiguous_internal_links_without_edges_or_network_reads() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "[missing](absent.mara.md) [anchor](#absent) [[REQ-ABSENT]] [ambiguous](#repeat) [external](https://invalid.invalid/missing.mara.md#absent) [code](src/lib.rs#missing)\n\n# Repeat\n\n<a name=\"repeat\"></a>\n\nBody.\n";
    fs::write(fixture.path().join("broken.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert_eq!(graph.diagnostics().len(), 6, "{:?}", graph.diagnostics());
    assert!(!graph.nodes().any(|n| {
        n.connections(Direction::Outgoing)
            .iter()
            .any(|c| c.kind == ConnectionKind::Mentions)
    }));
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args([
            "--project",
            fixture.path().to_str().unwrap(),
            "--format",
            "json",
            "project",
            "validate",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["valid"], false);
    assert_eq!(
        fs::read_to_string(fixture.path().join("broken.mara.md")).unwrap(),
        source
    );
}

#[test]
fn code_raw_context_and_escaped_references_stay_inert_and_definitions_span_items() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = format!(
        r#"[before][target]

`[code](#missing) [[REQ-NO]] <a name="no"></a>`

\[escaped](#missing)

```md
[code](#missing) [[REQ-NO]]
<a name="no"></a>
```

<!-- [hidden](#missing) [[REQ-NO]] <a name="no"></a> -->

{}
[after][target]
"#,
        item("[inside][target]\n\n## Destination\n\n[target]: #destination")
    );
    fs::write(fixture.path().join("context.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let links = graph
        .nodes()
        .flat_map(|n| n.connections(Direction::Outgoing))
        .filter(|c| c.kind == ConnectionKind::Mentions)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 3);
    assert!(
        links
            .iter()
            .all(|c| matches!(c.neighbour.kind(), Node::Section { .. }))
    );
    assert_eq!(corpus.documents()[0].references().len(), 3);
}

#[test]
fn duplicate_explicit_anchors_are_ambiguous_without_changing_generated_numbering() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "[bad](#repeat) [good](#repeat-1)\n\n<a name=\"repeat\"></a>\n<a name=\"repeat\"></a>\n\n# Repeat\n\n# Repeat\n";
    fs::write(fixture.path().join("duplicates.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert_eq!(graph.diagnostics().len(), 4, "{:?}", graph.diagnostics());
    let links = graph
        .nodes()
        .flat_map(|n| n.connections(Direction::Outgoing))
        .filter(|c| c.kind == ConnectionKind::Mentions)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].neighbour.source().span().start_byte(),
        source.rfind("# Repeat").unwrap()
    );
}

#[test]
fn narrative_mentions_now_prevent_delete_and_rename_from_leaving_broken_references() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let source = item("Body.");
    let narrative = "Narrative [[REQ-ONE]].\n";
    fs::write(fixture.path().join("item.mara.md"), &source).unwrap();
    fs::write(fixture.path().join("narrative.mara.md"), narrative).unwrap();
    assert!(
        load_corpus(&project, &load_schema(&project).unwrap())
            .unwrap()
            .discovery()
            .diagnostics()
            .is_empty()
    );
    // Full reference-aware mutation handling is MARA-53. The existing candidate
    // validation must already reject a broken narrative reference without writes.
    for args in [
        vec!["item", "delete", "REQ-ONE"],
        vec!["item", "rename", "REQ-ONE", "REQ-TWO"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args([
                "--project",
                fixture.path().to_str().unwrap(),
                "--format",
                "json",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let message = String::from_utf8(output.stdout).unwrap();
        assert!(message.contains("narrative.mara.md:1"), "{message}");
        assert!(message.contains("missing item 'REQ-ONE'"), "{message}");
        assert_eq!(
            fs::read_to_string(fixture.path().join("item.mara.md")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("narrative.mara.md")).unwrap(),
            narrative
        );
    }
}

#[test]
fn standalone_anchor_before_a_nested_heading_targets_the_section() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    let source = "[quoted](#quoted)\n\n> <a name=\"quoted\"></a>\n>\n> ## Nested\n>\n> Body.\n";
    fs::write(fixture.path().join("nested.mara.md"), source).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let graph = corpus.discovery();
    assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    let link = graph
        .nodes()
        .flat_map(|n| n.connections(Direction::Outgoing))
        .find(|c| c.kind == ConnectionKind::Mentions)
        .unwrap();
    assert!(
        matches!(link.neighbour.kind(), Node::Section { heading } if heading.heading_text() == Some("Nested"))
    );
}

#[test]
fn nested_link_labels_resolve_and_missing_destinations_fail_cli_validation() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    for (destination, valid) in [("dest", true), ("missing", false)] {
        let source = format!("[array[index]](#{destination})\n\n# Dest\n");
        fs::write(fixture.path().join("nested-label.mara.md"), &source).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        let reference = &corpus.documents()[0].references();
        assert_eq!(reference.len(), 1);
        assert_eq!(reference[0].target(), format!("#{destination}"));
        assert_eq!(
            &source[reference[0].source().span().start_byte()
                ..reference[0].source().span().end_byte()],
            format!("[array[index]](#{destination})")
        );
        let graph = corpus.discovery();
        let links = graph
            .nodes()
            .flat_map(|n| n.connections(Direction::Outgoing))
            .filter(|c| c.kind == ConnectionKind::Mentions)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), usize::from(valid));
        assert_eq!(graph.diagnostics().is_empty(), valid);
        if valid {
            assert!(
                matches!(links[0].neighbour.kind(), Node::Section { heading } if heading.heading_text() == Some("Dest"))
            );
        }
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args([
                "--project",
                fixture.path().to_str().unwrap(),
                "--format",
                "json",
                "project",
                "validate",
            ])
            .output()
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), valid, "{json}");
        assert_eq!(json["valid"], valid);
    }
}

#[test]
fn item_mentions_take_precedence_over_markdown_reference_definitions() {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    for destination in ["missing", "dest"] {
        let source = format!(
            "[[REQ-ONE]] [[{MID}]]\n\n{}\n# Dest\n\n[REQ-ONE]: #{destination}\n[{MID}]: #{destination}\n",
            item(&format!("[[REQ-ONE]] [[{MID}]]"))
        );
        fs::write(fixture.path().join("mentions.mara.md"), &source).unwrap();
        let corpus = load_corpus(&project, &schema).unwrap();
        assert_eq!(corpus.documents()[0].references().len(), 4);
        assert!(
            corpus.documents()[0]
                .references()
                .iter()
                .all(|r| r.kind() == ReferenceKind::Item)
        );
        let graph = corpus.discovery();
        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let links = graph
            .nodes()
            .flat_map(|n| n.connections(Direction::Outgoing))
            .filter(|c| c.kind == ConnectionKind::Mentions)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 4);
        assert!(
            links
                .iter()
                .all(|c| matches!(c.neighbour.kind(), Node::Item(_)))
        );
        let output = Command::new(env!("CARGO_BIN_EXE_mara"))
            .args([
                "--project",
                fixture.path().to_str().unwrap(),
                "--format",
                "json",
                "project",
                "validate",
            ])
            .output()
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{json}");
        assert_eq!(json["valid"], true);
    }
}
