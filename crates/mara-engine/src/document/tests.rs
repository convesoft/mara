//! Tests of the document parser.

use super::*;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";
const C: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";

fn parse_text(text: &str) -> Document {
    parse(text.as_bytes(), &mut WorkCounts::default())
}

/// `(line, code)` of each diagnostic.
fn problems(document: &Document) -> Vec<(usize, &'static str)> {
    document
        .diagnostics("docs/a.mara.md")
        .map(|diagnostic| (diagnostic.line, diagnostic.code))
        .collect()
}

/// A complete item with ID `REQ-<name>`: anchor on its first line, heading on its third, table
/// from its fifth to its eighth, body on its tenth.
fn item(mid: &str, name: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### {name}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-{name} |\n| Status | Draft |\n\nBody of {name}.\n"
    )
}

fn ids(document: &Document) -> Vec<&str> {
    document
        .items
        .iter()
        .map(|item| item.value("id").unwrap_or(""))
        .collect()
}

#[test]
fn reads_the_documented_example() {
    let page = include_str!("../../../../docs/format.mara.md");
    let (_, example) = page.split_once("```markdown\n").expect("an example");
    let (example, _) = example.split_once("\n```\n").expect("the example ends");
    let document = parse_text(example);
    assert_eq!(problems(&document), []);
    assert!(document.mentions.is_empty());
    let [requirement, decision] = &document.items[..] else {
        panic!("two items: {:?}", document.items);
    };

    assert_eq!(requirement.mid, "01M1PXP2KGVW5ZF2JGP9K4XE9B");
    assert_eq!(requirement.title, "Phishing-resistant authentication");
    assert_eq!(requirement.line, 7);
    let rows: Vec<_> = requirement
        .rows
        .iter()
        .map(|row| (row.line, row.name.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (11, "type", "Requirement"),
            (13, "id", "REQ-PHISHING-RESISTANT-AUTH"),
            (14, "status", "Accepted"),
            (
                15,
                "satisfies",
                "[NEED-SECURE-SIGN-IN](../needs.mara.md#01M1PXQ0R8T4D2VW6YH3KJ5N7A)"
            ),
        ]
    );
    let link = requirement.rows[3].link.as_ref().expect("a relation link");
    assert_eq!(link.text, "NEED-SECURE-SIGN-IN");
    assert_eq!(link.path, "../needs.mara.md");
    assert_eq!(link.fragment.as_deref(), Some("01M1PXQ0R8T4D2VW6YH3KJ5N7A"));
    assert_eq!(link.line, 15);
    assert_eq!(&example[link.span.clone()], requirement.rows[3].value);
    let body = &example[requirement.body.clone()];
    assert!(body.starts_with("Users must authenticate"), "{body:?}");
    assert!(body.ends_with("- Passkeys must be supported."), "{body:?}");
    assert!(example[requirement.span.clone()].starts_with("<a id="));

    assert_eq!(decision.value("id"), Some("ADR-USE-WEBAUTHN"));
    let link = decision.rows[3].link.as_ref().expect("a same-file link");
    assert_eq!(
        (link.path.as_str(), link.fragment.as_deref()),
        ("", Some(requirement.mid.as_str()))
    );
    assert_eq!(
        &example[decision.body.clone()],
        "WebAuthn provides phishing-resistant authentication."
    );
}

#[test]
fn breaks_regions_at_thematic_breaks_outside_code_only() {
    let text = format!(
        "# Breaks\n\n***\n\n{}\n___\n\n{}\n```text\n---\n```\n\n- - -\n\n{}",
        item(A, "A"),
        item(B, "B"),
        item(C, "C")
    );
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert_eq!(ids(&document), ["REQ-A", "REQ-B", "REQ-C"]);
    assert!(text[document.items[1].body.clone()].ends_with("---\n```"));
}

#[test]
fn a_break_inside_a_block_ends_the_region_after_that_block() {
    let text = format!("{}\n> quoted\n>\n> ***\n\n{}", item(A, "A"), item(B, "B"));
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert_eq!(ids(&document), ["REQ-A", "REQ-B"]);
    assert!(text[document.items[0].body.clone()].ends_with("> ***"));
}

#[test]
fn skips_yaml_front_matter_at_the_start() {
    let front = "---\ntitle: Breaks\nlist:\n\n  - a [m](#01M7D8DZ2RBHGQBYAV7V3789H7)\n---\n\n";
    let text = format!("{front}{}", item(A, "A"));
    for (text, ending) in [
        (text.clone(), "\n"),
        (format!("\u{feff}{text}"), "\n"),
        (text.replace('\n', "\r\n"), "\r\n"),
        (text.replace('\n', "\r"), "\r"),
    ] {
        let document = parse_text(&text);
        assert_eq!(problems(&document), [], "{ending:?}");
        assert!(document.mentions.is_empty(), "{ending:?}");
        let item = &document.items[0];
        assert_eq!(item.line, 8, "{ending:?}");
        assert_eq!(item.rows[1].line, 14, "{ending:?}");
        assert_eq!(item.span.start, text.find("<a id").unwrap(), "{ending:?}");
        assert_eq!(&text[item.body.clone()], "Body of A.", "{ending:?}");
    }
    // The block holds any lines, a blank first one included, with any line ending.
    for text in [
        "---\n\ntitle: x\n---\n",
        "---\rtitle: x\r---\r",
        "---\n---\n",
    ] {
        assert_eq!(problems(&parse_text(text)), [], "{text:?}");
    }
    let text = format!(
        "---\rtitle: x\r---\r\r{}\r---\r\r{}",
        item(A, "A"),
        item(B, "B")
    )
    .replace('\n', "\r");
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert_eq!(ids(&document), ["REQ-A", "REQ-B"]);
    assert_eq!(document.items[1].line, 18);
    // A `---` line never closed is a thematic break.
    let document = parse_text(&format!("---\ntitle: x\n\nSee [A](#{A}).\n"));
    assert_eq!(problems(&document), []);
    assert_eq!(document.mentions[0].line, 4);
    // A file that starts with a separator reads up to its next `---` as front matter: an item
    // there is reported and not read, and narrative there is skipped without a diagnostic.
    let text = format!("---\n\n{}\n---\n\n{}", item(A, "A"), item(B, "B"));
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(3, "item-in-front-matter")]);
    assert_eq!(ids(&document), ["REQ-B"]);
    let text = format!("---\n\nIntro.\n\n---\n\n{}", item(A, "A"));
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert_eq!(ids(&document), ["REQ-A"]);
}

