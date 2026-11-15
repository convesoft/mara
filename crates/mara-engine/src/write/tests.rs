//! Tests of the item writer: what it writes reads back as written, and splices leave every other
//! byte alone.

use std::sync::Arc;

use super::*;
use crate::document::{Document, DocumentKey, is_mid, parse};
use crate::hash::Hash;
use crate::schema::parse_schema;
use crate::snapshot::{DocumentInput, ParsedFile, SnapshotInputs};
use crate::validation;
use crate::work::WorkCounts;

const A: &str = "01M7D8DZ2663WP3A037SR2HJH2";
const B: &str = "01M7D8DZ2RBHGQBYAV7V3789H7";

fn parsed(text: &str) -> Document {
    parse(text.as_bytes(), &mut WorkCounts::default())
}

fn problems(document: &Document) -> Vec<(usize, &'static str)> {
    document
        .diagnostics("a.mara.md")
        .map(|diagnostic| (diagnostic.line, diagnostic.code))
        .collect()
}

/// `contents` after `splices`, read back by [`ReadBack::edit`], which must accept them.
fn edited(contents: &str, splices: &[Splice]) -> String {
    match try_edit(contents, splices) {
        Ok(text) => text,
        Err(misread) => panic!("{misread:?} in {contents:?}"),
    }
}

fn spliced(contents: &str, splice: &Splice) -> String {
    edited(contents, std::slice::from_ref(splice))
}

fn try_edit(contents: &str, splices: &[Splice]) -> Result<String, Misread> {
    let old = ParsedFile::parse(contents.as_bytes().to_vec(), &mut WorkCounts::default());
    let mut work = WorkCounts::default();
    let new = ReadBack::default().edit(&old, splices, &mut work)?;
    assert_eq!(work.parses, 1, "one parse reads an edit back");
    Ok(String::from_utf8(new.contents.into_vec()).unwrap())
}

/// An item of type `Requirement` with ID `REQ-<name>`, written with `eol`.
fn written(mid: &str, name: &str, eol: &str) -> String {
    let rows = [
        row("type", &label("requirement")),
        row("id", &cell(&format!("REQ-{name}")).unwrap()),
        row("status", &label("draft")),
    ];
    item(mid, 3, name, &rows, &format!("Body of {name}."), eol).unwrap()
}

fn titles(document: &Document) -> Vec<&str> {
    document
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect()
}

#[test]
fn writes_ulids_from_the_time_and_random_bits_given() {
    assert_eq!(mid(0, [0; 10]), "0".repeat(26));
    assert_eq!(mid(u64::MAX, [0xFF; 10]), format!("7{}", "Z".repeat(25)));
    // The AGENTS.md generator's encoding of (millis << 80) | random.
    let random = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    assert_eq!(mid(1_763_200_000_000, random), "01KA3EHR00041061050R3GG28A");
    assert!(is_mid(&mid(1_763_200_000_000, random)));
    assert!(mid(2, [0; 10]) > mid(1, [0xFF; 10]));
}

#[test]
fn writes_labels_cells_and_headings_that_read_back() {
    assert_eq!(label("derives_from"), "Derives from");
    assert_eq!(label("almost-certain"), "Almost-certain");
    assert_eq!(label(""), "");
    assert_eq!(cell("a | b").as_deref(), Ok("a \\| b"));
    for (value, refused) in [
        ("", Unwritable::Empty),
        ("a\nb", Unwritable::LineBreak),
        ("a\rb", Unwritable::LineBreak),
        (" a", Unwritable::Spaces),
        ("a\t", Unwritable::Spaces),
    ] {
        assert_eq!(cell(value), Err(refused), "{value:?}");
        assert_eq!(heading(2, value), Err(refused), "{value:?}");
    }

    let values = [
        "a | b",
        "x|y|",
        "*emphasis* and `code`",
        "[not a link",
        "a\\b",
        "a\\|b",
    ];
    let titles_in = [
        "Plain",
        "Ends with #",
        "#",
        "C#",
        "Use `code` *here*",
        "## Two",
    ];
    for (value, title) in values.iter().zip(titles_in) {
        let rows = [
            row("type", &label("requirement")),
            row("id", "REQ-A"),
            row("status", "Draft"),
            row("note", &cell(value).unwrap()),
        ];
        let text = item(A, 2, title, &rows, "Body.", "\n").unwrap();
        let document = parsed(&text);
        assert_eq!(problems(&document), [], "{text}");
        let [read] = &document.items[..] else {
            panic!("one item: {text}");
        };
        assert_eq!(read.mid, A);
        assert_eq!(read.value("note"), Some(*value), "{text}");
        let expected = title.replace(['*', '`'], "");
        assert_eq!(read.title, expected, "{text}");
    }
}

#[test]
fn puts_the_type_row_in_the_header() {
    let rows = [
        row("id", "REQ-A"),
        row("type", "Requirement"),
        row("status", "Draft"),
    ];
    assert_eq!(
        table(&rows, "\n"),
        "| Type | Requirement |\n| --- | --- |\n| ID | REQ-A |\n| Status | Draft |"
    );
}

