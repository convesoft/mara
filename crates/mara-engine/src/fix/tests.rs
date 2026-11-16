//! Tests of the repairs that fix plans, below the CLI.

use super::*;

#[test]
fn finds_the_destination_of_an_inline_link_as_written() {
    let mid = "01M7D8DZ2663WP3A037SR2HJH2";
    let destination = format!("a.mara.md#{mid}");
    let cases: Vec<(String, Option<&str>)> = vec![
        (format!("[REQ-A](a.mara.md#{mid})"), Some("a.mara.md#")),
        (format!("[REQ-A](<a.mara.md#{mid}>)"), Some("a.mara.md#")),
        (
            format!("[text]( a.mara.md#{mid} \"title\")"),
            Some("a.mara.md#"),
        ),
        (
            format!("[text](\n  a.mara.md#{mid}\n'title')"),
            Some("a.mara.md#"),
        ),
        (
            format!("[a [nested] `](x)` text](a.mara.md#{mid})"),
            Some("a.mara.md#"),
        ),
        // A title that repeats the destination leaves two places that could be it.
        (
            format!("[t](a.mara.md#{mid} \"](a.mara.md#{mid} )\")"),
            None,
        ),
        // Written otherwise than the parser reads it.
        (format!("[t](a\\.mara.md#{mid})"), None),
        (format!("[t](a.mara.md#{mid}x)"), None),
        (format!("[t][a.mara.md#{mid}]"), None),
    ];
    for (source, expected) in cases {
        let found = inline_destination(&source, &destination);
        let found = found.map(|range| {
            assert_eq!(&source[range.clone()], destination, "{source:?}");
            &source[range.start..range.start + "a.mara.md#".len()]
        });
        assert_eq!(found, expected, "{source:?}");
    }
}

#[test]
fn finds_the_destination_of_a_definition_as_written() {
    let destination = "../b.mara.md#01M7D8DZ2663WP3A037SR2HJH2";
    for (source, found) in [
        (format!("[t]: {destination}"), true),
        (format!("[t]: <{destination}>"), true),
        (format!("[a \\] b]:\n   {destination} \"Title\""), true),
        (format!("  [t]:{destination}"), true),
        (format!("[t]: {destination}x"), false),
        (format!("[t] : {destination}"), false),
    ] {
        let range = definition_destination(&source, destination);
        assert_eq!(range.is_some(), found, "{source:?}");
        if let Some(range) = range {
            assert_eq!(&source[range], destination, "{source:?}");
        }
    }
}

#[test]
fn marks_the_regions_that_lack_only_their_anchor() {
    let table = "| Type | Requirement |\n| --- | --- |\n| ID | REQ-A |\n";
    let mid = "01M7D8DZ2663WP3A037SR2HJH2";
    let cases = [
        (format!("### A\n\n{table}\nBody.\n"), true),
        (format!("# Page\n\n---\n\n### A\n{table}"), true),
        // After other text, the heading starts a region of its own (DES-REGIONS).
        (format!("Text first.\n\n### A\n\n{table}\nBody.\n"), true),
        (
            "Text first.\n\n### A\n\n| Type | Requirement |\n| --- | --- |\n".to_owned(),
            false,
        ),
        (
            format!("<a id=\"{}\"></a>\n\n### A\n\n{table}", mid.to_lowercase()),
            false,
        ),
        (format!("<a id=\"{mid}\"></a>\n\n{table}"), false),
        ("### A\n\n| ID | REQ-A |\n| --- | --- |\n".to_owned(), false),
        (
            format!("<a id=\"{mid}\"></a>\n\n### A\n\n{table}\nBody.\n"),
            false,
        ),
    ];
    for (text, lacking) in cases {
        let document = crate::document::parse(text.as_bytes(), &mut Default::default());
        let expected = lacking.then(|| text.find("### A").unwrap()..text.trim_end().len());
        let found = document
            .anchorless
            .first()
            .map(|region| region.span.clone());
        assert_eq!(found, expected, "{text:?}");
        assert!(document.anchorless.len() <= 1, "{text:?}");
    }
}