#[test]
fn reports_each_item_anchor_in_front_matter() {
    let text = format!(
        "---\ntitle: x\n<a id=\"{A}\"></a>\n\n### A\n  <a id=\"{B}\"></a>  \n---\n\n{}",
        item(C, "C")
    );
    for ending in ["\n", "\r\n", "\r"] {
        let text = text.replace('\n', ending);
        let document = parse_text(&text);
        assert_eq!(
            problems(&document),
            [(3, "item-in-front-matter"), (6, "item-in-front-matter")],
            "{ending:?}"
        );
        assert_eq!(ids(&document), ["REQ-C"], "{ending:?}");
        assert_eq!(document.items[0].line, 9, "{ending:?}");
    }
    // Only an anchor of the exact spelling on a line of its own counts.
    let text = format!(
        "---\nanchor: <a id=\"{A}\"></a>\n<a id='{A}'></a>\n<a id=\"{}\"></a>\n---\n",
        A.to_lowercase()
    );
    assert_eq!(problems(&parse_text(&text)), []);
    // Ordinary front matter, one with a blank first line, and an unclosed `---` report nothing.
    for text in [
        "---\ntitle: x\n---\n",
        "---\n\ntitle: x\n---\n",
        "---\n\nText.\n",
    ] {
        assert_eq!(problems(&parse_text(text)), [], "{text:?}");
    }
}

#[test]
fn reports_incomplete_items() {
    let table = "| Type | Requirement |\n| --- | --- |\n| ID | REQ-X |\n\nBody.\n";
    let cases = [
        // An anchor without a heading.
        (format!("<a id=\"{A}\"></a>\n\nText.\n\n{table}"), 1),
        // A heading without a table, and a table without a `Type` row.
        (
            format!("Intro.\n\n---\n\n<a id=\"{A}\"></a>\n\n## T\n\nText.\n\n{table}"),
            5,
        ),
        (
            format!("<a id=\"{A}\"></a>\n\n## T\n\n| ID | REQ-X |\n| --- | --- |\n\nBody.\n"),
            1,
        ),
        // No anchor, or an anchor that is not a MID.
        (format!("## T\n\n{table}"), 1),
        (
            format!("<a id=\"{}\"></a>\n\n## T\n\n{table}", A.to_lowercase()),
            1,
        ),
        (format!("<a id='{A}'></a>\n\n## T\n\n{table}"), 1),
    ];
    for (text, line) in cases {
        let document = parse_text(&text);
        assert_eq!(problems(&document), [(line, "incomplete-item")], "{text}");
        assert!(document.items.is_empty(), "{text}");
    }
    // Narrative that does not start like an item is not reported.
    let narrative =
        "## Types\n\nText.\n\n| Type | Meaning |\n| --- | --- |\n\n<a id=\"intro\"></a>\n";
    assert_eq!(problems(&parse_text(narrative)), []);
}

#[test]
fn reports_misplaced_anchors_and_still_reads_their_items() {
    // A missing separator: the second item is read as if it were there.
    let text = format!("{}\n{}", item(A, "A"), item(B, "B"));
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(12, "missing-separator")]);
    assert_eq!(ids(&document), ["REQ-A", "REQ-B"]);
    assert_eq!(document.items[0].body.end, text.find("\n\n<a id").unwrap());

    // An anchor merged into a paragraph or placed inside another block.
    let text = format!(
        "{}\nText\n<a id=\"{B}\"></a>\n\n- <a id=\"{C}\"></a>\n",
        item(A, "A")
    );
    let document = parse_text(&text);
    assert_eq!(
        problems(&document),
        [(13, "merged-anchor"), (15, "merged-anchor")]
    );
    assert_eq!(ids(&document), ["REQ-A"]);
}

#[test]
fn reports_setext_headings() {
    let text = format!("Narrative\n===\n\n---\n\n{}Last line\n---\n", item(A, "A"))
        .replace("### A", "Title A\n=======");
    let document = parse_text(&text);
    assert_eq!(
        problems(&document),
        [
            (2, "setext-heading"),
            (9, "setext-heading"),
            (18, "setext-heading")
        ]
    );
    assert_eq!(document.items[0].title, "Title A");
}