#[test]
fn writes_relation_links_that_resolve_and_read_as_written() {
    assert_eq!(relative("docs/a.mara.md", "docs/a.mara.md"), "");
    assert_eq!(relative("docs/a.mara.md", "docs/b.mara.md"), "b.mara.md");
    assert_eq!(
        relative("docs/x/a.mara.md", "docs/y/b.mara.md"),
        "../y/b.mara.md"
    );
    assert_eq!(relative("a.mara.md", "docs/b.mara.md"), "docs/b.mara.md");
    assert_eq!(relative("docs/a.mara.md", "b.mara.md"), "../b.mara.md");
    assert_eq!(
        relative("a.mara.md", "my notes (draft)#1.mara.md"),
        "my%20notes%20%28draft%29%231.mara.md"
    );
    // A `:` would make the part before it read as a URL scheme, so the link would be no mention.
    assert_eq!(
        relative("a.mara.md", "foo:bar.mara.md"),
        "foo%3Abar.mara.md"
    );
    assert_eq!(
        link("REQ-B", B, "a.mara.md", "a.mara.md"),
        format!("[REQ-B](#{B})")
    );
    assert_eq!(
        link("REQ_[B]", B, "a.mara.md", "a.mara.md"),
        format!("[REQ\\_\\[B\\]](#{B})")
    );

    // A link from every document to an item in each of these paths is no stale link, and the
    // project the writer wrote is valid.
    let schema_text = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\n\
        id_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n[relations.refines]\n\
        inverse = \"refined_by\"\ndescription = \"Refines.\"\n\
        targets = { requirement = [\"requirement\"] }\n";
    let (schema, problems) = parse_schema(".mara/schema.toml", schema_text.as_bytes());
    assert_eq!(problems, []);
    let paths = [
        "docs/a.mara.md",
        "docs/sub dir/b c.mara.md",
        "docs/odd/#%?&|<>().mara.md",
        "docs/odd/\\back.mara.md",
        "notes/é ü.mara.md",
        "top.mara.md",
        "foo:bar.mara.md",
        "docs/c:d/e:f.mara.md",
    ];
    let mids: Vec<String> = (0..paths.len()).map(|n| mid(n as u64, [7; 10])).collect();
    let mut work = WorkCounts::default();
    let documents = paths
        .iter()
        .enumerate()
        .map(|(n, path)| {
            let mut rows = vec![
                row("type", "Requirement"),
                row("id", &format!("REQ-{n}")),
                row("status", "Draft"),
            ];
            for (m, to) in paths.iter().enumerate().filter(|&(m, _)| m != n) {
                rows.push(row(
                    "refines",
                    &link(&format!("REQ-{m}"), &mids[m], path, to),
                ));
            }
            // The same links as mentions in the body.
            let body: Vec<String> = paths
                .iter()
                .enumerate()
                .filter(|&(m, _)| m != n)
                .map(|(m, to)| format!("See {}.", link(&format!("REQ-{m}"), &mids[m], path, to)))
                .collect();
            let mut text = String::new();
            let new = item(&mids[n], 3, "Linked", &rows, &body.join("\n\n"), "\n").unwrap();
            text = spliced(&text, &insert(&text, 0, &new));
            let document = parsed(&text);
            assert_eq!(document.items[0].mentions.len(), paths.len() - 1, "{text}");
            DocumentInput {
                path: (*path).to_owned(),
                key: DocumentKey::new(Hash::of_file(text.as_bytes(), &mut work)),
                file: Arc::new(ParsedFile::parse(text.into_bytes(), &mut work)),
            }
        })
        .collect();
    let schema = Some(Arc::new(schema.unwrap()));
    let snapshot = SnapshotInputs::new(documents, schema, None, None).build(&mut work);
    assert_eq!(snapshot.items().len(), paths.len());
    let findings: Vec<_> = validation::check(&snapshot, &mut work)
        .into_iter()
        .map(|finding| finding.diagnostic)
        .collect();
    assert_eq!(findings, []);
}

#[test]
fn inserts_an_item_as_its_own_region_and_never_starts_a_file_with_a_break() {
    let new = |eol: &str| written(B, "New", eol);
    let narrative = "# Page\n\nIntro.\n";
    let one = format!("# Page\n\n---\n\n{}\n", written(A, "Old", "\n"));
    let crlf = one.replace('\n', "\r\n");
    let cases: Vec<(&str, String, usize, Vec<&str>)> = vec![
        ("empty", String::new(), 0, vec!["New"]),
        (
            "narrative, at its end",
            narrative.into(),
            narrative.len(),
            vec!["New"],
        ),
        (
            "narrative, no final newline",
            "Intro.".into(),
            6,
            vec!["New"],
        ),
        ("narrative, at its start", narrative.into(), 0, vec!["New"]),
        ("after an item", one.clone(), one.len(), vec!["Old", "New"]),
        (
            "before an item, after its break",
            one.clone(),
            12,
            vec!["New", "Old"],
        ),
        (
            "before the break of an item",
            one.clone(),
            8,
            vec!["New", "Old"],
        ),
        (
            "CRLF, after an item",
            crlf.clone(),
            crlf.len(),
            vec!["Old", "New"],
        ),
        ("byte order mark", "\u{feff}Intro.\n".into(), 0, vec!["New"]),
        (
            "front matter",
            "---\ntitle: x\n---\nIntro.\n".into(),
            0,
            vec!["New"],
        ),
        ("front matter, empty", "---\n---\n".into(), 0, vec!["New"]),
        (
            "front matter, no final newline",
            "---\nx: 1\n---".into(),
            12,
            vec!["New"],
        ),
        (
            "front matter, blanks without final newline",
            "---\nx: 1\n---\n  ".into(),
            15,
            vec!["New"],
        ),
        ("byte order mark alone", "\u{feff}".into(), 3, vec!["New"]),
        (
            "a dangling break",
            "Intro.\n\n---\n".into(),
            12,
            vec!["New"],
        ),
    ];
    for (name, contents, at, expected) in cases {
        let eol = line_ending(&contents);
        let splice = insert(&contents, at, &new(eol));
        let text = spliced(&contents, &splice);
        let document = parsed(&text);
        assert_eq!(problems(&document), [], "{name}: {text:?}");
        assert_eq!(titles(&document), expected, "{name}: {text:?}");
        let markdown = text.trim_start_matches('\u{feff}');
        let front_matter = contents.trim_start_matches('\u{feff}').starts_with("---\n");
        assert!(
            front_matter || !markdown.starts_with("---"),
            "{name}: {text:?}"
        );
        let added = splice.text.replace("\r\n", "\n");
        assert!(
            !added.contains("---\n\n---") && !added.contains("\n\n\n"),
            "{name}: {text:?}"
        );
        if eol == "\r\n" {
            assert!(!text.replace("\r\n", "").contains('\n'), "{name}: {text:?}");
        }
        // Only bytes are added, at the line start given or after the front matter.
        assert!(splice.range.is_empty(), "{name}");
        assert_eq!(text.len(), contents.len() + splice.text.len(), "{name}");
    }
}

#[test]
fn removes_an_item_with_one_break_and_never_leaves_a_break_first() {
    let a = written(A, "First", "\n");
    let b = written(B, "Second", "\n");
    let c = written("01M7D8DZ39AMW9NPW9NA17D4S4", "Third", "\n");
    let three = format!("# Page\n\n---\n\n{a}\n\n---\n\n{b}\n\n---\n\n{c}\n");
    let first = format!("{a}\n\n---\n\n{b}\n");
    let cases = [
        (
            "first of three",
            three.clone(),
            0,
            vec!["Second", "Third"],
            "# Page\n",
        ),
        (
            "middle",
            three.clone(),
            1,
            vec!["First", "Third"],
            "# Page\n",
        ),
        (
            "last",
            three.clone(),
            2,
            vec!["First", "Second"],
            "# Page\n",
        ),
        (
            "first in the file",
            first.clone(),
            0,
            vec!["Second"],
            "<a id=",
        ),
        (
            "last after a first",
            first.clone(),
            1,
            vec!["First"],
            "<a id=",
        ),
        ("only", format!("{a}\n"), 0, vec![], ""),
        (
            "no final newline",
            format!("Intro.\n\n---\n\n{a}"),
            0,
            vec![],
            "Intro.",
        ),
        (
            "after front matter",
            format!("---\nx: 1\n---\n\n{a}\n\n---\n\n{b}\n"),
            0,
            vec!["Second"],
            "---\nx: 1\n---\n\n<a id=",
        ),
        (
            "CRLF",
            three.replace('\n', "\r\n"),
            1,
            vec!["First", "Third"],
            "# Page\r\n",
        ),
    ];
    for (name, contents, index, expected, start) in cases {
        let span = parsed(&contents).items[index].span.clone();
        let text = spliced(&contents, &remove(&contents, span));
        let document = parsed(&text);
        assert_eq!(problems(&document), [], "{name}: {text:?}");
        assert_eq!(titles(&document), expected, "{name}: {text:?}");
        assert!(text.starts_with(start), "{name}: {text:?}");
        assert!(
            !text.contains("---\n\n---") && !text.ends_with("\n\n"),
            "{name}: {text:?}"
        );
    }
}

