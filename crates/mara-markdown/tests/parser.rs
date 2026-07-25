use mara_core::{
    DiagnosticCode, IdentityDiagnosticCode, MidFormat, MidIdentity, SchemaField, SourceDocument,
    SourceIndex, SourceText, SyntaxDiagnosticCode,
};
use mara_markdown::{InlineReferenceContext, NarrativeKind, ParsedBlock, parse_document};

const MID: &str = "m_01ARZ3NDEKTSV4RRFFQ69G5FAV";

fn identity() -> MidIdentity {
    let index = SourceIndex::try_new("schema.yaml", "").unwrap();
    let span = index.try_span(0, 0, 1, 1, 1, 1).unwrap();
    MidIdentity::new(
        SchemaField::new(span.clone(), span.clone(), MidFormat::Ulid),
        SchemaField::new(span.clone(), span, "m_".to_owned()),
    )
}

fn parse(source: &str) -> mara_markdown::ParsedDocument {
    let document =
        SourceDocument::try_new("docs/example.mara.md", SourceText::new(source.to_owned()))
            .unwrap();
    parse_document(document, &identity())
}

fn slice<'a>(source: &'a str, span: &mara_core::SourceSpan) -> &'a str {
    &source[span.start_byte() as usize..span.end_byte() as usize]
}

#[test]
fn valid_items_are_lossless_and_keep_structural_header_values_out_of_metadata() {
    let source = format!(
        "# Intro\n\n:::req {MID}\n:id:  REQ-1 \n:verifies: TEST-1\n:verifies: TEST-2\n\nBody **markdown**.\n:status: remains body text\n:::\n\nTail.\n"
    );
    let parsed = parse(&source);

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.blocks().len(), 4);
    assert_eq!(
        parsed.blocks()[0].as_markdown().unwrap().raw(),
        "# Intro\n\n"
    );
    assert_eq!(
        parsed.blocks()[2..]
            .iter()
            .filter_map(ParsedBlock::as_markdown)
            .map(|markdown| markdown.raw())
            .collect::<String>(),
        "\n\nTail.\n"
    );

    let item = parsed.items().next().unwrap();
    assert_eq!(item.flavour(), "req");
    assert_eq!(item.mid().as_str(), MID);
    assert_eq!(
        item.metadata()
            .iter()
            .map(|entry| entry.key())
            .collect::<Vec<_>>(),
        ["id", "verifies", "verifies"]
    );
    assert_eq!(item.metadata()[0].raw_value(), "  REQ-1 ");
    assert_eq!(item.metadata()[0].value(), "REQ-1");
    assert_eq!(item.metadata()[1].value(), "TEST-1");
    assert_eq!(item.metadata()[2].value(), "TEST-2");
    assert_eq!(
        slice(&source, item.metadata()[1].source()),
        ":verifies: TEST-1"
    );
    assert_eq!(
        item.body_markdown(),
        "Body **markdown**.\n:status: remains body text\n"
    );
    assert_eq!(slice(&source, item.body_source()), item.body_markdown());
    assert_eq!(
        slice(&source, item.header_source()),
        format!(":::req {MID}")
    );
    assert_eq!(
        slice(&source, item.source()),
        format!(
            ":::req {MID}\n:id:  REQ-1 \n:verifies: TEST-1\n:verifies: TEST-2\n\nBody **markdown**.\n:status: remains body text\n:::"
        )
    );
}

#[test]
fn malformed_headers_metadata_and_boundaries_have_stable_codes() {
    let malformed_header = parse(&format!(":::req {MID} trailing\n\n:::\n"));
    assert_eq!(malformed_header.items().count(), 0);
    assert_eq!(
        malformed_header.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidItemHeader)
    );

    let malformed_metadata = parse(&format!(":::req {MID}\n:Bad: value\n\n:::\n"));
    assert_eq!(malformed_metadata.items().count(), 0);
    assert_eq!(
        malformed_metadata.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidMetadata)
    );
    assert_eq!(
        slice(
            malformed_metadata.source().source().as_str(),
            malformed_metadata.diagnostics()[0].primary().unwrap()
        ),
        ":Bad: value"
    );

    let missing_boundary = parse(&format!(":::req {MID}\n:id: REQ-1\n:::\n"));
    assert_eq!(missing_boundary.items().count(), 0);
    assert_eq!(
        missing_boundary.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidMetadata)
    );

    let unclosed = parse(&format!(":::req {MID}\n\nbody\n"));
    assert_eq!(unclosed.items().count(), 0);
    assert_eq!(
        unclosed.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::UnclosedItem)
    );
}

