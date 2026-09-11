use std::{fs, path::Path};

use mara::{Template, initialize_project, load_corpus, load_corpus_for_validation, load_schema};
use tempfile::TempDir;

fn initialized_project() -> (TempDir, mara::Project, mara::Schema) {
    let fixture = TempDir::new().unwrap();
    let project = initialize_project(fixture.path(), Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    (fixture, project, schema)
}

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

#[test]
fn discovers_only_configured_mara_documents_in_stable_path_order() {
    let (fixture, _project, schema) = initialized_project();
    let project_file = fixture.path().join(".mara/project.toml");
    let source = fs::read_to_string(&project_file).unwrap();
    fs::write(
        &project_file,
        source.replace("**/*.mara.md", "docs/*.mara.md"),
    )
    .unwrap();

    write(fixture.path(), "outside.mara.md", "Outside.\n");
    write(fixture.path(), "docs/z.mara.md", "Zed.\n");
    write(fixture.path(), "docs/a.mara.md", "Alpha.\n");
    write(fixture.path(), "docs/nested/skipped.mara.md", "Nested.\n");
    write(fixture.path(), "docs/not-mara.md", "Markdown.\n");

    let project = mara::resolve_project(Some(fixture.path()), fixture.path()).unwrap();
    let corpus = load_corpus(&project, &schema).unwrap();
    let paths = corpus
        .documents()
        .iter()
        .map(|document| document.path().to_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(paths, ["docs/a.mara.md", "docs/z.mara.md"]);
    assert_eq!(corpus.documents()[0].source(), "Alpha.\n");
    assert_eq!(project.content_patterns(), ["docs/*.mara.md"]);
}

#[test]
fn retains_item_content_references_and_precise_source_locations() {
    let (fixture, project, schema) = initialized_project();
    let source = r#"# Context

```markdown
:::mara requirement REQ-EXAMPLE
:title: Example only

[[REQ-NOT-DATA]]
:::
```

:::mara requirement REQ-PARSE
:title: Parse canonical documents
:tag: first
:depends_on: REQ-SOURCE
:tag: second

Body with [[REQ-SOURCE]] and `[[REQ-INLINE-CODE]]`.
:depends_on: REQ-BODY-TEXT

~~~text
[[REQ-FENCED-CODE]]
:::
~~~
:::
"#;
    write(fixture.path(), "docs/model.mara.md", source);

    let corpus = load_corpus(&project, &schema).unwrap();
    let item = corpus.items().next().unwrap();

    assert_eq!(corpus.items().count(), 1);
    assert_eq!(item.flavour(), "requirement");
    assert_eq!(item.id(), "REQ-PARSE");
    assert_eq!(item.title(), "Parse canonical documents");
    assert_eq!(
        item.metadata()
            .iter()
            .map(|entry| (entry.key(), entry.value()))
            .collect::<Vec<_>>(),
        [
            ("title", "Parse canonical documents"),
            ("tag", "first"),
            ("depends_on", "REQ-SOURCE"),
            ("tag", "second"),
        ]
    );
    assert!(item.body().starts_with("Body with [[REQ-SOURCE]]"));
    assert!(item.body().contains(":depends_on: REQ-BODY-TEXT"));
    assert_eq!(item.relations().len(), 1);
    assert_eq!(item.relations()[0].name(), "depends_on");
    assert_eq!(item.relations()[0].target(), "REQ-SOURCE");
    assert_eq!(
        item.mentions()
            .iter()
            .map(|mention| mention.target())
            .collect::<Vec<_>>(),
        ["REQ-SOURCE"]
    );

    assert_eq!(item.source().path(), Path::new("docs/model.mara.md"));
    assert_eq!(item.source().span().start_line(), 11);
    assert_eq!(item.source().span().end_line(), 24);
    assert_eq!(item.body_source().span().start_line(), 17);
    assert_eq!(item.metadata()[2].source().span().start_line(), 14);
    assert_eq!(item.relations()[0].source(), item.metadata()[2].source());
    assert_eq!(item.mentions()[0].source().span().start_line(), 17);
    let mention_span = item.mentions()[0].source().span();
    assert_eq!(
        &source[mention_span.start_byte()..mention_span.end_byte()],
        "[[REQ-SOURCE]]"
    );
}

#[test]
fn parses_the_repository_documents_deterministically() {
    let (fixture, project, schema) = initialized_project();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "alpha.mara.md",
        "format.mara.md",
        "index.mara.md",
        "taxonomy.mara.md",
    ] {
        let source = fs::read_to_string(repository.join("docs").join(name)).unwrap();
        write(fixture.path(), &format!("docs/{name}"), &source);
    }

    let first = load_corpus(&project, &schema).unwrap();
    let second = load_corpus(&project, &schema).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.documents().len(), 4);
    assert_eq!(first.items().count(), 40);
    assert!(
        first
            .items()
            .any(|item| item.id() == "REQ-CANONICAL-SOURCE")
    );
    assert!(first.items().any(|item| item.id() == "DES-DOCUMENT-FORMAT"));
    assert!(
        first
            .items()
            .any(|item| item.id() == "DES-DETERMINISTIC-KEYWORD-SEARCH")
    );
    assert!(
        first
            .items()
            .any(|item| item.id() == "REQ-PORTABLE-AGENT-ONBOARDING")
    );
    assert!(
        first
            .items()
            .any(|item| item.id() == "REQ-DURABLE-ITEM-IDENTITY")
    );
    assert!(
        first
            .items()
            .any(|item| item.id() == "ADR-RUSHDOWN-PARSER-ADAPTER")
    );
    assert!(!first.items().any(|item| item.id() == "REQ-FAIL-SAFETY"));
}

