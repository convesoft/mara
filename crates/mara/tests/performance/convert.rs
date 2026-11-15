//! The `project convert` row of VER-WORK-COUNTERS: exact counts through the real CLI on a small
//! hand-written 0.3.0 project, not generator output. The conversion has no scaling case
//! (ADR-NO-CONVERTER-SCALING).

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use mara_engine::work::WorkCounts;

use crate::{cli, restore};

const FILES: [(&str, &str); 14] = [
    (
        ".mara/project.toml",
        "format_version = 3\n\n[project]\nname = \"counts\"\nschema = \".mara/schema.yaml\"\n\n[content]\ninclude = [\"docs/**/*.mara.md\"]\n",
    ),
    (
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  depends_on:\n    description: Needs.\n    source: [requirement]\n    target: [requirement]\n    inverse: required_by\n",
    ),
    (
        "docs/a.mara.md",
        "# A\n\n:::mara requirement REQ-A\n:mid: 01M7FX0E000000000000000001\n:title: A\n:status: draft\n:depends_on: REQ-B\n\nNames [[REQ-B]].\n:::\n",
    ),
    (
        "docs/sub/b.mara.md",
        "# B\n\n:::mara requirement REQ-B\n:mid: 01M7FX0E000000000000000002\n:title: B\n:status: draft\n\nBody.\n:::\n",
    ),
    (
        "docs/notes.mara.md",
        "# Notes\n\nNo 0.3.0 syntax: not written.\n",
    ),
    (".gitignore", "ignored/\n"),
    ("docs/.gitignore", "drafts/\n"),
    ("docs/ignored/c.mara.md", "# Ignored\n"),
    (
        "docs/ignored/.gitignore",
        "Below an ignored directory: never read.\n",
    ),
    (
        "docs/drafts/d.mara.md",
        "# Ignored by the nested .gitignore\n",
    ),
    ("docs/notes.txt", "Not a document.\n"),
    ("README.md", "# Readme\n"),
    (".git/HEAD", "ref: refs/heads/main\n"),
    (".git/objects/00/object", "object"),
];

/// The files that an earlier, stopped run leaves: the report file, then also the schema.
const LEFT: [(&str, &str); 2] = [
    (
        ".mara/conversion-report.json",
        "{\"format_version\":1,\"entries\":[]}\n",
    ),
    (
        ".mara/schema.toml",
        "format_version = 1\nstatuses = [\"draft\"]\n",
    ),
];

/// Writes the project into `root`, with the first `left` files of [`LEFT`], and waits until
/// every file is settled, so that the checks of the write procedure read nothing again
/// (DES-INPUTS-AND-KEYS).
fn project(root: &Path, left: usize) {
    for (path, contents) in FILES.iter().chain(&LEFT[..left]) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    restore::settle(SystemTime::now());
}

#[test]
fn convert_counts() {
    // What the 0.3.0 configuration declares, the configuration, the schema and the three
    // documents, and the project's two `.gitignore` files, are each read and hashed once. So are
    // the report file and the schema that an earlier run left. Each document is parsed, and each
    // of the two it writes is read back. No `.git`, ignored or other undeclared file is read,
    // not even the `.gitignore` in an ignored directory.
    for left in 0..=LEFT.len() {
        let dir = tempfile::Builder::new()
            .prefix("mara-convert-counts-")
            .tempdir()
            .unwrap();
        project(dir.path(), left);
        let run = cli::run(dir.path(), &["project", "convert"], 0);
        let read = 5 + 2 + left as u64;
        let expected = WorkCounts {
            file_reads: read,
            hashes: read,
            parses: 3 + 2,
            ..WorkCounts::default()
        };
        assert_eq!(
            run.work, expected,
            "with {left} files of an earlier run left"
        );
    }
}