#[test]
fn edits_rows_in_place_with_the_table_line_endings() {
    let contents = format!(
        "<a id=\"{A}\"></a>\r\n\r\n## A\r\n\r\n|Type|Requirement|\r\n|---|---|\r\n|  ID   |REQ-A|\r\n\
         | Refines | [REQ-B](#{B}) |\r\n\r\nBody.\r\n"
    );
    let document = parsed(&contents);
    let item = &document.items[0];
    let [_, id, refines] = &item.rows[..] else {
        panic!("three rows");
    };

    let text = spliced(&contents, &set_value(id, &cell("REQ-RENAMED").unwrap()));
    assert_eq!(text, contents.replace("|REQ-A|", "|REQ-RENAMED|"));

    let text = spliced(&contents, &add_row(&contents, item, "status", "Draft"));
    let expected = contents.replace(
        &format!("(#{B}) |\r\n"),
        &format!("(#{B}) |\r\n| Status | Draft |\r\n"),
    );
    assert_eq!(text, expected);
    assert_eq!(parsed(&text).items[0].value("status"), Some("Draft"));

    let text = spliced(&contents, &remove_row(&contents, item, refines).unwrap());
    assert_eq!(
        text,
        contents.replace(&format!("| Refines | [REQ-B](#{B}) |\r\n"), "")
    );
    // The last row of a file without a final line ending: the file still ends without one.
    let last = contents[..contents.find("\r\n\r\nBody.").unwrap()].to_owned();
    let document = parsed(&last);
    let rows = &document.items[0].rows;
    let text = spliced(
        &last,
        &remove_row(&last, &document.items[0], &rows[2]).unwrap(),
    );
    assert_eq!(
        text,
        last.replace(&format!("\r\n| Refines | [REQ-B](#{B}) |"), "")
    );

    let text = spliced(&contents, &remove_row(&contents, item, id).unwrap());
    assert_eq!(text, contents.replace("|  ID   |REQ-A|\r\n", ""));

    // The header row's place takes the next row's text, here after a relation that an author
    // put first.
    let first = format!(
        "<a id=\"{A}\"></a>\n\n## A\n\n| Refines | [REQ-B](#{B}) |\n|---|---|\n\
         | Type | Requirement |\n| ID | REQ-A |\n\nBody.\n"
    );
    let document = parsed(&first);
    let item = &document.items[0];
    let text = spliced(&first, &remove_row(&first, item, &item.rows[0]).unwrap());
    let expected = first.replace(
        &format!("| Refines | [REQ-B](#{B}) |\n|---|---|\n| Type | Requirement |\n"),
        "| Type | Requirement |\n|---|---|\n",
    );
    assert_eq!(text, expected);
    let document = parsed(&text);
    assert_eq!(problems(&document), []);
    let names: Vec<&str> = document.items[0]
        .rows
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(names, ["type", "id"]);
    let alone =
        format!("<a id=\"{A}\"></a>\n\n## A\n\n| Type | Requirement |\n| --- | --- |\n\nBody.\n");
    let document = parsed(&alone);
    let item = &document.items[0];
    assert_eq!(remove_row(&alone, item, &item.rows[0]), None);
}

#[test]
fn finds_the_line_ending_of_the_first_line() {
    assert_eq!(line_ending(""), "\n");
    assert_eq!(line_ending("a"), "\n");
    assert_eq!(line_ending("a\r\nb\n"), "\r\n");
    assert_eq!(line_ending("a\nb\r\n"), "\n");
    assert_eq!(line_ending("a\rb"), "\r");
}

/// Review round 1, finding 2: a merged anchor's item is removed without the narrative before it.
#[test]
fn removes_a_merged_anchor_item_and_keeps_the_text_before_it() {
    let merged = format!(
        "Narrative to keep. <a id=\"{A}\"></a>\n\n### First\n\n| Type | Requirement |\n\
         | --- | --- |\n| ID | REQ-A |\n\nBody.\n"
    );
    let second = written(B, "Second", "\n");
    for (contents, expected) in [
        (merged.clone(), "Narrative to keep.\n".to_owned()),
        (
            format!("# Page\n\n---\n\n{merged}\n---\n\n{second}\n"),
            format!("# Page\n\n---\n\nNarrative to keep.\n\n---\n\n{second}\n"),
        ),
        (
            merged.trim_end().to_owned(),
            "Narrative to keep.".to_owned(),
        ),
    ] {
        let document = parsed(&contents);
        let first = document.items.iter().find(|item| item.mid == A).unwrap();
        let text = spliced(&contents, &remove(&contents, first.span.clone()));
        assert_eq!(text, expected);
    }
}

/// A line right after the removed item keeps a blank line before it, so an anchor without its
/// separator does not merge into the text before, nor a `---` turn that text into a heading.
#[test]
fn removes_an_item_without_joining_the_lines_around_it() {
    let second = written(B, "Second", "\n");
    let fenced = format!(
        "<a id=\"{A}\"></a>\n\n### First\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-A |\n\n```text\ncode\n```\n"
    );
    let table = format!(
        "<a id=\"{A}\"></a>\n\n### First\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-A |\n\n| Body | table |\n| --- | --- |\n| a | b |\n"
    );
    for (contents, expected) in [
        (
            format!("Intro.\n\n---\n\n{fenced}{second}\n"),
            format!("Intro.\n\n{second}\n"),
        ),
        (
            format!("Intro.\n\n---\n\n{table}---\n\n{second}\n"),
            format!("Intro.\n\n---\n\n{second}\n"),
        ),
    ] {
        let document = parsed(&contents);
        let text = spliced(
            &contents,
            &remove(&contents, document.items[0].span.clone()),
        );
        assert_eq!(text, expected);
    }
}