#[test]
fn reports_malformed_rows_and_keeps_the_others() {
    let text = format!(
        "<a id=\"{A}\"></a>\n\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n\
         | Aliases | a | b |\n| Status |\n| Satisfies | |\n| Methods | Test \\| Analysis |\n\
         Not a row\n\nBody.\n\n---\n\n<a id=\"{B}\"></a>\n\n## U\n\n| Type | Requirement |\n\
         | --- | --- |\n| ID | REQ-U |\n- list directly after the table\n\nBody.\n"
    );
    let document = parse_text(&text);
    assert_eq!(
        problems(&document),
        [
            (8, "malformed-row"),
            (9, "malformed-row"),
            (10, "malformed-row"),
            (12, "malformed-row"),
            (25, "malformed-row"),
        ]
    );
    let rows: Vec<_> = document.items[0]
        .rows
        .iter()
        .map(|row| (row.key.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Type", "Requirement"),
            ("ID", "REQ-T"),
            ("Aliases", "a"),
            ("Methods", "Test | Analysis")
        ]
    );
    assert_eq!(ids(&document), ["REQ-T", "REQ-U"]);

    // A metadata table with three columns.
    let text = item(A, "A").replace(
        "| Type | Requirement |\n| --- | --- |",
        "| Type | Requirement | X |\n| --- | --- | --- |",
    );
    assert_eq!(problems(&parse_text(&text)), [(5, "malformed-row")]);
}

#[test]
fn matches_labels_case_insensitively() {
    let text = item(A, "A").replace("| ID |", "| id |").replace(
        "| Status | Draft |",
        "| STATUS | Draft |\n| Derives From | [REQ-B](#01M7D8DZ2RBHGQBYAV7V3789H7) |",
    );
    let document = parse_text(&text);
    let names: Vec<_> = document.items[0]
        .rows
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(names, ["type", "id", "status", "derives_from"]);
    assert_eq!(label_name("Almost-certain"), "almost-certain");
    assert_eq!(label_name("Derives  from"), "derives__from");
}

#[test]
fn reads_relation_links_and_mentions() {
    let text = format!(
        "# Doc\n\nSee [the first](#{A}), [the web](https://example.com/a.mara.md#{A}) and \
         [an anchor](other.mara.md#intro).\n\n---\n\n{}",
        item(A, "A").replace(
            "| Status | Draft |\n\nBody of A.",
            &format!(
                "| Status | Draft |\n| Satisfies | [NEED-X](../needs.mara.md#{B}) |\n\
                 | Derives from | see [REQ-B](#{B}) |\n\
                 \nBody of A, after [b][ref].\n\n| Case | Target |\n| --- | --- |\n\
                 | One | [C](sub/c.mara.md#{C}) |\n\n[ref]: b.mara.md#{B}"
            ),
        )
    );
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    let narrative: Vec<_> = document
        .mentions
        .iter()
        .map(|link| (link.line, link.text.as_str()))
        .collect();
    assert_eq!(narrative, [(3, "the first")]);
    let item = &document.items[0];
    let row_links: Vec<_> = item
        .rows
        .iter()
        .map(|row| row.link.as_ref().map(|link| link.path.as_str()))
        .collect();
    assert_eq!(
        row_links,
        [None, None, None, Some("../needs.mara.md"), None]
    );
    let mentions: Vec<_> = item
        .mentions
        .iter()
        .map(|link| {
            (
                link.line,
                link.text.as_str(),
                link.path.as_str(),
                link.fragment.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        mentions,
        [
            (18, "b", "b.mara.md", Some(B)),
            (22, "C", "sub/c.mara.md", Some(C)),
        ]
    );
}

#[test]
fn reports_an_empty_body() {
    let text = item(A, "A").replace("\nBody of A.\n", "");
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(1, "empty-body")]);
    assert_eq!(document.items.len(), 1);
}

#[test]
fn counts_one_parse_per_document() {
    let mut work = WorkCounts::default();
    parse(item(A, "A").as_bytes(), &mut work);
    parse(b"\xff", &mut work);
    parse(b"", &mut work);
    assert_eq!(
        work,
        WorkCounts {
            parses: 3,
            ..WorkCounts::default()
        }
    );
}

#[test]
fn locates_problems_with_every_line_ending() {
    let text = format!("Intro\n===\n\n---\n\n{}\n{}", item(A, "A"), item(B, "B"));
    for ending in ["\n", "\r\n", "\r"] {
        let document = parse_text(&text.replace('\n', ending));
        assert_eq!(
            problems(&document),
            [(2, "setext-heading"), (17, "missing-separator")],
            "{ending:?}"
        );
        assert_eq!(document.items[1].rows[1].line, 23, "{ending:?}");
    }
}

#[test]
fn reports_a_document_that_is_not_utf8() {
    let mut bytes = item(A, "A").into_bytes();
    bytes.splice(45..45, [0xC3, 0x28]);
    let document = parse(&bytes, &mut WorkCounts::default());
    assert_eq!(problems(&document), [(3, "invalid-utf8")]);
    assert!(document.items.is_empty());
}

#[test]
fn never_panics() {
    let example = item(A, "A") + "\n---\n\n" + &item(B, "B");
    for end in 0..=example.len() {
        parse(&example.as_bytes()[..end], &mut WorkCounts::default());
    }
    let long_line = format!("{}{}", item(A, "A"), "word [link](#x) ".repeat(250_000));
    let inputs = [
        String::new(),
        "\n\r\n \t".to_owned(),
        "---".to_owned(),
        "---\n---".to_owned(),
        "| a | b |\n| - | - |".to_owned(),
        long_line,
        format!("{}{}", ">".repeat(50_000), item(A, "A")),
        "- ".repeat(20_000) + "x",
        "[".repeat(100_000) + &"](#x)".repeat(1000),
        format!("<a id=\"{A}\"></a>").repeat(10_000),
    ];
    for input in inputs {
        parse(input.as_bytes(), &mut WorkCounts::default());
    }
}

