mod support;
use mara::resolve_project;
use serde_json::{Value, json};
use std::fs;
use support::*;

// @mara implements VER-DOCUMENT-NAVIGATION
// @mara checks DES-DOCUMENT-STRUCTURE
#[test]
fn unicode_setext_headings_create_and_reload_through_the_real_cli() {
    for (body, level) in [("Café\n====\n", 1), ("First line\r\n終🙂\r\n----\r\n", 2)] {
        let fixture = fixture();
        let initialized = mara(fixture.path(), &["project", "init"]);
        assert!(initialized.status.success(), "{}", stderr(&initialized));
        let created = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                "REQ-UNICODE",
                "unicode.mara.md",
                "--title",
                "Unicode heading",
                "--body",
                body,
            ],
        );
        assert!(created.status.success(), "{}", stderr(&created));
        let original = fs::read_to_string(fixture.path().join("unicode.mara.md")).unwrap();
        let fetched = mara(fixture.path(), &["--format", "json", "get", "REQ-UNICODE"]);
        assert!(fetched.status.success(), "{}", stderr(&fetched));
        assert_eq!(
            serde_json::from_slice::<Value>(&fetched.stdout).unwrap()["content"],
            body
        );
        let validated = mara(fixture.path(), &["--format", "json", "project", "validate"]);
        assert!(validated.status.success(), "{}", stderr(&validated));
        assert_eq!(
            serde_json::from_slice::<Value>(&validated.stdout).unwrap()["valid"],
            true
        );

        let project = resolve_project(Some(fixture.path()), fixture.path()).unwrap();
        let schema = mara::load_schema(&project).unwrap();
        let corpus = mara::load_corpus(&project, &schema).unwrap();
        let item = corpus.items().next().unwrap();
        let heading = &item.body_blocks()[0];
        assert_eq!(heading.kind(), mara::MarkdownBlockKind::Heading { level });
        let span = heading.source().span();
        assert_eq!(&original[span.start_byte()..span.end_byte()], body);
        assert_eq!(
            span.end_line() - span.start_line() + 1,
            body.lines().count()
        );
        assert_eq!(
            fs::read_to_string(fixture.path().join("unicode.mara.md")).unwrap(),
            original
        );
    }
}

// @mara implements VER-DOCUMENT-NAVIGATION
// @mara checks DES-DOCUMENT-STRUCTURE
#[test]
fn tab_indented_unicode_loads_through_real_cli_workflows() {
    for body in ["1. a\n\n\t   α\n", "1. a\r\n\r\n\t   🙂\r\n"] {
        let fixture = fixture();
        let initialized = mara(fixture.path(), &["project", "init"]);
        assert!(initialized.status.success(), "{}", stderr(&initialized));
        let created = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                "REQ-TABS",
                "tabs.mara.md",
                "--title",
                "Tabs",
                "--body",
                body,
            ],
        );
        assert!(created.status.success(), "{}", stderr(&created));
        let original = fs::read_to_string(fixture.path().join("tabs.mara.md")).unwrap();
        let fetched = mara(fixture.path(), &["--format", "json", "get", "REQ-TABS"]);
        assert!(fetched.status.success(), "{}", stderr(&fetched));
        assert_eq!(
            serde_json::from_slice::<Value>(&fetched.stdout).unwrap()["content"],
            body
        );
        let validated = mara(fixture.path(), &["--format", "json", "project", "validate"]);
        assert!(validated.status.success(), "{}", stderr(&validated));
        let project = resolve_project(Some(fixture.path()), fixture.path()).unwrap();
        let schema = mara::load_schema(&project).unwrap();
        let corpus = mara::load_corpus(&project, &schema).unwrap();
        let mut blocks = corpus
            .items()
            .next()
            .unwrap()
            .body_blocks()
            .iter()
            .collect::<Vec<_>>();
        let mut saw_code = false;
        while let Some(block) = blocks.pop() {
            let span = block.source().span();
            assert!(
                original.get(span.start_byte()..span.end_byte()).is_some(),
                "{block:?}"
            );
            if block.kind() == mara::MarkdownBlockKind::CodeBlock {
                saw_code = true;
                assert_eq!(
                    &original[span.start_byte()..span.end_byte()],
                    body.rsplit_once('\t').unwrap().1.trim_start()
                );
            }
            blocks.extend(block.children());
        }
        assert!(saw_code);
        assert_eq!(
            fs::read_to_string(fixture.path().join("tabs.mara.md")).unwrap(),
            original
        );
    }
}

