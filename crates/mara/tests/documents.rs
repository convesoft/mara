//! This repository's own documents in `docs/` parse without diagnostics (mara-t6xn9z). The test
//! reads them from the checkout and writes nothing.

use std::fs;
use std::path::Path;

use mara_engine::document;
use mara_engine::work::WorkCounts;

#[test]
fn the_documents_in_docs_parse_without_diagnostics() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut paths: Vec<_> = fs::read_dir(&docs)
        .expect("list docs/")
        .map(|entry| entry.expect("read docs/").path())
        .filter(|path| path.to_string_lossy().ends_with(".mara.md"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no documents in {}", docs.display());

    let mut work = WorkCounts::default();
    let mut diagnostics = Vec::new();
    for path in &paths {
        let bytes = fs::read(path).expect("read a document");
        let name = format!("docs/{}", path.file_name().unwrap().to_string_lossy());
        let parsed = document::parse(&bytes, &mut work);
        diagnostics.extend(parsed.diagnostics(&name).map(|d| d.to_string()));
        // Every anchor line outside code is an item, so nothing is silently left out.
        let anchors = anchor_lines(&String::from_utf8_lossy(&bytes));
        let items: Vec<usize> = parsed.items.iter().map(|item| item.line).collect();
        assert_eq!(items, anchors, "item lines of {name}");
        for item in &parsed.items {
            assert!(item.value("id").is_some(), "{name}:{} has no ID", item.line);
        }
        // The same document with lone-CR line endings reads alike (DES-REGIONS).
        let lone_cr: Vec<u8> = bytes
            .iter()
            .map(|&b| if b == b'\n' { b'\r' } else { b })
            .collect();
        let reread = document::parse(&lone_cr, &mut WorkCounts::default());
        diagnostics.extend(reread.diagnostics(&name).map(|d| format!("lone CR: {d}")));
        assert_eq!(reread, parsed, "{name} with lone CR");
    }
    assert!(diagnostics.is_empty(), "{}", diagnostics.join("\n"));
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