#[test]
fn reports_malformed_item_openers_instead_of_silently_dropping_data() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "broken.mara.md",
        ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
    );

    let error = load_corpus(&project, &schema).unwrap_err().to_string();

    assert!(error.contains("broken.mara.md:1"), "{error}");
    assert!(error.contains("with no other tokens"), "{error}");
}

#[test]
fn validation_retains_independent_document_parse_diagnostics() {
    let (fixture, project, schema) = initialized_project();
    for name in ["first", "second"] {
        write(
            fixture.path(),
            &format!("{name}.mara.md"),
            ":::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBody.\n:::\n",
        );
    }

    let (corpus, diagnostics) = load_corpus_for_validation(&project, &schema).unwrap();

    assert!(corpus.documents().is_empty());
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].source().path(), Path::new("first.mara.md"));
    assert_eq!(diagnostics[1].source().path(), Path::new("second.mara.md"));
}

#[test]
fn validation_suppresses_body_blocks_for_invalid_titles() {
    let (fixture, project, schema) = initialized_project();
    for title_metadata in ["", ":title: \n", ":title: First\n:title: Second\n"] {
        let source = format!(
            ":::mara requirement REQ-FIRST\n:title: First\n\nBefore.\n:::\n\n:::mara requirement REQ-BROKEN\n{title_metadata}\n# Body\n\nSee [[REQ-FIRST]].\n:::\n\n:::mara requirement REQ-LAST\n:title: Last\n\nAfter.\n:::\n"
        );
        write(fixture.path(), "titles.mara.md", &source);
        let (corpus, diagnostics) = load_corpus_for_validation(&project, &schema).unwrap();
        let items = corpus.items().collect::<Vec<_>>();
        assert_eq!(
            items.iter().map(|item| item.id()).collect::<Vec<_>>(),
            ["REQ-FIRST", "REQ-BROKEN", "REQ-LAST"]
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message(),
            "item must have exactly one non-empty title entry"
        );
        assert_eq!(diagnostics[0].source().span().start_line(), 7);
        assert!(items[1].body_blocks().is_empty(), "{title_metadata:?}");
        assert!(!items[0].body_blocks().is_empty());
        assert!(!items[2].body_blocks().is_empty());
        assert_eq!(items[1].body(), "# Body\n\nSee [[REQ-FIRST]].\n");
        assert_eq!(items[1].mentions()[0].target(), "REQ-FIRST");
        assert_eq!(
            items[1]
                .metadata()
                .iter()
                .filter(|entry| entry.key() == "title")
                .count(),
            title_metadata.lines().count()
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("titles.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn title_recovery_preserves_independent_missing_body_diagnostics() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "empty.mara.md",
        ":::mara requirement REQ-EMPTY\n:title: \n\n:::\n",
    );
    let (corpus, parse_diagnostics) = load_corpus_for_validation(&project, &schema).unwrap();
    assert!(parse_diagnostics.iter().any(
        |diagnostic| diagnostic.message() == "item must have exactly one non-empty title entry"
    ));
    assert!(
        mara::validate_corpus(&corpus, &schema)
            .iter()
            .any(|diagnostic| diagnostic.message() == "required body is empty")
    );
}

#[test]
fn validation_retains_valid_items_around_a_malformed_item() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "mixed.mara.md",
        ":::mara requirement REQ-FIRST\n:title: First\n\nFirst body.\n:::\n\n:::mara requirement REQ-BROKEN trailing\n:title: Broken\n\nBroken body.\n:::\n\n:::mara requirement REQ-LAST\n:title: Last\n\nLast body.\n:::\n",
    );

    let (corpus, diagnostics) = load_corpus_for_validation(&project, &schema).unwrap();
    let ids = corpus.items().map(mara::Item::id).collect::<Vec<_>>();

    assert_eq!(corpus.documents().len(), 1);
    assert_eq!(ids, ["REQ-FIRST", "REQ-LAST"]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source().path(), Path::new("mixed.mara.md"));
    assert_eq!(diagnostics[0].source().span().start_line(), 7);
    assert!(diagnostics[0].message().contains("with no other tokens"));
}

