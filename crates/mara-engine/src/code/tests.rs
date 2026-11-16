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
    };
    let rust = LanguageKey::new(&language(&["//"]));
    assert_eq!(rust, LanguageKey::new(&language(&["//"])));
    assert_ne!(rust, LanguageKey::new(&language(&["//", "*"])));
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