#[test]
fn measures_every_kind_of_link_in_full() {
    let text = format!("[REQ-T][]\n\n[REQ-T]: #{A}\n");
    let document = parse_text(&text);
    assert_eq!(document.mentions[0].span, 0..9);

    let links = [
        "[REQ-T][]".to_owned(),
        "[REQ-T][ref]".to_owned(),
        "[REQ-T]".to_owned(),
        format!("[REQ-T](#{A})"),
        format!("<https://example.com/#{A}>"),
    ];
    let definitions = format!("\n[REQ-T]: #{A}\n[ref]: b.mara.md#{A}\n");
    for link in &links {
        let text = format!("See {link}.\n{definitions}");
        let document = parse_text(&text);
        let found: Vec<_> = document
            .mentions
            .iter()
            .map(|m| &text[m.span.clone()])
            .collect();
        let expected: &[&str] = if link.starts_with("<https") {
            &[]
        } else {
            &[link]
        };
        assert_eq!(found, expected, "{link}");

        let text = item(B, "B").replace(
            "| Status | Draft |",
            &format!("| Status | Draft |\n| Satisfies | {link} |"),
        ) + &definitions;
        let document = parse_text(&text);
        assert_eq!(problems(&document), [], "{link}");
        let row = &document.items[0].rows[3];
        let found = row.link.as_ref().map(|link| &text[link.span.clone()]);
        assert_eq!(found, Some(link.as_str()), "{link}");
        assert_eq!(row.value, *link);
    }
}

#[test]
fn reports_anchor_lines_inside_html_blocks() {
    // The reviewer's inputs: attribute contents are no anchor; an anchor line is one, after a
    // `script` element or an attribute that looks like one.
    let anchor = format!("<a id=\"{A}\"></a>");
    for (text, expected) in [
        (format!("<div title='{anchor}'>\n</div>"), vec![]),
        (
            format!("<div title=\"<script>\">\n{anchor}\n</div>"),
            vec![(2, "merged-anchor")],
        ),
        (
            format!("<div>\n<script>text</script >\n{anchor}\n</div>"),
            vec![(3, "merged-anchor")],
        ),
        (
            format!("> <div>\n>   {anchor}\t\n> </div>\n"),
            vec![(2, "merged-anchor")],
        ),
        (
            format!("<pre>\r{anchor}\r</pre>\r"),
            vec![(2, "merged-anchor")],
        ),
        // An anchor line inside a comment or a script block counts, as the rule says.
        (format!("<!--\n{anchor}\n-->\n"), vec![(2, "merged-anchor")]),
        (
            format!("<script>\n{anchor}\n</script>\n"),
            vec![(2, "merged-anchor")],
        ),
        // An anchor sharing its line with other HTML does not.
        (
            format!("<div>\n<b>x</b> {anchor}\n<A ID=\"{A}\"></A>\n</div>\n"),
            vec![],
        ),
    ] {
        assert_eq!(problems(&parse_text(&text)), expected, "{text:?}");
    }
    // Nor does code.
    let text = format!("`{anchor}`\n\n```html\n{anchor}\n```\n\n    {anchor}\n");
    assert_eq!(problems(&parse_text(&text)), []);
}

#[test]
fn reports_an_end_tag_that_the_parser_misses() {
    let anchor = format!("<a id=\"{A}\"></a>");
    let tail = "\n\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n\
                | Status | Accepted |\n\nBody.";
    // The reviewer's input: the block swallows the rest, and nothing in it is reported.
    let text = format!("<script>\ntext\n</SCRIPT>\n\n***\n\n{anchor}{tail}");
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(3, "html-end-tag-case")]);
    assert!(document.items.is_empty());
    for (text, expected) in [
        (
            "<PRE class=\"x\">\ncode\n</Pre>\n".to_owned(),
            vec![(3, "html-end-tag-case")],
        ),
        (
            "<style>a {}</STYLE>\n".to_owned(),
            vec![(1, "html-end-tag-case")],
        ),
        (
            "<textarea>\n</TEXTAREA> and </TEXTAREA>\n".to_owned(),
            vec![(2, "html-end-tag-case")],
        ),
        // A lowercase end tag on the same line, or no end tag in another case, is fine.
        ("<script>\n</SCRIPT></script>\n".to_owned(), vec![]),
        (
            format!("<script>\nlet a;\n</script>\n\n***\n\n{anchor}{tail}"),
            vec![],
        ),
        // `<scripts>` opens no such block; another block's end tag is not its own.
        ("<scripts>\n</SCRIPTS>\n".to_owned(), vec![]),
        ("<pre>\n</SCRIPT>\n</pre>\n".to_owned(), vec![]),
    ] {
        assert_eq!(problems(&parse_text(&text)), expected, "{text:?}");
    }
}

#[test]
fn reports_a_missing_blank_line_after_an_anchor() {
    let text = format!(
        "<a id=\"{A}\"></a>\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n\
         | Status | Accepted |\n\nBody.\n"
    );
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(1, "merged-anchor")]);
    assert_eq!(ids(&document), ["REQ-T"]);
}