#[test]
fn reports_tab_separated_item_openers_as_malformed() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "tabbed.mara.md",
        ":::mara\trequirement REQ-BROKEN\n:title: Broken\n\nBody.\n:::\n",
    );

    let error = load_corpus(&project, &schema).unwrap_err().to_string();

    assert!(error.contains("tabbed.mara.md:1"), "{error}");
    assert!(error.contains("with no other tokens"), "{error}");
}

#[test]
fn excludes_gitignored_mara_documents() {
    let (fixture, project, schema) = initialized_project();
    write(fixture.path(), ".gitignore", "ignored.mara.md\n");
    write(
        fixture.path(),
        "ignored.mara.md",
        ":::mara requirement REQ-IGNORED\n:title: Ignored\n\nBody.\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();

    assert!(corpus.documents().is_empty());
}

#[test]
fn excludes_documents_ignored_by_the_parent_git_repository() {
    let fixture = TempDir::new().unwrap();
    fs::create_dir(fixture.path().join(".git")).unwrap();
    write(fixture.path(), ".gitignore", "project/generated.mara.md\n");
    let project_root = fixture.path().join("project");
    let project = initialize_project(&project_root, Template::Minimal).unwrap();
    let schema = load_schema(&project).unwrap();
    write(&project_root, "kept.mara.md", "Kept.\n");
    write(&project_root, "generated.mara.md", "Generated.\n");

    let corpus = load_corpus(&project, &schema).unwrap();

    assert_eq!(corpus.documents().len(), 1);
    assert_eq!(corpus.documents()[0].path(), Path::new("kept.mara.md"));
}

#[test]
fn treats_multiline_inline_code_as_example_text_during_item_scans() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "docs/examples.mara.md",
        r#"`example
:::mara requirement REQ-EXAMPLE
`

:::mara requirement REQ-REAL
:title: Real

Before.
`code
:::
`
After with [[REQ-TARGET]].
:::
"#,
    );

    let corpus = load_corpus(&project, &schema).unwrap();
    let item = corpus.items().next().unwrap();

    assert_eq!(corpus.items().count(), 1);
    assert_eq!(item.id(), "REQ-REAL");
    assert!(item.body().contains("After with [[REQ-TARGET]]"));
    assert_eq!(
        item.mentions()
            .iter()
            .map(|mention| mention.target())
            .collect::<Vec<_>>(),
        ["REQ-TARGET"]
    );
}

#[test]
fn accepts_whitespace_only_body_boundaries() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "spaces.mara.md",
        ":::mara requirement REQ-SPACES\n:title: Spaces\n \t \nBody.\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();

    assert_eq!(corpus.items().next().unwrap().body(), "Body.\n");
}

#[test]
fn treats_unmatched_backticks_as_text_when_extracting_mentions() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "mention.mara.md",
        ":::mara requirement REQ-MENTION\n:title: Mention\n\nAn unmatched ` before [[REQ-TARGET]].\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();
    let item = corpus.items().next().unwrap();

    assert_eq!(item.mentions()[0].target(), "REQ-TARGET");
    assert_eq!(item.mentions().len(), 1);
}

