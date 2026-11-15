use super::create::propose;
use super::heading;
use crate::snapshot::ParsedFile;
use crate::work::WorkCounts;

#[test]
fn proposes_an_id_from_the_prefix_and_the_title() {
    for (prefix, title, id) in [
        (
            Some("REQ-"),
            "Phishing-resistant auth",
            "REQ-PHISHING-RESISTANT-AUTH",
        ),
        (Some("ADR-"), "Use *WebAuthn*  now!", "ADR-USE-WEBAUTHN-NOW"),
        (Some("TERM-"), "  Café 2 ", "TERM-CAF-2"),
        (None, "Sign in", "SIGN-IN"),
        (Some("REQ-"), "¿?", "REQ-"),
    ] {
        assert_eq!(propose(prefix, title), id, "{title:?}");
    }
}

#[test]
fn finds_the_text_and_level_of_each_kind_of_heading() {
    for (heading_text, level, atx, text) in [
        ("## Title", 2, true, "Title"),
        ("### Title ##", 3, true, "Title"),
        ("#   Spaced\t", 1, true, "Spaced"),
        ("## C#", 2, true, "C#"),
        ("## Ends \\#", 2, true, "Ends \\#"),
        ("  #### Indented", 4, true, "Indented"),
        ("Setext\n===", 1, false, "Setext"),
        ("Two\nlines\n---", 2, false, "Two\nlines"),
    ] {
        let source = format!(
            "<a id=\"01M7BF6GKV9FQZBDWDZQJVE164\"></a>\n\n{heading_text}\n\n| Type | Requirement |\n| --- | --- |\n\nBody.\n"
        );
        let file = ParsedFile::parse(source.clone().into_bytes(), &mut WorkCounts::default());
        let item = &file.document.items[0];
        let found = heading(&file, item).unwrap_or_else(|| panic!("{heading_text:?}"));
        assert_eq!(
            (found.level, found.atx, &source[found.text]),
            (level, atx, text),
            "{heading_text:?}"
        );
    }
}