/// Review round 1, finding 3: an item that a block before it would take in is refused, not
/// written where it would silently not be read.
#[test]
fn refuses_an_inserted_item_that_would_not_read_back() {
    let new = written(B, "New", "\n");
    for open in [
        "Intro.\n\n```text\nunclosed code\n",
        "Intro.\n\n<!-- an unclosed comment\n",
        "Intro.\n\n<script>\nlet unclosed;\n",
        "Intro.\n\n<pre>\nunclosed\n",
    ] {
        let misread = try_edit(open, &[insert(open, open.len(), &new)]).unwrap_err();
        assert_eq!(misread.line, open.lines().count() + 1, "{open:?}");
        assert!(
            misread
                .message
                .contains("would not be read where it is written"),
            "{misread:?}"
        );
        let diagnostic = misread.diagnostic("a.mara.md");
        assert_eq!(
            (diagnostic.code, diagnostic.path.as_str()),
            (UNREADABLE_EDIT, "a.mara.md")
        );
    }
}

/// The read-back guard covers every splice, the operations' own ones included.
#[test]
fn refuses_every_edit_that_does_not_read_back_as_written() {
    let a = written(A, "First", "\n");
    let b = written(B, "Second", "\n");
    let two = format!("Intro.\n\n---\n\n{a}\n\n---\n\n{b}\n");
    let document = parsed(&two);
    let (first, second) = (&document.items[0], &document.items[1]);
    let at = |text: &str| two.find(text).unwrap();
    let bytes = |range: Range<usize>, text: &str| Splice {
        range,
        text: text.to_owned(),
        expect: Expect::Bytes,
    };
    let cases: Vec<(&str, Vec<Splice>, &str)> = vec![
        (
            "a removal over two items",
            vec![Splice {
                range: first.span.start..second.span.end,
                text: String::new(),
                expect: Expect::Removal,
            }],
            "would take 2 items",
        ),
        (
            "a removal that takes narrative",
            vec![Splice {
                range: 0..first.span.end,
                text: String::new(),
                expect: Expect::Removal,
            }],
            "narrative",
        ),
        (
            "a splice across an item's end",
            vec![bytes(first.span.end - 3..first.span.end + 3, "x")],
            "would add a problem, `missing-separator`",
        ),
        (
            "an item's table broken",
            vec![bytes(at("| --- | --- |")..at("| --- | --- |") + 13, "text")],
            "would no longer read as an item",
        ),
        (
            "a value with a bare `|`",
            vec![set_value(&first.rows[1], "REQ-A | extra")],
            "metadata rows",
        ),
        (
            "narrative turned into a heading",
            vec![bytes(at("Intro.")..at("Intro.") + 6, "Intro.\n===")],
            "would add a problem, `setext-heading`",
        ),
        (
            "an anchor written into narrative",
            vec![bytes(
                at("Intro.")..at("Intro.") + 6,
                &format!("<a id=\"{B}\"></a> Intro."),
            )],
            "would add a problem, `merged-anchor`",
        ),
    ];
    for (name, splices, expected) in cases {
        let misread = try_edit(&two, &splices).unwrap_err();
        assert!(misread.message.contains(expected), "{name}: {misread:?}");
    }
}

/// Review round 1, finding 4: the row that replaces a removed header row gets a delimiter row
/// with as many columns as it has cells.
#[test]
fn removes_a_header_row_whose_columns_differ_from_the_next_row() {
    let contents = format!(
        "<a id=\"{A}\"></a>\n\n## A\n\n| Refines | [REQ-B](#{B}) | x |\n|:---|---|---|\n\
         | Type | Requirement |\n| ID | REQ-A |\n\nBody.\n"
    );
    let document = parsed(&contents);
    assert_eq!(problems(&document), [(5, "malformed-row")]);
    let item = &document.items[0];
    let text = spliced(
        &contents,
        &remove_row(&contents, item, &item.rows[0]).unwrap(),
    );
    let expected = contents.replace(
        &format!("| Refines | [REQ-B](#{B}) | x |\n|:---|---|---|\n| Type | Requirement |\n"),
        "| Type | Requirement |\n| --- | --- |\n",
    );
    assert_eq!(text, expected);
    let names: Vec<String> = parsed(&text).items[0]
        .rows
        .iter()
        .map(|row| row.name.clone())
        .collect();
    assert_eq!(names, ["type", "id"]);
    assert_eq!(cells("| a | b \\| c |"), 2);
    assert_eq!(cells("a | b | c"), 3);
}

/// Every editing function on items that load although they are not well formed: each edit reads
/// back, except the two that would lose something and are refused.
#[test]
fn edits_items_that_load_although_they_are_not_well_formed() {
    let anchor = format!("<a id=\"{A}\"></a>");
    let table = "| Type | Requirement |\n| --- | --- |\n| ID | REQ-A |\n| Tags | x |\n";
    let item_with = |table: &str, body: &str| format!("{anchor}\n\n## A\n\n{table}{body}");
    let plain = item_with(table, "\nBody.\n");
    let documents: Vec<(&str, String)> = vec![
        ("no final newline", item_with(table.trim_end(), "")),
        ("trailing spaces", plain.replace(" |\n", " |   \n")),
        (
            "three columns",
            item_with(
                "| Type | Requirement | z |\n| --- | --- | --- |\n| ID | REQ-A |\n| Tags | x | y |\n",
                "\nBody.\n",
            ),
        ),
        (
            "excess cells",
            plain.replace("| REQ-A |", "| REQ-A | extra |"),
        ),
        ("merged anchor", format!("Lead text {plain}")),
        (
            "a line right after the table",
            item_with(table, "# Heading\n\nBody.\n"),
        ),
        ("CRLF", plain.replace('\n', "\r\n")),
        ("lone CR", plain.replace('\n', "\r")),
        ("front matter", format!("---\nx: 1\n---\n{plain}")),
        (
            "unclosed fence in the body",
            item_with(table, "\n```\ncode\n"),
        ),
        (
            "indented table",
            item_with(
                &table.replace('|', "  |").replace("  |  |", "  | |"),
                "\nBody.\n",
            ),
        ),
        (
            "table without outer pipes",
            item_with(
                "Type | Requirement\n--- | ---\nID | REQ-A\nTags | x\n",
                "\nBody.\n",
            ),
        ),
        ("missing separator", format!("Intro.\n\n{plain}")),
        ("byte order mark", format!("\u{feff}{plain}")),
    ];
    for (name, contents) in &documents {
        let document = parsed(contents);
        let item = &document.items[0];
        let row = |key: &str| item.rows.iter().find(|row| row.name == key).unwrap();
        let new = written(B, "New", line_ending(contents));
        let edits = [
            ("add_row", Some(add_row(contents, item, "status", "Draft"))),
            ("set_value", Some(set_value(row("id"), "REQ-B"))),
            (
                "set_value of the last row",
                Some(set_value(row("tags"), "y")),
            ),
            ("remove_row", remove_row(contents, item, row("id"))),
            (
                "remove_row of the last row",
                remove_row(contents, item, row("tags")),
            ),
            ("remove", Some(remove(contents, item.span.clone()))),
            (
                "insert at the end",
                Some(insert(contents, contents.len(), &new)),
            ),
            ("insert at the start", Some(insert(contents, 0, &new))),
            // The header is the `Type` row, without which the item is no item.
            (
                "remove_row of the header",
                remove_row(contents, item, &item.rows[0]),
            ),
        ];
        for (edit, splice) in edits {
            let result = try_edit(contents, &[splice.unwrap()]);
            let refused = edit == "remove_row of the header"
                || (*name == "unclosed fence in the body" && edit == "insert at the end");
            assert_eq!(result.is_err(), refused, "{name}, {edit}: {result:?}");
        }
    }
}