#[test]
fn does_not_pair_an_unmatched_backtick_across_markdown_blocks() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "paragraphs.mara.md",
        "An unmatched ` in ordinary prose.\n\n:::mara requirement REQ-REAL\n:title: Real\n\nBody.\n:::\n\nLater `code`.\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();

    assert_eq!(corpus.items().next().unwrap().id(), "REQ-REAL");
    assert_eq!(corpus.items().count(), 1);
}

#[test]
fn excludes_mentions_inside_blockquoted_fenced_code() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "quote.mara.md",
        ":::mara requirement REQ-REAL\n:title: Real\n\n> ~~~markdown\n> [[REQ-EXAMPLE]]\n> ~~~\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();

    assert!(corpus.items().next().unwrap().mentions().is_empty());
}

#[test]
fn recognizes_adjacent_item_delimiters_through_markdown_inline_parsing() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "adjacent.mara.md",
        ":::mara requirement REQ-FIRST\n:title: First\n\nFirst body.\n:::\n:::mara requirement REQ-SECOND\n:title: Second\n\nSecond body.\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();

    assert_eq!(
        corpus.items().map(|item| item.id()).collect::<Vec<_>>(),
        ["REQ-FIRST", "REQ-SECOND"]
    );
}

#[test]
fn ignores_mara_syntax_in_raw_html_blocks() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "raw.mara.md",
        "<script>\n:::mara requirement REQ-EXAMPLE\n:title: Example\n\n[[REQ-NOT-DATA]]\n:::\n</script>\n\n:::mara requirement REQ-REAL\n:title: Real\n\n<script>\n[[REQ-NOT-DATA]]\n</script>\n\n[[REQ-TARGET]]\n:::\n",
    );

    let corpus = load_corpus(&project, &schema).unwrap();
    let item = corpus.items().next().unwrap();

    assert_eq!(corpus.items().count(), 1);
    assert_eq!(item.id(), "REQ-REAL");
    assert_eq!(
        item.mentions()
            .iter()
            .map(|mention| mention.target())
            .collect::<Vec<_>>(),
        ["REQ-TARGET"]
    );
}

#[test]
fn rejects_nested_items_after_the_body_boundary() {
    let (fixture, project, schema) = initialized_project();
    write(
        fixture.path(),
        "nested.mara.md",
        ":::mara requirement REQ-OUTER\n:title: Outer\n\n:::mara requirement REQ-INNER\n:title: Inner\n\nBody.\n:::\n:::\n",
    );

    let error = load_corpus(&project, &schema).unwrap_err().to_string();

    assert!(error.contains("nested.mara.md:4"), "{error}");
    assert!(error.contains("items cannot nest"), "{error}");
}