#[test]
fn malformed_mid_tokens_keep_the_identity_diagnostic_category() {
    for mid in [
        "x_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "m_01ARZ3NDEKTSV4RRFFQ69G5FAI",
        "m_81ARZ3NDEKTSV4RRFFQ69G5FAV",
    ] {
        let source = format!(":::req {mid}\n\n:::\n");
        let parsed = parse(&source);

        assert_eq!(parsed.items().count(), 0);
        assert_eq!(parsed.diagnostics().len(), 1);
        assert_eq!(
            parsed.diagnostics()[0].code(),
            DiagnosticCode::Identity(IdentityDiagnosticCode::InvalidMid),
            "{mid}"
        );
    }
}

#[test]
fn nested_real_items_are_diagnosed_and_never_extracted() {
    let source = format!(":::req {MID}\n\nouter\n:::test {MID}\ninner\n:::\n:::\n");
    let parsed = parse(&source);

    assert_eq!(parsed.items().count(), 1);
    assert_eq!(parsed.items().next().unwrap().flavour(), "req");
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidItemHeader)
    );
    assert_eq!(
        slice(&source, parsed.diagnostics()[0].primary().unwrap()),
        format!(":::test {MID}")
    );
}

#[test]
fn diagnostics_follow_canonical_source_order_across_parse_stages() {
    let source =
        format!(":::req {MID}\n:Bad: malformed first\n\nbody\n:::test {MID}\nnested\n:::\n:::\n");
    let parsed = parse(&source);

    assert_eq!(parsed.items().count(), 0);
    assert_eq!(parsed.diagnostics().len(), 2);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidMetadata)
    );
    assert_eq!(
        slice(&source, parsed.diagnostics()[0].primary().unwrap()),
        ":Bad: malformed first"
    );
    assert_eq!(
        parsed.diagnostics()[1].code(),
        DiagnosticCode::Syntax(SyntaxDiagnosticCode::InvalidItemHeader)
    );
    assert_eq!(
        slice(&source, parsed.diagnostics()[1].primary().unwrap()),
        format!(":::test {MID}")
    );
}

#[test]
fn ordinary_markdown_and_mara_like_code_are_preserved_intact() {
    let source = format!(
        "# Ordinary\n\n:::note\nnot a Mara item\n:::\n\n:::warning Caution\nstill not a Mara item\n:::\n\n::: note\nnot a Mara item\n:::\n\n```markdown\n:::req {MID}\n\n:::\n```\n\n<pre>\n:::req {MID}\n:::\n</pre>\n"
    );
    let parsed = parse(&source);

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.items().count(), 0);
    assert_eq!(
        parsed
            .blocks()
            .iter()
            .filter_map(ParsedBlock::as_markdown)
            .map(|markdown| markdown.raw())
            .collect::<String>(),
        source
    );
}

#[test]
fn exact_closers_inside_fenced_code_do_not_close_the_item() {
    let source = format!(":::req {MID}\n\n```markdown\n:::\n```\n\\:::\n:::\n");
    let parsed = parse(&source);
    let item = parsed.items().next().unwrap();

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(item.body_markdown(), "```markdown\n:::\n```\n\\:::\n");
    assert!(matches!(parsed.blocks()[0], ParsedBlock::Item(_)));
}

#[test]
fn crlf_source_spans_and_empty_bodies_remain_exact() {
    let source = format!("before\r\n:::req {MID}\r\n:id:\tX \r\n\r\n:::\r\nafter\r\n");
    let parsed = parse(&source);
    let item = parsed.items().next().unwrap();

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(item.body_markdown(), "");
    assert!(item.body_source().is_empty());
    assert_eq!(item.body_source().start_line(), 5);
    assert_eq!(item.metadata()[0].raw_value(), "\tX ");
    assert_eq!(item.metadata()[0].value(), "X");
    assert_eq!(slice(&source, item.metadata()[0].source()), ":id:\tX ");
}