#[test]
fn reports_every_independent_problem_of_a_region() {
    let cases = [
        // A missing anchor with a malformed row, or with a `Type` row of too many cells.
        (
            "## T\n\n| Type | Requirement |\n| --- | --- |\n| Status | |\n\nBody.\n".to_owned(),
            vec![(1, "incomplete-item"), (5, "malformed-row")],
            vec![],
        ),
        (
            "## T\n\n| Key | Value |\n| --- | --- |\n| Type | Requirement | extra |\n\nBody.\n"
                .to_owned(),
            vec![(1, "incomplete-item"), (5, "malformed-row")],
            vec![],
        ),
        // A narrative table whose `Type` header has more than two cells is still a `Type` row.
        (
            "## Types\n\n| Type | Meaning | Since |\n| --- | --- | --- |\n\nText.\n".to_owned(),
            vec![(1, "incomplete-item"), (3, "malformed-row")],
            vec![],
        ),
        // No blank line after the anchor, a malformed row and no body.
        (
            format!("<a id=\"{A}\"></a>\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n| Status |\n"),
            vec![(1, "merged-anchor"), (1, "empty-body"), (7, "malformed-row")],
            vec!["REQ-T"],
        ),
        // A missing anchor, a setext heading, a malformed row and no body.
        (
            "Title\n=====\n\n| Type | Requirement |\n| --- | --- |\n| Aliases | a | b |\n".to_owned(),
            vec![(1, "empty-body"), (1, "incomplete-item"), (2, "setext-heading"), (6, "malformed-row")],
            vec![],
        ),
        // An anchor merged into the first paragraph, and a malformed row.
        (
            format!("Text <a id=\"{A}\"></a>\n\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n| Status | |\n\nBody.\n"),
            vec![(1, "merged-anchor"), (8, "malformed-row")],
            vec!["REQ-T"],
        ),
        // A missing separator, no blank line after the anchor and a malformed row.
        (
            format!("{}\n<a id=\"{B}\"></a>\n## U\n\n| Type | Requirement |\n| --- | --- |\n| Status |\n\nBody.\n", item(A, "A")),
            vec![(12, "missing-separator"), (12, "merged-anchor"), (17, "malformed-row")],
            vec!["REQ-A", ""],
        ),
        // A paragraph in the heading's place, a malformed row and no body.
        (
            format!("<a id=\"{A}\"></a>\n\nTitle\n\n| Type | Requirement |\n| --- | --- |\n| Status |\n"),
            vec![(1, "empty-body"), (1, "incomplete-item"), (7, "malformed-row")],
            vec![],
        ),
        // HTML that is not a MID anchor, and a malformed row.
        (
            "<a id=\"intro\"></a>\n\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID |\n\nBody.\n".to_owned(),
            vec![(1, "incomplete-item"), (7, "malformed-row")],
            vec![],
        ),
    ];
    for (text, expected, items) in cases {
        let document = parse_text(&text);
        assert_eq!(problems(&document), expected, "{text}");
        assert_eq!(ids(&document), items, "{text}");
    }
}

/// The reviewer's anchor and item tail.
const MID: &str = "01M7D8DZ2RBHGQBYAV7V3789H6";
const TAIL: &str = "\n\n## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n\
                    | Status | Accepted |\n\nBody.";

/// Asserts the `Item` contract: the span starts at an exact anchor of the item's MID and ends
/// with its last block, and the line is the anchor's.
fn assert_item_contract(text: &str, document: &Document) {
    let lines = LineIndex::new(text.as_bytes());
    for item in &document.items {
        let anchor = format!("<a id=\"{}\"></a>", item.mid);
        assert!(
            text[item.span.start..].starts_with(&anchor),
            "{item:?} in {text:?}"
        );
        assert_eq!(item.line, lines.line(item.span.start), "{item:?}");
        let (before, after) = text.split_at(item.span.end);
        assert!(!before.ends_with(char::is_whitespace), "{item:?}");
        assert!(
            after.is_empty() || after.starts_with(char::is_whitespace),
            "{item:?}"
        );
        assert!(item.body.start >= item.span.start && item.body.end == item.span.end);
    }
}

#[test]
fn recovers_only_from_an_exact_anchor_at_its_place() {
    let anchor = format!("<a id=\"{MID}\"></a>");
    // An opening tag alone is no anchor: no item, and nothing to report.
    let text = format!("Text <a id=\"{MID}\">{TAIL}");
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert!(document.items.is_empty());

    // A merged first anchor makes the region an item, with every other problem reported.
    let text = format!("{anchor} tail\n\n## T\n\n| ID | REQ-T |\n| --- | --- |\n| Status | |\n");
    let document = parse_text(&text);
    assert_eq!(
        problems(&document),
        [
            (1, "merged-anchor"),
            (1, "empty-body"),
            (1, "incomplete-item"),
            (7, "malformed-row")
        ]
    );
    assert!(document.items.is_empty());

    // The item starts at its anchor, and links before it are narrative.
    let text = format!("[See](#{A})\n{anchor}{TAIL}");
    let document = parse_text(&text);
    assert_eq!(problems(&document), [(2, "merged-anchor")]);
    let item = &document.items[0];
    let start = text.find("<a id").unwrap();
    assert_eq!((item.line, item.span.clone()), (2, start..text.len()));
    assert_eq!(document.mentions[0].text, "See");
    assert!(item.mentions.is_empty());
    assert_item_contract(&text, &document);
    let text = format!("Text\n{anchor}{TAIL}");
    let item = &parse_text(&text).items[0];
    assert_eq!((item.line, item.span.clone()), (2, 5..text.len()));

    // Two merged anchors, or one in a heading or another block, make no item and are reported
    // once each.
    for (text, expected) in [
        (
            format!("{anchor} <a id=\"{B}\"></a>{TAIL}"),
            vec![(1, "merged-anchor"), (1, "merged-anchor")],
        ),
        (
            format!("## T {anchor}\n\n| Type | Requirement |\n| --- | --- |\n| ID |\n\nBody.\n"),
            vec![(1, "merged-anchor"), (5, "malformed-row")],
        ),
        (format!("- {anchor}{TAIL}"), vec![(1, "merged-anchor")]),
        (
            format!("<div>\n{anchor}\n</div>{TAIL}"),
            vec![(2, "merged-anchor")],
        ),
    ] {
        let document = parse_text(&text);
        assert_eq!(problems(&document), expected, "{text}");
        assert!(document.items.is_empty(), "{text}");
    }
}