#[test]
fn retains_markdown_children_inside_items_with_original_utf8_crlf_spans() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    let body = "### Héading\r\n\r\n> Quote.\r\n>\r\n> - Outer\r\n>   - Inner with **bold** and `code`.\r\n\r\n| Name | Value |\r\n| --- | --- |\r\n| α | β |\r\n\r\n```text\r\n:::mara requirement REQ-EXAMPLE\r\n:::\r\n```\r\n\r\n<script>\r\n:::mara requirement REQ-RAW\r\n:::\r\n</script>\r\n\r\nLast paragraph with [[REQ-TARGET]].\r\n";
    let source = format!(
        "# Outside\r\n\r\n:::mara requirement REQ-TREE\r\n:title: Tree\r\n:tag: first\r\n:tag: second\r\n\r\n{body}:::\r\n\r\n# After\r\n"
    );
    write(fixture.path(), "tree.mara.md", &source);

    let corpus = load_corpus(&project, &schema).unwrap();
    let item = corpus.items().next().unwrap();
    assert_eq!(corpus.items().count(), 1);
    assert_eq!(item.body(), body);
    assert_eq!(corpus.documents()[0].source(), source);
    let blocks = item.body_blocks();
    assert_eq!(
        blocks.iter().map(|block| block.kind()).collect::<Vec<_>>(),
        [
            Kind::Heading { level: 3 },
            Kind::Blockquote,
            Kind::Table,
            Kind::CodeBlock,
            Kind::HtmlBlock,
            Kind::Paragraph,
        ]
    );
    assert_eq!(blocks[1].children()[1].kind(), Kind::List);
    let quote_paragraph = blocks[1].children()[0].source().span();
    assert_eq!(
        &source[quote_paragraph.start_byte()..quote_paragraph.end_byte()],
        "Quote.\r\n"
    );
    let outer_item = &blocks[1].children()[1].children()[0];
    assert_eq!(outer_item.kind(), Kind::ListItem);
    assert_eq!(outer_item.children()[1].kind(), Kind::List);
    assert_eq!(item.mentions().len(), 1);
    assert_eq!(item.mentions()[0].target(), "REQ-TARGET");
    let expected = [
        "### Héading\r\n",
        "> Quote.\r\n>\r\n> - Outer\r\n>   - Inner with **bold** and `code`.\r\n",
        "| Name | Value |\r\n| --- | --- |\r\n| α | β |\r\n",
        "```text\r\n:::mara requirement REQ-EXAMPLE\r\n:::\r\n```\r\n",
        "<script>\r\n:::mara requirement REQ-RAW\r\n:::\r\n</script>\r\n",
        "Last paragraph with [[REQ-TARGET]].\r\n",
    ];
    for (block, expected) in blocks.iter().zip(expected) {
        assert_eq!(
            &source[block.source().span().start_byte()..block.source().span().end_byte()],
            expected,
            "{:?}",
            block.kind()
        );
        let start = source.find(expected).unwrap();
        assert_eq!(block.source().span().start_byte(), start);
        assert_eq!(
            block.source().span().start_line(),
            source[..start].bytes().filter(|&b| b == b'\n').count() + 1
        );
    }
    assert_eq!(
        fs::read_to_string(fixture.path().join("tree.mara.md")).unwrap(),
        source
    );
}

#[test]
fn container_boundaries_preserve_code_context_and_adjacent_empty_items() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    let source = "    :::mara requirement REQ-INDENTED\n\n> :::mara requirement REQ-QUOTED\n\n:::mara requirement REQ-CODE\n:title: Code\n\n`multiline\n:::\n:::mara requirement REQ-EXAMPLE\n`\n\n    :::mara requirement REQ-INDENTED-BODY\n    :::\n\nEnd.\n:::\n:::mara requirement REQ-EMPTY\n:title: Empty\n\n:::";
    write(fixture.path(), "contexts.mara.md", source);
    let corpus = load_corpus(&project, &schema).unwrap();
    let items = corpus.items().collect::<Vec<_>>();
    assert_eq!(
        items.iter().map(|item| item.id()).collect::<Vec<_>>(),
        ["REQ-CODE", "REQ-EMPTY"]
    );
    assert_eq!(
        items[0]
            .body_blocks()
            .iter()
            .map(|block| block.kind())
            .collect::<Vec<_>>(),
        [Kind::Paragraph, Kind::CodeBlock, Kind::Paragraph]
    );
    assert!(items[1].body_blocks().is_empty());
    assert_eq!(items[1].source().span().end_byte(), source.len());
}

