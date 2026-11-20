use super::*;

fn prefixes(list: &[&str]) -> Vec<String> {
    list.iter().map(|prefix| (*prefix).to_owned()).collect()
}

/// The markers of `text` as (line, relation and ID, or `None` for an invalid one).
fn scan(text: &[u8], list: &[&str]) -> Vec<(usize, Option<(String, String)>)> {
    let mut work = WorkCounts::default();
    let file = ScannedFile::scan(text, &prefixes(list), &mut work);
    assert_eq!(work.code_scans, 1);
    file.markers
        .iter()
        .map(|marker| {
            let link = marker.link.as_ref().map(|link| {
                let at = link.id_start;
                assert_eq!(&text[at..at + link.id.len()], link.id.as_bytes());
                (link.relation.clone(), link.id.clone())
            });
            (marker.line, link)
        })
        .collect()
}

fn link(line: usize, id: &str) -> (usize, Option<(String, String)>) {
    (line, Some(("implements".to_owned(), id.to_owned())))
}

#[test]
fn reads_markers_as_developers_write_them() {
    let rust = b"// @mara implements REQ-A\nfn a() {\n    // @mara implements REQ-B\n    \
                 check(); // @mara implements REQ-C\n}\n/// @mara implements REQ-D\n\t//@mara\timplements\tREQ-E\t\n";
    assert_eq!(
        scan(rust, &["//", "///", "*"]),
        [
            link(1, "REQ-A"),
            link(3, "REQ-B"),
            link(4, "REQ-C"),
            link(6, "REQ-D"),
            link(7, "REQ-E")
        ]
    );
    let crlf = b"// @mara implements REQ-A\r\nfn a() {}\r\n";
    assert_eq!(scan(crlf, &["//"]), [link(1, "REQ-A")]);
    let jsdoc = b"/**\n * Signs in.\n * @mara implements REQ-A\n */\n";
    assert_eq!(scan(jsdoc, &["//", "*"]), [link(3, "REQ-A")]);
    let python = b"#!/usr/bin/env python3\n# @mara implements REQ-A\n";
    assert_eq!(scan(python, &["#"]), [link(2, "REQ-A")]);
    let latin1 = b"// caf\xe9\n// @mara implements REQ-A";
    assert_eq!(scan(latin1, &["//"]), [link(2, "REQ-A")]);
}

#[test]
fn reports_malformed_markers_and_ignores_what_is_no_marker() {
    let text = b"// @mara implements\n// @mara implements REQ-A REQ-B\n// @mara implements req-a\n\
                 // @mara\nlet s = \" // @mara implements REQ-A\";\n";
    assert_eq!(
        scan(text, &["//"]),
        [(1, None), (2, None), (3, None), (4, None), (5, None)]
    );
    let none = b"// see @mara implements REQ-A\nlet s = \"// @mara implements REQ-A\";\n\
                 // @marax implements REQ-A\n/// @mara implements REQ-A\nx // y // @mara implements REQ-A\n";
    assert_eq!(scan(none, &["//"]), []);
    // A prefix that is also an operator starts a comment where it stands after a space.
    assert_eq!(
        scan(b"a * b; // @mara implements REQ-A\n", &["//", "*"]),
        []
    );
}

#[test]
fn keys_markers_by_prefixes_and_contents() {
    let mut work = WorkCounts::default();
    let file = Hash::of_file(b"x", &mut work);
    let language = |list: &[&str]| Language {
        name: "rust".to_owned(),
        include: Vec::new(),
        prefixes: prefixes(list),
        grammar: None,
    };
    let rust = LanguageKey::new(&language(&["//"]), None);
    assert_eq!(rust, LanguageKey::new(&language(&["//"]), None));
    assert_ne!(rust, LanguageKey::new(&language(&["//", "*"]), None));
    let grammar = Hash::of_bytes(b"grammar");
    let with_grammar = LanguageKey::new(&language(&["//"]), Some(&grammar));
    assert_ne!(rust, with_grammar);
    assert_ne!(
        with_grammar,
        LanguageKey::new(&language(&["//"]), Some(&Hash::of_bytes(b"other")))
    );
    assert_ne!(
        MarkerKey::new(rust, file),
        MarkerKey::new(rust, Hash::of_bytes(b"y"))
    );
    let empty = ScannedFile::scan(b"fn main() {}\n", &prefixes(&["//"]), &mut work);
    assert_eq!(
        (empty.markers.len(), empty.contents, empty.lines),
        (0, None, 1)
    );
    let last = ScannedFile::scan(b"a\n\nb", &prefixes(&["//"]), &mut work);
    assert_eq!(last.lines, 3);
}

#[test]
fn renames_only_the_bytes_of_the_old_id_in_markers() {
    let mut work = WorkCounts::default();
    let text = b"// @mara implements REQ-A\r\nlet s = \"REQ-A\"; // @mara implements REQ-A-B\n\
                 x(); // @mara implements REQ-A";
    let scanned = ScannedFile::scan(text, &prefixes(&["//"]), &mut work);
    let layer = CodeLayer::new(
        vec![CodeFile {
            path: "src/a.rs".to_owned(),
            key: MarkerKey(Hash::of_bytes(b"k")),
            hash: Hash::of_bytes(text),
            scanned: Arc::new(scanned),
        }],
        Vec::new(),
    );
    let edits = rename(&layer, "REQ-A", "REQ-SIGN-IN").unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].contents,
        b"// @mara implements REQ-SIGN-IN\r\nlet s = \"REQ-A\"; // @mara implements REQ-A-B\n\
          x(); // @mara implements REQ-SIGN-IN"
    );
    assert_eq!(rename(&layer, "REQ-C", "REQ-D").unwrap(), []);
    assert_eq!(
        rename(&layer, "REQ-A", "not an id"),
        Err("src/a.rs".to_owned())
    );
}