#[test]
fn an_anchor_needs_a_blank_line_after_it_even_at_the_end() {
    let anchor = format!("<a id=\"{MID}\"></a>");
    for text in [
        anchor.clone(),
        format!("{anchor}\n"),
        format!("{anchor}\r"),
        format!("{anchor}\n***\n"),
    ] {
        assert_eq!(
            problems(&parse_text(&text)),
            [(1, "merged-anchor"), (1, "incomplete-item")],
            "{text:?}"
        );
    }
    for text in [format!("{anchor}\n\n"), format!("{anchor}\r\n  \r\n")] {
        assert_eq!(
            problems(&parse_text(&text)),
            [(1, "incomplete-item")],
            "{text:?}"
        );
    }
}

#[test]
fn every_recovery_keeps_the_item_contract() {
    let anchor = |mid: &str| format!("<a id=\"{mid}\"></a>");
    let cases = [
        // Missing separator, with a malformed row in the recovered item.
        (
            format!(
                "{}\n{}\n\n## U\n\n| Type | Requirement |\n| --- | --- |\n| Status |\n\nBody.",
                item(A, "A"),
                anchor(B)
            ),
            vec![(12, "missing-separator"), (18, "malformed-row")],
            vec!["REQ-A", ""],
        ),
        // Merged first anchor.
        (
            format!("Text {}{TAIL}", anchor(MID)),
            vec![(1, "merged-anchor")],
            vec!["REQ-T"],
        ),
        // Missing blank line.
        (
            format!("{}\n## T\n{}", anchor(MID), &TAIL[7..]),
            vec![(1, "merged-anchor")],
            vec!["REQ-T"],
        ),
        // Setext heading read as the title.
        (
            format!("{}\n\nT\n={}", anchor(MID), &TAIL[6..]),
            vec![(4, "setext-heading")],
            vec!["REQ-T"],
        ),
        // Excess cells.
        (
            format!(
                "{}{}",
                anchor(MID),
                TAIL.replace("| ID | REQ-T |", "| ID | REQ-T | x |")
            ),
            vec![(7, "malformed-row")],
            vec!["REQ-T"],
        ),
        // Empty body.
        (
            format!("{}{}", anchor(MID), TAIL.replace("\n\nBody.", "")),
            vec![(1, "empty-body")],
            vec!["REQ-T"],
        ),
    ];
    for (text, expected, items) in cases {
        let document = parse_text(&text);
        assert_eq!(problems(&document), expected, "{text}");
        assert_eq!(ids(&document), items, "{text}");
        assert_item_contract(&text, &document);
    }
}

/// `text` with its line endings cycling through LF, lone CR and CRLF.
fn mixed_endings(text: &str) -> String {
    let endings = ["\n", "\r", "\r\n"];
    text.split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| match line.strip_suffix('\n') {
            Some(line) => format!("{line}{}", endings[index % 3]),
            None => line.to_owned(),
        })
        .collect()
}

/// `document` without byte spans, which CRLF line endings shift.
fn without_spans(mut document: Document) -> Document {
    let clear = |link: &mut Link| link.span = 0..0;
    for item in &mut document.items {
        (item.span, item.body) = (0..0, 0..0);
        item.rows
            .iter_mut()
            .filter_map(|row| row.link.as_mut())
            .for_each(clear);
        item.mentions.iter_mut().for_each(clear);
    }
    document.mentions.iter_mut().for_each(clear);
    document
}

#[test]
fn code_never_supplies_an_anchor_with_any_line_ending() {
    // The reviewer's input is the lone-CR form of the first case.
    let anchor = format!("<a id=\"{MID}\"></a>");
    for text in [
        format!("```\n{anchor}\n```"),
        format!("Text\n\n    {anchor}\n    more\n\nAfter.\n"),
        format!("Text `{anchor}` here.\n"),
    ] {
        for ending in ["\n", "\r\n", "\r"] {
            let text = text.replace('\n', ending);
            let document = parse_text(&text);
            assert_eq!(problems(&document), [], "{text:?}");
            assert!(document.items.is_empty(), "{text:?}");
        }
    }
}