#[test]
fn mixed_document_hierarchy_and_inline_contexts_remain_lossless() {
    let source = format!(
        "Preamble [[PRE]].\n\n# Top [[TOP|label]]\n\nParagraph [[TEXT]] and `[[CODE]]`.\n\n- list [[LIST]]\n\n| value |\n| --- |\n| [[CELL]] |\n\n```text\n[[FENCE]]\n```\n\n<div>[[HTML]]</div>\n\n:::req {MID}\n\nBody [[ITEM]] and \\[[ESCAPED]].\n:::\n\n## Child\n\nChild text.\n"
    );
    let parsed = parse(&source);

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(parsed.sections().len(), 1);
    let top = &parsed.sections()[0];
    assert_eq!(top.level(), 1);
    assert_eq!(top.title(), "Top [[TOP|label]]");
    assert_eq!(top.children().len(), 1);
    assert_eq!(top.children()[0].level(), 2);
    assert_eq!(top.children()[0].title(), "Child");
    assert_eq!(
        slice(&source, top.source()),
        &source[top.source().start_byte() as usize..]
    );

    let reconstructed = parsed
        .blocks()
        .iter()
        .map(|block| match block {
            ParsedBlock::Markdown(markdown) => markdown.raw(),
            ParsedBlock::Item(item) => slice(&source, item.source()),
        })
        .collect::<String>();
    assert_eq!(reconstructed, source);
    assert!(
        parsed
            .blocks()
            .iter()
            .filter_map(ParsedBlock::as_markdown)
            .any(|block| block.kind() == NarrativeKind::Table)
    );

    let narrative_references = parsed
        .blocks()
        .iter()
        .filter_map(ParsedBlock::as_markdown)
        .flat_map(|block| block.references())
        .map(|reference| (reference.target(), reference.label(), reference.context()))
        .collect::<Vec<_>>();
    assert_eq!(
        narrative_references,
        [
            ("PRE", None, InlineReferenceContext::Text),
            ("TOP", Some("label"), InlineReferenceContext::Heading),
            ("TEXT", None, InlineReferenceContext::Text),
            ("LIST", None, InlineReferenceContext::ListItem),
            ("CELL", None, InlineReferenceContext::TableCell),
        ]
    );
    let item = parsed.items().next().unwrap();
    assert_eq!(item.references().len(), 1);
    assert_eq!(item.references()[0].target(), "ITEM");
    assert_eq!(item.references()[0].context(), InlineReferenceContext::Text);
    assert_eq!(slice(&source, item.references()[0].source()), "[[ITEM]]");
    assert_eq!(slice(&source, top.heading_source()), "# Top [[TOP|label]]");
}

#[test]
fn section_ranges_distinguish_preamble_direct_content_children_and_siblings() {
    let source = format!(
        "Preamble.\n\n:::req {MID}\n\nItem body.\n:::\n\n# One\n\nDirect one.\n\n## Child\n\nChild body.\n\n# Two\n\nDirect two.\n"
    );
    let parsed = parse(&source);

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .preamble()
            .iter()
            .filter_map(ParsedBlock::as_item)
            .count(),
        1
    );
    assert_eq!(parsed.sections().len(), 2);
    let one = &parsed.sections()[0];
    let two = &parsed.sections()[1];
    assert_eq!(slice(&source, one.heading_source()), "# One");
    assert_eq!(slice(&source, two.heading_source()), "# Two");
    assert_eq!(
        parsed.blocks()[one.content_range()]
            .iter()
            .filter_map(ParsedBlock::as_markdown)
            .map(|block| block.raw())
            .collect::<String>(),
        "Direct one.\n\n"
    );
    assert_eq!(one.children().len(), 1);
    assert_eq!(
        slice(&source, one.children()[0].heading_source()),
        "## Child"
    );
    assert_eq!(
        slice(&source, one.source()),
        "# One\n\nDirect one.\n\n## Child\n\nChild body.\n\n"
    );
    assert_eq!(slice(&source, two.source()), "# Two\n\nDirect two.\n");
}

#[test]
fn inline_reference_spans_follow_unicode_crlf_and_escape_parity() {
    let source = "é [[ONE]]\r\n\\\\[[TWO|label]] and \\[[ESCAPED]]\r\n<span data-ref=\"[[ATTR]]\"></span>\r\n";
    let parsed = parse(source);
    let references = parsed
        .blocks()
        .iter()
        .filter_map(ParsedBlock::as_markdown)
        .flat_map(|block| block.references())
        .collect::<Vec<_>>();

    assert!(parsed.is_valid(), "{:?}", parsed.diagnostics());
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].target(), "ONE");
    assert_eq!(slice(source, references[0].source()), "[[ONE]]");
    assert_eq!(references[0].source().start_byte(), 3);
    assert_eq!(references[0].source().start_line(), 1);
    assert_eq!(references[0].source().start_column(), 3);
    assert_eq!(references[1].target(), "TWO");
    assert_eq!(references[1].label(), Some("label"));
    assert_eq!(slice(source, references[1].source()), "[[TWO|label]]");
    assert_eq!(references[1].source().start_line(), 2);
}