/// An item inserted at every line start outside an item reads back, except where it would
/// split a block or a link reference definition, where it is refused; and every item is removed.
/// Each line of the documents says whether it continues the block or definition before it.
#[test]
fn inserts_at_every_line_and_removes_every_item() {
    let a = written(A, "First", "\n");
    let b = written(B, "Second", "\n");
    let mixed: Vec<(&str, bool)> = vec![
        ("# Page", false),
        ("", false),
        ("Intro with [m](#01M7D8DZ2663WP3A037SR2HJH2) and", false),
        ("a lazy line.", true),
        ("", false),
        ("- list", false),
        ("- items", true),
        ("", false),
        // Another bullet starts another list.
        ("* loose", false),
        ("", true),
        ("* list", true),
        ("", false),
        ("> quote", false),
        ("> more", true),
        ("", false),
        ("```text", false),
        ("code", true),
        ("```", true),
        ("", false),
        ("<details>", false),
        ("<summary>x</summary>", true),
        ("</details>", true),
        ("", false),
        ("Setext", false),
        ("===", true),
        ("", false),
        ("| a | b |", false),
        ("|---|---|", true),
        ("| 1 | 2 |", true),
        ("", false),
        ("[d]: #01M7D8DZ2RBHGQBYAV7V3789H7", false),
        ("\"Title\"", true),
        ("", false),
        ("See [d] here.", false),
        ("", false),
        ("---", false),
        ("", false),
        (&a, false),
        ("", false),
        ("---", false),
        ("", false),
        ("Closing.", false),
        ("", false),
        ("---", false),
        ("", false),
        (&b, false),
    ];
    let front = format!("---\nt: 1\n---\n\n{a}\n\n***\n\n{b}\n\n___\n\nEnd");
    let documents = [mixed, vec![(front.as_str(), false)]];
    for entries in &documents {
        let mut text = String::new();
        let mut continues = Vec::new();
        for (entry, flag) in entries {
            for (index, line) in entry.split('\n').enumerate() {
                continues.push(index == 0 && *flag);
                text.push_str(line);
                text.push('\n');
            }
        }
        let endings = [
            text.clone(),
            text.replace('\n', "\r\n"),
            text.replace('\n', "\r"),
        ];
        for contents in endings {
            let document = parsed(&contents);
            for item in &document.items {
                let removed = try_edit(&contents, &[remove(&contents, item.span.clone())]);
                assert!(removed.is_ok(), "{}: {removed:?}", item.title);
            }
            let eol = line_ending(&contents);
            let starts =
                std::iter::once(0).chain(contents.match_indices(eol).map(|(at, _)| at + eol.len()));
            for (line, at) in starts.enumerate().take(continues.len()) {
                if document
                    .items
                    .iter()
                    .any(|item| item.span.start < at && at <= item.span.end)
                {
                    continue;
                }
                let new = written("01M7D8DZ39AMW9NPW9NA17D4S4", "New", eol);
                let result = try_edit(&contents, &[insert(&contents, at, &new)]);
                let shown = contents[at..].lines().next().unwrap_or_default();
                assert_eq!(
                    result.is_err(),
                    continues[line],
                    "line {} {shown:?}: {result:?}",
                    line + 1
                );
                if let Err(misread) = result {
                    assert!(
                        misread.message.contains("would split the block"),
                        "{misread:?}"
                    );
                }
            }
        }
    }
}

/// An item of type `Requirement` with ID `REQ-<name>`, the `rows` after its built-in ones, and
/// `body`.
fn with(mid: &str, name: &str, rows: &[String], body: &str) -> String {
    let mut all = vec![
        row("type", "Requirement"),
        row("id", &format!("REQ-{name}")),
        row("status", "Draft"),
    ];
    all.extend_from_slice(rows);
    item(mid, 3, name, &all, body, "\n").unwrap()
}

const C: &str = "01M7D8DZ39AMW9NPW9NA17D4S4";

/// Review round 2, defect 2, and the ordinary items like it: an item with any well-formed body
/// is inserted into any document and then edited, and reads back as written, the bytes that
/// its last block leaves out, such as definitions and trailing spaces, included.
#[test]
fn inserts_and_edits_items_with_every_kind_of_body() {
    let bodies = [
        "Plain body.".to_owned(),
        "See [site][r].\n\n[r]: https://example.invalid".to_owned(),
        "Ends with two spaces.  ".to_owned(),
        "A hard break  \nand a backslash one\\\nhere.".to_owned(),
        "<details>\n<summary>More</summary>\n\nHidden.\n\n</details>".to_owned(),
        "- one\n- two\n  - nested\n\n1. first\n2. second".to_owned(),
        "```rust\nfn main() {}\n```\n\n    indented code".to_owned(),
        "> quote\n>\n> > nested\n> > more".to_owned(),
        format!("Links to [the first](#{A}) and [by label][first].\n\n[first]: #{A}"),
        "| a | b |\n| --- | --- |\n| 1 | 2 |".to_owned(),
        "Text with *emphasis*, `code` and <span>HTML</span>.".to_owned(),
        "A footnote[^1].\n\n[^1]: The note.".to_owned(),
        "A definition with a title ends it.\n\n[t]: https://example.invalid \"Title\"".to_owned(),
    ];
    let first = with(A, "First", &[], "Body of the first.");
    let hosts = [
        String::new(),
        "Intro.".to_owned(),
        format!("# Page\n\n---\n\n{first}\n"),
        format!("# Page\n\n---\n\n{first}").replace('\n', "\r\n"),
        format!("# Page\n\n---\n\n{first}\n").replace('\n', "\r"),
    ];
    for body in &bodies {
        for host in &hosts {
            let eol = line_ending(host);
            let relation = row("refines", &link("REQ-FIRST", A, "a.mara.md", "a.mara.md"));
            let mut all = vec![
                row("type", "Requirement"),
                row("id", "REQ-NEW"),
                row("status", "Draft"),
            ];
            all.push(relation);
            let new = item(C, 3, "New", &all, body, eol).unwrap();
            for at in [0, host.len()] {
                let text = edited(host, &[insert(host, at, &new)]);
                assert!(text.contains(&new), "{body:?} into {host:?}: {text:?}");
                let document = parsed(&text);
                let index = document
                    .items
                    .iter()
                    .position(|item| item.mid == C)
                    .unwrap();
                let item = &document.items[index];
                let edits = vec![
                    add_row(&text, item, "status", "Accepted"),
                    set_value(&item.rows[1], "REQ-RENAMED"),
                    remove_row(&text, item, &item.rows[3]).unwrap(),
                    remove(&text, item.span.clone()),
                ];
                for splice in edits {
                    let result = try_edit(&text, &[splice]);
                    assert!(result.is_ok(), "{body:?} in {text:?}: {result:?}");
                }
                if let Some(first) = document.items.iter().find(|item| item.mid == A) {
                    let result = try_edit(&text, &[remove(&text, first.span.clone())]);
                    assert!(result.is_ok(), "{body:?} in {text:?}: {result:?}");
                }
            }
        }
    }
}