#[test]
fn reads_a_document_alike_with_every_line_ending() {
    let text = format!(
        "---\ntitle: Whole\n---\n\n# Whole\n\nIntro with [a mention](#{A}).\n\n---\n\n\
         <a id=\"{A}\"></a>\n\n### First\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-FIRST |\n| Status | Accepted |\n| Satisfies | [NEED-X](needs.mara.md#{B}) |\n\n\
         Body with [the second](#{C}).\n\n```text\n---\n<a id=\"{A}\"></a>\n```\n\n\
         - one\n- two [link](other.mara.md#{B})\n\n> quoted\n> text\n\n\
         <details>\n<summary>More</summary>\n</details>\n\n---\n\n\
         <a id=\"{C}\"></a>\n\n### Second\n\n| Type | Design |\n| --- | --- |\n\
         | ID | DES-SECOND |\n| Status | Draft |\n| Satisfies | [REQ-FIRST][] |\n\nBody.\n\n\
         [REQ-FIRST]: #{A}\n"
    );
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    assert_eq!(ids(&document), ["REQ-FIRST", "DES-SECOND"]);
    let row_links = document
        .items
        .iter()
        .flat_map(|item| &item.rows)
        .filter(|row| row.link.is_some());
    assert_eq!(row_links.count(), 2);
    assert_eq!(
        document.mentions.len() + document.items[0].mentions.len(),
        3
    );
    // Lone CR keeps every byte offset, so the whole result is the same.
    assert_eq!(parse_text(&text.replace('\n', "\r")), document);
    assert_eq!(
        without_spans(parse_text(&mixed_endings(&text))),
        without_spans(document)
    );
}

#[test]
fn reports_each_problem_alike_with_every_line_ending() {
    let anchor = |mid: &str| format!("<a id=\"{mid}\"></a>");
    let cases = [
        (
            format!("---\ntitle: x\n{}\n---\n\nText.\n", anchor(A)),
            (3, "item-in-front-matter"),
        ),
        (
            "<script>\nx\n</SCRIPT>\n".to_owned(),
            (3, "html-end-tag-case"),
        ),
        (
            format!("{}\n{}", item(A, "A"), item(B, "B")),
            (12, "missing-separator"),
        ),
        (format!("# T\n\n- {}\n", anchor(A)), (3, "merged-anchor")),
        ("Title\n===\n\nText.\n".to_owned(), (2, "setext-heading")),
        (
            "## T\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-T |\n\nBody.\n".to_owned(),
            (1, "incomplete-item"),
        ),
        (
            item(A, "A").replace("\nBody of A.\n", ""),
            (1, "empty-body"),
        ),
        (
            item(A, "A").replace("| Status | Draft |", "| Status |"),
            (8, "malformed-row"),
        ),
    ];
    for (text, expected) in cases {
        for form in [text.clone(), text.replace('\n', "\r"), mixed_endings(&text)] {
            assert_eq!(problems(&parse_text(&form)), [expected], "{form:?}");
        }
    }
    for ending in [&b"\n"[..], b"\r", b"\r\n"] {
        let bytes = [&b"# T"[..], ending, ending, b"ok", ending, b"\xff", ending].concat();
        let document = parse(&bytes, &mut WorkCounts::default());
        assert_eq!(problems(&document), [(4, "invalid-utf8")], "{ending:?}");
    }
}

#[test]
fn normalizes_words_alike_for_documents_and_queries() {
    let normalized: Vec<String> = words("Hello, WORLD-wide 42x ÄBC _x_ façade").collect();
    assert_eq!(
        normalized,
        ["hello", "world", "wide", "42x", "äbc", "x", "façade"]
    );
}

#[test]
fn keeps_each_word_once_per_item_and_zone_in_byte_order() {
    let text = format!(
        "{}\n---\n\n{}",
        item(A, "Zeta zeta"),
        item(B, "Alpha").replace("Body of Alpha.", "Zeta and alpha, *alpha*.")
    );
    let document = parse_text(&text);
    let words: Vec<(&str, Vec<(usize, Zone)>)> = document
        .words
        .iter()
        .filter(|word| ["alpha", "zeta"].contains(&word.text.as_str()))
        .map(|word| {
            let postings = word.postings.iter().map(|p| (p.item, p.zone)).collect();
            (word.text.as_str(), postings)
        })
        .collect();
    assert_eq!(
        words,
        [
            (
                "alpha",
                vec![(1, Zone::Id), (1, Zone::Title), (1, Zone::Body)]
            ),
            (
                "zeta",
                vec![
                    (0, Zone::Id),
                    (0, Zone::Title),
                    (0, Zone::Body),
                    (1, Zone::Body)
                ]
            ),
        ]
    );
    let all: Vec<&str> = document
        .words
        .iter()
        .map(|word| word.text.as_str())
        .collect();
    let mut sorted = all.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(all, sorted);
}

#[test]
fn the_document_key_is_the_parser_version_and_the_file_hash() {
    let hash = crate::hash::Hash::of_file(b"x", &mut WorkCounts::default());
    let key = DocumentKey::new(hash);
    assert_eq!((key.parser_version, key.file), (PARSER_VERSION, hash));
}

/// The words of `zone` in the document's only item.
fn zone_words(document: &Document, zone: Zone) -> Vec<&str> {
    document
        .words
        .iter()
        .filter(|word| word.postings.iter().any(|posting| posting.zone == zone))
        .map(|word| word.text.as_str())
        .collect()
}

/// An item whose body is `body`, with `rows` after its `Type` row and `title` as its heading.
fn item_with(title: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{A}\"></a>\n\n### {title}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-X |\n{rows}\n{body}"
    )
}

