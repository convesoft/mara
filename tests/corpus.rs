use std::{fs, path::Path};

use mara::{Template, initialize_project, load_corpus, load_schema};
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