/// Review round 2, defect 1, and the edits like it: link reference definitions are
/// document-wide, so an edit that would change how text it does not touch reads is refused,
/// naming that text and the definition.
#[test]
fn refuses_edits_that_change_what_other_text_reads_as() {
    let refines = row("refines", "[REQ-THIRD][r]");
    let third = with(C, "Third", &[], "Body of the third.");
    let second = with(
        B,
        "Second",
        std::slice::from_ref(&refines),
        "See [the third][r].",
    );
    let first_inside = with(A, "First", &[], &format!("Before.\n\n[r]: #{C}\n\nAfter."));
    let first_trailing = with(A, "First", &[], &format!("Body.\n\n[r]: #{C}"));
    let notes = format!("Notes.\n\n[r]: #{C}\n");
    let page = |parts: &[&str]| parts.join("\n\n---\n\n") + "\n";
    let new_with = |body: &str| with("01M7FH6HEWNYQ66F5KE912BQSM", "New", &[], body);

    // The reviewer's case: the definition that the second item uses is in the first.
    let contents = page(&[&first_inside, &second, &third]);
    let document = parsed(&contents);
    assert_eq!(
        document.items[1].rows[3]
            .link
            .as_ref()
            .unwrap()
            .fragment
            .as_deref(),
        Some(C)
    );
    let misread = try_edit(
        &contents,
        &[remove(&contents, document.items[0].span.clone())],
    )
    .unwrap_err();
    assert!(misread.message.contains("the item `Second`"), "{misread:?}");
    assert!(
        misread.message.contains("is in the item being removed"),
        "{misread:?}"
    );

    // A definition after an item's last block is outside it and stays when the item goes.
    let contents = page(&[&first_trailing, &second, &third]);
    let document = parsed(&contents);
    let text = edited(
        &contents,
        &[remove(&contents, document.items[0].span.clone())],
    );
    let after = parsed(&text);
    assert_eq!(
        after.items[0].rows[3]
            .link
            .as_ref()
            .unwrap()
            .fragment
            .as_deref(),
        Some(C)
    );

    let new_defining = |target: &str| new_with(&format!("Mine: [m][r].\n\n[r]: #{target}"));
    type Edit<'e> = Box<dyn Fn(&str) -> Splice + 'e>;
    let cases: Vec<(&str, String, Edit<'_>, &str)> = vec![
        (
            "a definition that turns narrative into a link",
            page(&["Text with [r] in it.", &third]),
            Box::new(|text: &str| insert(text, text.len(), &new_defining(C))),
            "would become a link",
        ),
        (
            "a definition that shadows a later one",
            page(&[&second, &third, &notes]),
            Box::new(|text: &str| insert(text, 0, &new_defining(A))),
            "would come first and shadow the one at line",
        ),
        (
            "an inserted item's own definition shadowed",
            page(&[&notes, &third]),
            Box::new(|text: &str| insert(text, text.len(), &new_defining(A))),
            "outside the item",
        ),
        (
            "an inserted item that uses a definition outside it",
            page(&[&notes, &third]),
            Box::new(|text: &str| insert(text, text.len(), &new_with("Uses [x][r]."))),
            "outside the item",
        ),
        (
            "a definition changed in narrative",
            page(&[&second, &third, &notes]),
            Box::new(|text: &str| {
                let at = text.rfind(&format!("#{C}")).unwrap();
                Splice {
                    range: at..at + 27,
                    text: format!("#{A}"),
                    expect: Expect::Bytes,
                }
            }),
            "changes the definition `[r]`",
        ),
        (
            "a later definition of a label split from its title",
            page(&[&format!("{notes}\n[r]: #{A}\n\"Title\"\n"), &third]),
            Box::new(|text: &str| {
                insert(text, text.find("\"Title\"").unwrap(), &new_with("Body."))
            }),
            "read as a block of its own",
        ),
        (
            "an item inside a paragraph",
            page(&["Two lines\nof one paragraph.", &third]),
            Box::new(|text: &str| insert(text, text.find("of one").unwrap(), &new_with("Body."))),
            "would split the block that starts at line 1",
        ),
    ];
    for (name, contents, edit, expected) in cases {
        let misread = try_edit(&contents, &[edit(&contents)]).unwrap_err();
        assert!(misread.message.contains(expected), "{name}: {misread:?}");
    }
}

/// ADR-READ-BACK-EVERY-EDIT: the read-back parses each distinct content of an edit once,
/// however many documents it writes with that content.
#[test]
fn reads_back_each_distinct_content_of_an_edit_once() {
    let page = |mid: &str| format!("# Page\n\n---\n\n{}\n", written(mid, "Item", "\n"));
    let documents: Vec<ParsedFile> = [A, B, "01M7D8DZ39AMW9NPW9NA17D4S4"]
        .iter()
        .map(|mid| ParsedFile::parse(page(mid).into_bytes(), &mut WorkCounts::default()))
        .collect();
    let mut read_back = ReadBack::default();
    let mut work = WorkCounts::default();
    let mut written = Vec::new();
    // The first two lose their item and become the same narrative; the third keeps it.
    for (index, old) in documents.iter().enumerate() {
        let contents = std::str::from_utf8(&old.contents).unwrap();
        let item = &old.document.items[0];
        let splice = match index {
            2 => set_value(&item.rows[1], "REQ-RENAMED"),
            _ => remove(contents, item.span.clone()),
        };
        written.push(read_back.edit(old, &[splice], &mut work).unwrap());
    }
    assert_eq!(written[0].contents, written[1].contents);
    assert_eq!(written[0].document, written[1].document);
    let expected = WorkCounts {
        parses: 2,
        ..WorkCounts::default()
    };
    assert_eq!(work, expected);
}