// @mara implements VER-DOCUMENT-NAVIGATION
// @mara checks DES-DOCUMENT-STRUCTURE
#[test]
fn nested_markdown_round_trips_through_real_authoring_and_editing() {
    let fixture = fixture();
    assert!(mara(fixture.path(), &["project", "init"]).status.success());
    let body = "## Résumé\n\n> - Outer\n>   - Inner with **emphasis**.\n\n| Name | Value |\n| --- | --- |\n| α | β |\n\n```markdown\n:::mara requirement REQ-EXAMPLE\n:::\n```\n\n`multiline\n:::\n`\n";
    let created = mara(
        fixture.path(),
        &[
            "item",
            "create",
            "requirement",
            "REQ-NESTED",
            "source.mara.md",
            "--title",
            "Nested Markdown",
            "--body",
            body,
        ],
    );
    assert!(created.status.success(), "{}", stderr(&created));
    let original = fs::read_to_string(fixture.path().join("source.mara.md")).unwrap();
    assert!(original.contains(body));

    let updated = mara(
        fixture.path(),
        &[
            "item",
            "update",
            "REQ-NESTED",
            "--title",
            "Retained Markdown",
        ],
    );
    assert!(updated.status.success(), "{}", stderr(&updated));
    let expected = original.replace(":title: Nested Markdown", ":title: Retained Markdown");
    assert_eq!(
        fs::read_to_string(fixture.path().join("source.mara.md")).unwrap(),
        expected
    );

    let moved = mara(
        fixture.path(),
        &["item", "move", "REQ-NESTED", "destination.mara.md"],
    );
    assert!(moved.status.success(), "{}", stderr(&moved));
    assert_eq!(
        fs::read_to_string(fixture.path().join("destination.mara.md")).unwrap(),
        expected
    );
    let fetched = mara(fixture.path(), &["--format", "json", "get", "REQ-NESTED"]);
    assert!(fetched.status.success(), "{}", stderr(&fetched));
    let fetched: Value = serde_json::from_slice(&fetched.stdout).unwrap();
    assert_eq!(fetched["content"], body);

    let responses = mcp_exchange(
        fixture.path(),
        &[
            mcp_initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            mcp_call(
                2,
                "get",
                json!({"project":fixture.path(),"reference":"REQ-NESTED"}),
            ),
        ],
    );
    assert_eq!(
        mcp_response(&responses, 2)["result"]["structuredContent"],
        fetched
    );
    let validated = mara(fixture.path(), &["--format", "json", "project", "validate"]);
    assert!(validated.status.success(), "{}", stdout(&validated));
    assert_eq!(
        serde_json::from_slice::<Value>(&validated.stdout).unwrap()["valid"],
        true
    );
}

// @mara implements VER-DOCUMENT-NAVIGATION
// @mara checks DES-DOCUMENT-STRUCTURE
#[test]
fn eof_reference_titles_load_through_real_cli_workflows() {
    for newline in ["\n", "\r\n"] {
        let fixture = support::fixture();
        assert!(mara(fixture.path(), &["project", "init"]).status.success());
        let created = mara(
            fixture.path(),
            &[
                "item",
                "create",
                "requirement",
                "REQ-ONE",
                "eof.mara.md",
                "--title",
                "One",
                "--body",
                "Body.",
            ],
        );
        assert!(created.status.success(), "{}", stderr(&created));
        let path = fixture.path().join("eof.mara.md");
        let source = format!(
            "{}\n[ref]: https://example.com\n  \"終🙂 title\"",
            fs::read_to_string(&path).unwrap()
        )
        .replace('\n', newline);
        fs::write(&path, &source).unwrap();
        let validated = mara(fixture.path(), &["--format", "json", "project", "validate"]);
        assert!(validated.status.success(), "{}", stderr(&validated));
        assert_eq!(
            serde_json::from_slice::<Value>(&validated.stdout).unwrap()["valid"],
            true
        );
        let fetched = mara(fixture.path(), &["--format", "json", "get", "REQ-ONE"]);
        assert!(fetched.status.success(), "{}", stderr(&fetched));
        assert_eq!(
            serde_json::from_slice::<Value>(&fetched.stdout).unwrap()["content"],
            format!("Body.{newline}")
        );
        let project = resolve_project(Some(fixture.path()), fixture.path()).unwrap();
        let schema = mara::load_schema(&project).unwrap();
        let corpus = mara::load_corpus(&project, &schema).unwrap();
        let definition = &corpus.documents()[0].blocks()[0];
        assert_eq!(
            definition.kind(),
            mara::MarkdownBlockKind::LinkReferenceDefinition
        );
        let span = definition.source().span();
        assert_eq!(
            &source[span.start_byte()..span.end_byte()],
            format!("[ref]: https://example.com{newline}  \"終🙂 title\"")
        );
        assert_eq!(span.end_byte(), source.len());
        assert_eq!(fs::read_to_string(path).unwrap(), source);
    }
}