#[test]
fn every_block_boundary_separates_words() {
    // The review's reproduction: a body of exactly two list entries.
    let document = parse_text(&item_with("T", "", "- alpha\n- beta\n"));
    assert_eq!(problems(&document), []);
    assert_eq!(zone_words(&document, Zone::Body), ["alpha", "beta"]);

    let body = "p1\n\np2\nsoft1\nsoft2  \nhard1\n\n#### head1\n\nafter1\n\n> quote1\n>\n> quote2\n\n\
                ```\ncode1\ncode2\n```\n\n- outer1\n  - inner1\n- outer2\n\n| c1 | c2 |\n| --- | --- |\n\
                | c3 | c4 |\n\nhtml1<br>html2\n\nem*ph*asis co`de`span li[n](x)k\n\n1. ord1\n2. ord2\n";
    let document = parse_text(&item_with("T", "", body));
    let mut expected = vec![
        "after1", "c1", "c2", "c3", "c4", "code1", "code2", "codespan", "emphasis", "hard1",
        "head1", "html1", "html2", "inner1", "link", "ord1", "ord2", "outer1", "outer2", "p1",
        "p2", "quote1", "quote2", "soft1", "soft2",
    ];
    expected.sort_unstable();
    assert_eq!(zone_words(&document, Zone::Body), expected);
}

#[test]
fn no_destination_or_html_is_indexed() {
    // The review's reproduction: a link inside other metadata text.
    let rows = "| Description | See [label](https://secret.example/hidden) now |\n\
                | Owner | [Whole](https://whole.example/x) |\n| Note | a <b>bold</b> c |\n";
    let body = "![alt words](https://img.example/pic.png) and [ref text][r], [r] and\n\
                <https://auto.example/page>.\n\n[r]: https://refdest.example/x\n";
    let text = item_with("Title [tl](https://titledest.example/y)", rows, body);
    let document = parse_text(&text);
    assert_eq!(problems(&document), []);
    let row = |name: &str| {
        let rows = &document.items[0].rows;
        rows.iter()
            .find(|row| row.name == name)
            .unwrap()
            .text
            .as_str()
    };
    assert_eq!(row("description"), "See label now");
    assert_eq!(row("owner"), "Whole");
    assert_eq!(row("note"), "a bold c");
    assert_eq!(
        zone_words(&document, Zone::Metadata),
        [
            "a",
            "bold",
            "c",
            "label",
            "now",
            "requirement",
            "see",
            "whole"
        ]
    );
    assert_eq!(zone_words(&document, Zone::Title), ["title", "tl"]);
    // An autolink's text is its address, which is what GitHub shows.
    assert_eq!(
        zone_words(&document, Zone::Body),
        [
            "alt", "and", "auto", "example", "https", "page", "r", "ref", "text", "words"
        ]
    );
    for hidden in [
        "secret",
        "hidden",
        "img",
        "pic",
        "png",
        "refdest",
        "titledest",
        "b",
    ] {
        assert!(
            document.words.iter().all(|word| word.text != hidden),
            "{hidden} is indexed"
        );
    }
}

/// Every word boundary of DES-SNAPSHOT in every place that search text is collected from: the
/// title, a metadata value and the body. `None` marks a place where Markdown cannot put that
/// boundary: a table cell and an ATX heading hold one line of inline content.
#[test]
fn every_boundary_separates_words_in_every_place() {
    type Places = [Option<&'static str>; 3];
    let boundaries: [(&str, Places); 6] = [
        ("block boundary", [None, None, Some("- alpha\n- beta")]),
        (
            "table cell boundary",
            [None, None, Some("| alpha | beta |\n| --- | --- |")],
        ),
        (
            "soft line break",
            [Some("alpha\nbeta\n==="), None, Some("alpha\nbeta")],
        ),
        (
            "hard line break",
            [Some("alpha\\\nbeta\n==="), None, Some("alpha  \nbeta")],
        ),
        (
            "inline HTML tag",
            [
                Some("### alpha<br>beta"),
                Some("alpha<br>beta"),
                Some("alpha<br>beta"),
            ],
        ),
        (
            "HTML block",
            [None, None, Some("- alpha\n  <!-- a comment -->\n  beta")],
        ),
    ];
    for (boundary, places) in boundaries {
        for (zone, snippet) in [Zone::Title, Zone::Metadata, Zone::Body]
            .into_iter()
            .zip(places)
        {
            let Some(snippet) = snippet else { continue };
            let text = match zone {
                Zone::Title => format!(
                    "<a id=\"{A}\"></a>\n\n{snippet}\n\n| Type | Requirement |\n| --- | --- |\n\
                     | ID | REQ-X |\n\nBody.\n"
                ),
                Zone::Metadata => {
                    item_with("T", &format!("| Description | {snippet} |\n"), "Body.\n")
                }
                _ => item_with("T", "", &format!("{snippet}\n")),
            };
            let document = parse_text(&text);
            assert_eq!(document.items.len(), 1, "{boundary} in {zone:?}: {text:?}");
            let words = zone_words(&document, zone);
            assert!(
                words.contains(&"alpha") && words.contains(&"beta"),
                "{boundary} in {zone:?}: {words:?}"
            );
            assert!(!words.contains(&"alphabeta"), "{boundary} in {zone:?}");
        }
    }
}

/// The review's reproduction: an unspaced HTML tag in a metadata value.
#[test]
fn an_html_tag_in_a_metadata_value_separates_words() {
    let document = parse_text(&item_with(
        "T",
        "| Description | alpha<br>beta |\n",
        "Body.\n",
    ));
    assert_eq!(document.items[0].rows[2].text, "alpha beta");
    assert_eq!(
        zone_words(&document, Zone::Metadata),
        ["alpha", "beta", "requirement"]
    );
}