#[test]
fn tab_indented_containers_do_not_overlap_following_siblings() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    for newline in ["\n", "\r\n"] {
        let first = format!("*\t>\t-{newline}");
        let following = format!(">\ttéxt 🌱{newline}end{newline}");
        let body = format!("{first}{following}");
        let source = format!(":::mara requirement REQ-TABS\n:title: Tabs\n\n{body}:::\n");
        write(fixture.path(), "tabs.mara.md", &source);
        let corpus = load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let blocks = item.body_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind(), Kind::List);
        assert_eq!(blocks[1].kind(), Kind::Blockquote);
        let list_span = blocks[0].source().span();
        let quote_span = blocks[1].source().span();
        assert!(list_span.end_byte() <= quote_span.start_byte());
        assert_eq!(&source[list_span.start_byte()..list_span.end_byte()], first);
        assert_eq!(
            &source[quote_span.start_byte()..quote_span.end_byte()],
            following
        );
        assert_eq!(list_span.start_line(), 4);
        assert_eq!(list_span.end_line(), 4);
        assert_eq!(quote_span.start_line(), 5);
        assert_eq!(quote_span.end_line(), 6);
        let paragraph = blocks[1].children()[0].source().span();
        assert_eq!(
            &source[paragraph.start_byte()..paragraph.end_byte()],
            &following[2..]
        );
        let mut pending = blocks.iter().collect::<Vec<_>>();
        while let Some(parent) = pending.pop() {
            let span = parent.source().span();
            let mut previous_end = span.start_byte();
            for child in parent.children() {
                let child_span = child.source().span();
                assert!(previous_end <= child_span.start_byte());
                assert!(child_span.start_byte() <= child_span.end_byte());
                assert!(child_span.end_byte() <= span.end_byte());
                previous_end = child_span.end_byte();
                pending.push(child);
            }
        }
        assert_eq!(item.body(), body);
        assert_eq!(
            fs::read_to_string(fixture.path().join("tabs.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn nested_containers_exclude_outer_quote_separators() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    for (body, expected, kind) in [
        ("> - one\n>\n> next\n", "- one\n", Kind::List),
        ("> > inner\n>\n> outer\n", "> inner\n", Kind::Blockquote),
        ("> -\n>\n> next\n", "-\n", Kind::List),
        ("> >\n>\n> next\n", ">\n", Kind::Blockquote),
        (
            "> > α\r\n> >\r\n>\r\n> next\r\n",
            "> α\r\n> >\r\n",
            Kind::Blockquote,
        ),
        ("> - > α\n>   >\n>\n> next\n", "- > α\n>   >\n", Kind::List),
        (
            "> > inner\nlazy continuation\n>\n> outer\n",
            "> inner\nlazy continuation\n",
            Kind::Blockquote,
        ),
        (
            "> - first\n>\n> - second\n>\n> next\n",
            "- first\n>\n> - second\n",
            Kind::List,
        ),
    ] {
        let source =
            format!(":::mara requirement REQ-CONTAINERS\n:title: Containers\n\n{body}:::\n");
        write(fixture.path(), "containers.mara.md", &source);
        let corpus = load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let outer = &item.body_blocks()[0];
        assert_eq!(outer.kind(), Kind::Blockquote);
        let blocks = outer.children();
        assert_eq!(blocks[0].kind(), kind);
        assert_eq!(blocks[1].kind(), Kind::Paragraph);
        let span = blocks[0].source().span();
        assert_eq!(
            &source[span.start_byte()..span.end_byte()],
            expected,
            "{body}"
        );
        assert_eq!(span.start_byte(), source.find(expected).unwrap());
        assert_eq!(span.start_line(), 4);
        assert_eq!(span.end_line(), 3 + expected.lines().count());
        if kind == Kind::List {
            let last_item = blocks[0].children().last().unwrap();
            assert_eq!(last_item.kind(), Kind::ListItem);
            assert_eq!(last_item.source().span().end_byte(), span.end_byte());
        }
        let outer_span = outer.source().span();
        assert_eq!(
            &source[outer_span.start_byte()..outer_span.end_byte()],
            body
        );
        assert_eq!(item.body(), body);
        assert_eq!(
            fs::read_to_string(fixture.path().join("containers.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn quoted_html_and_indented_code_end_at_their_parsed_content() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    for (body, expected, kind) in [
        (
            "> <!-- comment -->\n>\n> para\n",
            "<!-- comment -->\n",
            Kind::HtmlBlock,
        ),
        (">     code\n>\n> para\n", "code\n", Kind::CodeBlock),
        (
            "> > <script>\r\n> >\r\n> > α\r\n> > </script>\r\n> >\r\n> > para\r\n",
            "<script>\r\n> >\r\n> > α\r\n> > </script>\r\n",
            Kind::HtmlBlock,
        ),
        (
            "> >     α\r\n> >\r\n> >     > literal\r\n> >\r\n> > para\r\n",
            "α\r\n> >\r\n> >     > literal\r\n",
            Kind::CodeBlock,
        ),
    ] {
        let source = format!(":::mara requirement REQ-RAW\n:title: Raw\n\n{body}:::\n");
        write(fixture.path(), "raw.mara.md", &source);
        let corpus = load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let mut blocks = item.body_blocks();
        while blocks[0].kind() == Kind::Blockquote {
            blocks = blocks[0].children();
        }
        assert_eq!(blocks[0].kind(), kind);
        let span = blocks[0].source().span();
        assert_eq!(
            &source[span.start_byte()..span.end_byte()],
            expected,
            "{body}"
        );
        assert_eq!(span.start_byte(), source.find(expected).unwrap());
        assert_eq!(span.start_line(), 4);
        assert_eq!(span.end_line(), 3 + expected.lines().count());
        assert_eq!(blocks[1].kind(), Kind::Paragraph);
        assert_eq!(item.body(), body);
        assert_eq!(
            fs::read_to_string(fixture.path().join("raw.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn quoted_leaf_blocks_exclude_following_quote_separators() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    for (block_text, kind) in [
        ("# Héading ###\n", Kind::Heading { level: 1 }),
        ("Two-line\nheading\n===\n", Kind::Heading { level: 1 }),
        ("```text\n>\n\n```\n", Kind::CodeBlock),
        ("~~~\n~~~\n", Kind::CodeBlock),
        ("---\n", Kind::ThematicBreak),
        ("[foo]: /url\n", Kind::LinkReferenceDefinition),
        (
            "[foo]: /url\n  \"A\n  title\"\n",
            Kind::LinkReferenceDefinition,
        ),
        ("[foo]: /url\n  \"\"\n", Kind::LinkReferenceDefinition),
    ] {
        for prefix in ["> ", "> > "] {
            let quoted = block_text
                .lines()
                .map(|line| format!("{prefix}{line}\r\n"))
                .collect::<String>();
            let body = format!("{quoted}{prefix}\r\n{prefix}para\r\n");
            let source = format!(":::mara requirement REQ-QUOTE\n:title: Quote\n\n{body}:::\n");
            write(fixture.path(), "quote.mara.md", &source);
            let corpus = load_corpus(&project, &schema).unwrap();
            let item = corpus.items().next().unwrap();
            let mut blocks = item.body_blocks();
            while blocks[0].kind() == Kind::Blockquote {
                blocks = blocks[0].children();
            }
            assert_eq!(blocks[0].kind(), kind, "{body}");
            let span = blocks[0].source().span();
            assert_eq!(
                &source[span.start_byte()..span.end_byte()],
                &quoted[prefix.len()..],
                "{body}"
            );
            assert_eq!(span.start_line(), 4);
            assert_eq!(span.end_line(), 3 + block_text.lines().count());
            assert_eq!(blocks[1].kind(), Kind::Paragraph);
            assert_eq!(item.body(), body);
            assert_eq!(
                fs::read_to_string(fixture.path().join("quote.mara.md")).unwrap(),
                source
            );
        }
    }
}

#[test]
fn quoted_unclosed_fence_preserves_literal_quote_lines_until_container_end() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    let source = ":::mara requirement REQ-CODE\n:title: Code\n\n> ```text\n> literal\n> >\n\nOutside.\n:::\n";
    write(fixture.path(), "code.mara.md", source);
    let corpus = load_corpus(&project, &schema).unwrap();
    let blocks = corpus.items().next().unwrap().body_blocks();
    assert_eq!(blocks[0].kind(), Kind::Blockquote);
    assert_eq!(blocks[1].kind(), Kind::Paragraph);
    let code = &blocks[0].children()[0];
    assert_eq!(code.kind(), Kind::CodeBlock);
    let span = code.source().span();
    assert_eq!(
        &source[span.start_byte()..span.end_byte()],
        "```text\n> literal\n> >\n"
    );
}

#[test]
fn reference_definitions_and_adjacent_prose_have_separate_source_spans() {
    use mara::MarkdownBlockKind as Kind;

    let (fixture, project, schema) = initialized_project();
    for (definitions, prose, newline, prefix) in [
        (vec!["[foo]: /url"], "ordinary paragraph", "\n", ""),
        (
            vec!["[foo]: /url", "[bar]: /other \"Title\""],
            "Résumé with [foo] and [bar].",
            "\r\n",
            "> ",
        ),
    ] {
        let body = definitions
            .iter()
            .copied()
            .chain(std::iter::once(prose))
            .map(|line| format!("{prefix}{line}{newline}"))
            .collect::<String>();
        let source =
            format!("Prelude.\n\n:::mara requirement REQ-REF\n:title: References\n\n{body}:::\n");
        write(fixture.path(), "references.mara.md", &source);
        let corpus = load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let blocks = if prefix.is_empty() {
            item.body_blocks()
        } else {
            item.body_blocks()[0].children()
        };
        assert_eq!(blocks.len(), definitions.len() + 1);
        let mut previous_end = item.body_source().span().start_byte();
        for (index, (block, expected)) in blocks
            .iter()
            .zip(definitions.iter().copied().chain(std::iter::once(prose)))
            .enumerate()
        {
            assert_eq!(
                block.kind(),
                if index < definitions.len() {
                    Kind::LinkReferenceDefinition
                } else {
                    Kind::Paragraph
                }
            );
            let expected = format!("{expected}{newline}");
            let span = block.source().span();
            assert_eq!(&source[span.start_byte()..span.end_byte()], expected);
            assert_eq!(span.start_byte(), source.find(&expected).unwrap());
            assert_eq!(span.start_line(), 6 + index);
            assert_eq!(span.end_line(), span.start_line());
            assert!(span.start_byte() >= previous_end);
            previous_end = span.end_byte();
        }
        assert_eq!(item.body(), body);
        assert_eq!(
            fs::read_to_string(fixture.path().join("references.mara.md")).unwrap(),
            source
        );
    }
}

#[test]
fn table_children_retain_only_their_own_source() {
    use mara::{MarkdownBlock, MarkdownBlockKind as Kind};

    fn text<'a>(source: &'a str, block: &MarkdownBlock) -> &'a str {
        let span = block.source().span();
        &source[span.start_byte()..span.end_byte()]
    }

    let (fixture, project, schema) = initialized_project();
    // Header-only and quoted tables must not lend separator rows or enclosing
    // quote markers to their cells. Short rows contain synthetic empty cells.
    for (body, header, rows, cells) in [
        (
            "| A | B |\n|---|---|\n| a | b |\n",
            "| A | B |\n",
            vec!["| a | b |\n"],
            vec![vec!["A", "B"], vec!["a", "b"]],
        ),
        (
            "| A | B |\n|---|---|\n",
            "| A | B |\n",
            vec![],
            vec![vec!["A", "B"]],
        ),
        (
            "> | α | β |\r\n> | --- | --- |\r\n> | a\\|b | `γ` |\r\n> | δ |\r\n>\r\n",
            "| α | β |\r\n",
            vec!["| a\\|b | `γ` |\r\n", "| δ |\r\n"],
            vec![vec!["α", "β"], vec!["a\\|b", "`γ`"], vec!["δ", ""]],
        ),
    ] {
        let source =
            format!("Prelude.\n\n:::mara requirement REQ-TABLE\n:title: Table\n\n{body}:::\n");
        write(fixture.path(), "table.mara.md", &source);
        let corpus = load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let block = &item.body_blocks()[0];
        let table = if block.kind() == Kind::Blockquote {
            &block.children()[0]
        } else {
            block
        };
        assert_eq!(
            table.kind(),
            Kind::Table,
            "body: {body:?}, blocks: {:?}",
            item.body_blocks()
        );
        let table_header = &table.children()[0];
        let header_row = &table_header.children()[0];
        assert_eq!(text(&source, table_header), header);
        assert_eq!(text(&source, header_row), header);
        let body_rows = table
            .children()
            .get(1)
            .map_or(&[][..], |body| body.children());
        assert_eq!(
            body_rows
                .iter()
                .map(|row| text(&source, row))
                .collect::<Vec<_>>(),
            rows
        );
        for (row, expected) in std::iter::once(header_row).chain(body_rows).zip(cells) {
            assert_eq!(
                row.children()
                    .iter()
                    .map(|cell| text(&source, cell))
                    .collect::<Vec<_>>(),
                expected
            );
            for cell in row.children() {
                let span = cell.source().span();
                assert!(span.start_byte() >= row.source().span().start_byte());
                assert!(span.end_byte() <= row.source().span().end_byte());
                assert_eq!(span.start_line(), span.end_line());
            }
        }
        assert_eq!(item.body(), body);
        assert_eq!(
            fs::read_to_string(fixture.path().join("table.mara.md")).unwrap(),
            source
        );
    }
}