/// Review round 3: only the bytes that the splices replace or insert may read differently. Each
/// case changes something that unchanged bytes read as, through an item, a narrative unit or a
/// table that a splice merely touches, and is refused, naming what would change.
#[test]
fn refuses_any_change_to_what_unchanged_bytes_read_as() {
    let new_item = |body: &str| with(B, "New", &[], body);
    let bytes = |_: &str, from: usize, to: usize, by: &str| Splice {
        range: from..to,
        text: by.to_owned(),
        expect: Expect::Bytes,
    };
    let at = |text: &str, needle: &str| text.find(needle).unwrap();
    let narrative = format!("# Page\n\nBefore [x][r].\n\nAfter [x][r].\n\n[r]: #{A}\n");
    let undefined = "# Page\n\nBefore [x][r].\n\nAfter [x][r].\n".to_owned();
    let defining = with(A, "Def", &[], &format!("[r]: #{C}\n\nBody."));
    let split = format!("# Page\n\nBefore.\n\nAfter [x][r].\n\n---\n\n{defining}\n");
    let labelled = with(
        A,
        "First",
        &[],
        &format!("Untouched [r].\n\n[s]: #{C}\n\nEnd."),
    );
    let titled = format!("# [Heading][h]\n\nText.\n\n[h]: #{A}\n");
    let images = format!(
        "![logo][i]\n\n[i]: logo.png\n\n---\n\n{}\n",
        written(C, "Third", "\n")
    );
    let notes = format!(
        "A note[^n].\n\n---\n\n{}\n\n[^n]: The note.\n",
        written(C, "Third", "\n")
    );
    let other = format!(
        "[site][w] here.\n\n[w]: https://one.example\n\n---\n\n{}\n",
        written(C, "Third", "\n")
    );
    let paragraphs = format!(
        "First paragraph.\n\nSecond paragraph.\n\n---\n\n{}\n",
        written(C, "Third", "\n")
    );
    let incomplete = format!(
        "### Lacks an anchor\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-X |\n\nBody [m](#{A}).\n"
    );
    type Edit<'e> = Box<dyn Fn(&str) -> Vec<Splice> + 'e>;
    let cases: Vec<(&str, String, Edit<'_>, &[&str])> = vec![
        (
            "D1: an item between two paragraphs whose definition shadows theirs",
            narrative.clone(),
            Box::new(|text: &str| {
                vec![insert(
                    text,
                    at(text, "After"),
                    &new_item(&format!("[r]: #{C}\n\nBody.")),
                )]
            }),
            &["the narrative", "`[x][r]`", "would come first and shadow"],
        ),
        (
            "D1: an item between two paragraphs whose definition makes plain text links",
            undefined.clone(),
            Box::new(|text: &str| {
                vec![insert(
                    text,
                    at(text, "After"),
                    &new_item(&format!("[r]: #{C}\n\nBody.")),
                )]
            }),
            &[
                "the narrative",
                "would become a link",
                "that the edit writes or changes would decide it",
            ],
        ),
        (
            "D1: an insertion and the removal of the item holding the definition",
            split.clone(),
            Box::new(|text: &str| {
                let document = parsed(text);
                vec![
                    insert(text, at(text, "After"), &new_item("Body.")),
                    remove(text, document.items[0].span.clone()),
                ]
            }),
            &[
                "the narrative",
                "would no longer be a link",
                "is in the item being removed",
            ],
        ),
        (
            "D2: a definition's label changed inside the item",
            labelled.clone(),
            Box::new(|text: &str| {
                vec![bytes(text, at(text, "[s]:") + 1, at(text, "[s]:") + 2, "r")]
            }),
            &["the item `First`", "`[r]`", "would become a link"],
        ),
        (
            "a narrative heading whose link loses its definition",
            titled.clone(),
            Box::new(|text: &str| vec![bytes(text, at(text, "[h]:"), text.len(), "")]),
            &["`[Heading][h]`", "would no longer be a link"],
        ),
        (
            "an image whose definition changes",
            images.clone(),
            Box::new(|text: &str| {
                vec![bytes(
                    text,
                    at(text, "logo.png"),
                    at(text, "logo.png") + 8,
                    "other.png",
                )]
            }),
            &[
                "`![logo][i]`",
                "would lead to `other.png` instead of `logo.png`",
            ],
        ),
        (
            "a footnote reference whose footnote an inserted item shadows",
            notes.clone(),
            Box::new(|text: &str| {
                vec![insert(
                    text,
                    at(text, "---"),
                    &new_item("Body.\n\n[^n]: Another note."),
                )]
            }),
            &["`[^n]`", "would refer to another footnote"],
        ),
        (
            "a link to a web page whose definition changes",
            other.clone(),
            Box::new(|text: &str| vec![bytes(text, at(text, "one."), at(text, "one.") + 3, "two")]),
            &["`[site][w]`", "would lead to `https://two.example`"],
        ),
        (
            "two paragraphs joined by changing only the blank line between them",
            paragraphs.clone(),
            Box::new(|text: &str| {
                vec![bytes(
                    text,
                    at(text, ".\n\nSecond") + 1,
                    at(text, "Second"),
                    " ",
                )]
            }),
            &["would join another"],
        ),
        (
            "a paragraph that would take in a later definition of a label",
            format!(
                "[r]: #{A}\n\nA paragraph.\n\n[r]: #{C}\n\n---\n\n{}\n",
                written(C, "Third", "\n")
            ),
            Box::new(|text: &str| {
                vec![bytes(
                    text,
                    at(text, ".\n\n[r]") + 2,
                    at(text, ".\n\n[r]") + 3,
                    "",
                )]
            }),
            &["`[r]` in it would become a link"],
        ),
        (
            "narrative that would join an item through a removed separator",
            format!(
                "{}\n\n---\n\nNarrative after it.\n",
                written(C, "Third", "\n")
            ),
            Box::new(|text: &str| {
                vec![bytes(
                    text,
                    at(text, "\n---\n\nNarrative"),
                    at(text, "Narrative"),
                    "\n",
                )]
            }),
            &["would end elsewhere"],
        ),
        (
            "an anchor written before a heading, which turns unchanged rows into an item's",
            incomplete.clone(),
            Box::new(|text: &str| vec![bytes(text, 0, 0, &format!("<a id=\"{B}\"></a>\n\n"))]),
            &["would make other text read as the `Type` row"],
        ),
    ];
    for (name, contents, edit, expected) in cases {
        let misread = try_edit(&contents, &edit(&contents)).unwrap_err();
        for fragment in expected {
            assert!(misread.message.contains(fragment), "{name}: {misread:?}");
        }
    }

    // What an edit means to change, it writes: an anchor written with the whole item it makes,
    // or a definition changed in a splice that holds the links that use it, reads back.
    let whole = vec![bytes(
        &incomplete,
        0,
        incomplete.len(),
        &format!("<a id=\"{B}\"></a>\n\n{incomplete}"),
    )];
    assert!(try_edit(&incomplete, &whole).is_ok());
    let retargeted = narrative.replace(&format!("#{A}"), &format!("#{C}"));
    let all = vec![bytes(&narrative, 0, narrative.len(), &retargeted)];
    assert!(try_edit(&narrative, &all).is_ok());
}

