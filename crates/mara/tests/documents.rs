//! The parser reads this repository's documents in `docs/` without losing an item, whatever their
//! line endings (DES-REGIONS, mara-t6xn9z). Whether they are valid is the `project validate` local
//! check of AGENTS.md (mara-37qtf2). The test reads them from the checkout and writes nothing.

use std::fs;
use std::path::Path;

use mara_engine::document;
use mara_engine::work::WorkCounts;

#[test]
fn every_anchor_in_docs_is_an_item_with_any_line_ending() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut paths: Vec<_> = fs::read_dir(&docs)
        .expect("list docs/")
        .map(|entry| entry.expect("read docs/").path())
        .filter(|path| path.to_string_lossy().ends_with(".mara.md"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no documents in {}", docs.display());

    let mut work = WorkCounts::default();
    for path in &paths {
        let bytes = fs::read(path).expect("read a document");
        let name = format!("docs/{}", path.file_name().unwrap().to_string_lossy());
        let parsed = document::parse(&bytes, &mut work);
        // Every anchor line outside code is an item, so nothing is silently left out.
        let anchors = anchor_lines(&String::from_utf8_lossy(&bytes));
        let items: Vec<usize> = parsed.items.iter().map(|item| item.line).collect();
        assert_eq!(items, anchors, "item lines of {name}");
        // The same document with lone-CR line endings reads alike, diagnostics included.
        let lone_cr: Vec<u8> = bytes
            .iter()
            .map(|&b| if b == b'\n' { b'\r' } else { b })
            .collect();
        let reread = document::parse(&lone_cr, &mut WorkCounts::default());
        assert_eq!(reread, parsed, "{name} with lone CR");
    }
    assert_eq!(work.parses, paths.len() as u64);
}

/// The lines that are a MID anchor of their own, outside fenced code blocks.
fn anchor_lines(text: &str) -> Vec<usize> {
    let mut fenced = false;
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("<a id=\"") && line.ends_with("\"></a>") {
            lines.push(index + 1);
        }
    }
    lines
}