/// The range of the `n`th occurrence of `needle` in `text`, from its start to `end`'s end after
/// it.
fn span(text: &[u8], needle: &str, end: &str) -> std::ops::Range<usize> {
    let start = find(text, needle.as_bytes()).unwrap();
    let length = find(&text[start..], end.as_bytes()).unwrap() + end.len();
    start..start + length
}

fn declaration(
    name: &str,
    range: std::ops::Range<usize>,
    parent: Option<usize>,
) -> SyntaxDeclaration {
    SyntaxDeclaration {
        name: name.to_owned(),
        range,
        parent_start: parent,
    }
}

/// Each marker of `scanned` as (line, ID, location line, declaration).
fn owned(scanned: &ScannedFile) -> Vec<(usize, String, usize, Option<String>)> {
    (scanned.markers.iter())
        .map(|marker| {
            let location = scanned.location(marker.owner);
            let id = marker
                .link
                .as_ref()
                .map_or("?".to_owned(), |l| l.id.clone());
            (
                marker.line,
                id,
                location.line,
                location.declaration.map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn owns_markers_by_the_declaration_they_precede_or_stand_in() {
    let text: &[u8] =
        b"// @mara implements REQ-A\nexport function f() {\n  g(); // @mara implements REQ-B\n}\n\
                        mod m {\n    struct S; // @mara implements REQ-C\n}\n\
                        /*\n * @mara implements REQ-D\n */\n#[a]\n\nfn h() {}\n\
                        // @mara implements REQ-E\nlet x = 1;\n// @mara implements REQ-F";
    let export = span(text, "export", "}\n");
    let function = span(text, "function f", "}\n");
    let module = span(text, "mod m", "}\n");
    let structure = span(text, "struct S", ";");
    let h = span(text, "fn h", "}");
    let syntax = Syntax {
        comments: vec![
            span(text, "// @mara implements REQ-A", "A"),
            span(text, "// @mara implements REQ-B", "B"),
            span(text, "// @mara implements REQ-C", "C"),
            span(text, "/*", "*/"),
            span(text, "// @mara implements REQ-E", "E"),
            span(text, "// @mara implements REQ-F", "F"),
        ],
        attributes: vec![span(text, "#[a]", "]")],
        declarations: vec![
            declaration("f", function, Some(export.start)),
            declaration("m", module.clone(), Some(0)),
            declaration("S", structure, Some(module.start)),
            declaration("h", h, Some(0)),
            // Out of the file: cut to it, then dropped as empty.
            declaration("gone", 10_000..10_010, None),
        ],
    };
    let mut work = WorkCounts::default();
    let scanned = ScannedFile::scan_syntax(text, &prefixes(&["//", "*"]), &syntax, &mut work);
    assert_eq!(work.code_scans, 1);
    let at = |line, id: &str, location, name: Option<&str>| {
        (line, id.to_owned(), location, name.map(str::to_owned))
    };
    assert_eq!(
        owned(&scanned),
        [
            // The function of the `export` statement on the next line.
            at(1, "REQ-A", 2, Some("f")),
            at(3, "REQ-B", 2, Some("f")),
            // After code on an indented line: that code's declaration.
            at(6, "REQ-C", 6, Some("S")),
            // A block comment's later line, then an attribute and a blank line before `h`.
            at(9, "REQ-D", 13, Some("h")),
            // Before a statement that is no declaration, and at the end of the file.
            at(14, "REQ-E", 1, None),
            at(16, "REQ-F", 1, None),
        ]
    );
    let lines: Vec<Owner> = (1..=16).map(|line| scanned.owner(line)).collect();
    let d = Owner::Declaration;
    assert_eq!(
        lines,
        [
            d(0),
            d(0),
            d(0),
            d(0),
            d(1),
            d(2),
            d(1),
            d(3),
            d(3),
            d(3),
            d(3),
            Owner::File,
            d(3),
            Owner::File,
            Owner::File,
            Owner::File,
        ]
    );
    let function = scanned.location(d(0));
    assert_eq!((function.line, function.last_line), (2, Some(4)));
}

#[test]
fn reads_markers_only_in_comment_nodes_at_the_grammar_level() {
    let text: &[u8] = b"let s = \"// @mara implements REQ-A\";\n/* @mara implements REQ-B */\n\
                        // @mara implements\n/// @mara implements REQ-C\n";
    let syntax = Syntax {
        comments: vec![
            span(text, "/* @mara", "*/"),
            span(text, "// @mara implements\n", "\n"),
            span(text, "/// @mara", "C"),
        ],
        ..Syntax::default()
    };
    let mut work = WorkCounts::default();
    let scanned = ScannedFile::scan_syntax(text, &prefixes(&["//", "///"]), &syntax, &mut work);
    let found: Vec<(usize, Option<String>)> = (scanned.markers.iter())
        .map(|marker| (marker.line, marker.link.as_ref().map(|l| l.id.clone())))
        .collect();
    // The string holds none; `/*` is no prefix; an invalid marker is one.
    assert_eq!(found, [(3, None), (4, Some("REQ-C".to_owned()))]);
    let renamed = scanned.markers[1].link.as_ref().unwrap();
    assert_eq!(&text[renamed.id_start..renamed.id_start + 5], b"REQ-C");
    // At the basic level the string's prefix after a space is no comment start, as its quote
    // precedes it, and the location is the marker's line.
    let basic = ScannedFile::scan(text, &prefixes(&["//", "///"]), &mut work);
    assert_eq!(basic.markers.len(), 2);
    assert_eq!(basic.location(basic.owner(4)).line, 4);
}