/// Review round 3, D3: writing the splices of an edit and reading it back each cost a constant
/// multiple of parsing the old and the new contents however many splices it has, so a batch of
/// removals, insertions or splits of narrative units is not quadratic. A quadratic path takes
/// forty to two hundred and fifty times as long here.
#[test]
fn reads_back_many_splices_in_time_proportional_to_the_document() {
    let count = 1500;
    let mut items = String::from("# Page\n");
    for n in 0..count {
        let name = format!("I{n}");
        items.push_str(&format!(
            "\n---\n\n{}\n",
            with(&mid(n as u64, [1; 10]), &name, &[], "Body.")
        ));
    }
    let mut units = String::new();
    for n in 0..count {
        units.push_str(&format!(
            "## Unit {n}\n\nFirst of {n}.\n\nSecond of {n}.\n\n"
        ));
    }
    let removals = |text: &str| -> Vec<Splice> {
        let document = parsed(text);
        document
            .items
            .iter()
            .map(|item| remove(text, item.span.clone()))
            .collect()
    };
    let insertions = |text: &str| -> Vec<Splice> {
        text.match_indices("Second of ")
            .enumerate()
            .map(|(n, (at, _))| {
                insert(
                    text,
                    at,
                    &with(&mid(n as u64, [2; 10]), &format!("N{n}"), &[], "Body."),
                )
            })
            .collect()
    };
    type Splices<'s> = &'s dyn Fn(&str) -> Vec<Splice>;
    let shapes: [(&str, &String, Splices<'_>); 2] = [
        ("removals", &items, &removals),
        ("insertions that split units", &units, &insertions),
    ];
    for (name, text, splices) in shapes {
        let started = std::time::Instant::now();
        let splices = splices(text);
        let writing = started.elapsed();
        assert_eq!(splices.len(), count, "{name}");
        let old = ParsedFile::parse(text.as_bytes().to_vec(), &mut WorkCounts::default());
        let new = apply(text.as_bytes(), &splices);
        let (mut edit_time, mut parse_time) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..3 {
            let started = std::time::Instant::now();
            let result = ReadBack::default().edit(&old, &splices, &mut WorkCounts::default());
            edit_time = edit_time.min(started.elapsed());
            assert!(result.is_ok(), "{name}: {result:?}");
            let started = std::time::Instant::now();
            ParsedFile::parse(text.as_bytes().to_vec(), &mut WorkCounts::default());
            ParsedFile::parse(new.clone(), &mut WorkCounts::default());
            parse_time = parse_time.min(started.elapsed());
        }
        assert!(
            edit_time < parse_time * 20,
            "{name}: the read-back took {edit_time:?}, a parse {parse_time:?}"
        );
        assert!(
            writing < parse_time * 20,
            "{name}: writing the splices took {writing:?}, a parse {parse_time:?}"
        );
    }
}

/// What the operations of this slice write through splices of their own reads back under the
/// stricter rule: they change the bytes of what they mean to change.
#[test]
fn accepts_the_edits_that_operations_make() {
    let bytes = |range: Range<usize>, text: &str| Splice {
        range,
        text: text.to_owned(),
        expect: Expect::Bytes,
    };
    let at = |text: &str, needle: &str| text.find(needle).unwrap();
    let third = with(C, "Third", &[], "Body of the third.");
    let linking = with(
        B,
        "Second",
        &[row("refines", &format!("[REQ-Third](#{C})"))],
        &format!("See [REQ-Third](#{C}) and [the third][t].\n\n[t]: #{C}"),
    );
    let page = format!("# See [REQ-Third](#{C})\n\n---\n\n{linking}\n\n---\n\n{third}\n");
    let document = parsed(&page);
    let second = &document.items[0];
    let cases: Vec<(&str, Vec<Splice>)> = vec![
        (
            "rename: the mention text in a heading and a body, then the relation link text",
            vec![
                bytes(
                    at(&page, "[REQ-Third](#")..at(&page, "[REQ-Third](#") + 11,
                    "[REQ-RENAMED]",
                ),
                set_value(&second.rows[3], &format!("[REQ-RENAMED](#{C})")),
                {
                    let body = at(&page, "\nSee [REQ-Third]") + 5;
                    bytes(body..body + 11, "[REQ-RENAMED]")
                },
            ],
        ),
        (
            "move: a link path rewritten, in a definition with the links that use it",
            {
                let from = at(&page, "and [the third]");
                let to = at(&page, &format!("[t]: #{C}")) + format!("[t]: #{C}").len();
                let rewritten =
                    page[from..to].replace(&format!("#{C}"), &format!("other.mara.md#{C}"));
                vec![bytes(from..to, &rewritten)]
            },
        ),
        (
            "fix: a setext heading written as an ATX heading",
            vec![bytes(0..9, "# Title")],
        ),
    ];
    // delete: an item that holds the first definition of a label, which a later definition of
    // the label then takes over, while no link uses it.
    let holding = with(A, "Holding", &[], &format!("Body.\n\n[r]: #{C}\n\nMore."));
    let doubled = format!("{holding}\n\n---\n\nNotes.\n\n[r]: #{B}\n");
    let span = parsed(&doubled).items[0].span.clone();
    assert!(try_edit(&doubled, &[remove(&doubled, span)]).is_ok());
    for (name, splices) in cases {
        let contents = if name.starts_with("fix") {
            "Title\n===\n\nText.\n"
        } else {
            page.as_str()
        };
        let result = try_edit(contents, &splices);
        assert!(result.is_ok(), "{name}: {result:?}");
    }
}
