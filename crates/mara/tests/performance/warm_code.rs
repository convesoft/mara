//! The warm rows of VER-CODE-WORK-COUNTERS (`docs/code/verification.mara.md`) but those of `get`
//! (`code_get.rs`), and the code row of VER-WORK-COUNTERS (`docs/verification.mara.md`), at 1×
//! through the real CLI against a daemon the test starts, as `warm.rs` runs the other warm rows.
//! The corpus declares Rust at the basic level; a code directory holds a `.gitignore`, so that a
//! row would count the listing of that directory (VER-CODE-WORK-COUNTERS).

#![cfg(debug_assertions)]

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use mara_engine::work::WorkCounts;

use crate::derived::{self, Case, decided};
use crate::grammar_server::{GrammarServer, fixture};
use crate::restore::{self, Written};
use crate::warm::{
    Change, LIST, UNCHANGED, VALIDATE, Warm, assert_counts, document, edit_in_place, reads_in,
};

/// The corpus's code file `index`.
fn code(warm: &Warm, index: usize) -> PathBuf {
    warm.root().join(&warm.corpus.code_files[index])
}

/// Appends a comment naming `run` to the code file at `path`: new contents.
fn edit_code(path: &PathBuf, run: &str) {
    let mut contents = fs::read_to_string(path).unwrap();
    contents.push_str(&format!("// Edited in {run}.\n"));
    fs::write(path, contents).unwrap();
}

/// [`Warm::start`] on a corpus whose first code file's directory holds a `.gitignore`.
fn start() -> Warm {
    let warm = Warm::new();
    gitignore_in_code(&warm);
    restore::settle(SystemTime::now());
    warm.start_daemon(warm.root());
    warm.run(VALIDATE);
    warm
}

/// Writes a `.gitignore` that ignores no file of the corpus into the directory of its first
/// code file, which holds no document.
fn gitignore_in_code(warm: &Warm) {
    let directory = code(warm, 0).parent().unwrap().to_owned();
    fs::write(directory.join(".gitignore"), "/never.rs\n").unwrap();
}

/// VER-WORK-COUNTERS: one code file edited, then a request that reads code: that file read,
/// hashed and scanned, and no document parsed. The watcher's derivation, if any, reads no code.
#[test]
fn an_edited_code_file_is_scanned_again_and_no_document_is_parsed() {
    let warm = start();
    edit_code(&code(&warm, 0), "the edit");
    let work = warm.run(VALIDATE);
    let expected = [
        ("file_reads", 1),
        ("hashes", 1),
        ("code_scans", 1),
        ("parses", 0),
        ("snapshot_builds", 0),
    ];
    assert_counts(&work, &expected, "validate after the edit");
}

/// A code file edited to the contents of another: its contents are read and hashed, and its
/// markers are those of the other's scan, by key (DES-INPUTS-AND-KEYS).
#[test]
fn a_code_file_edited_to_the_contents_of_another_is_not_scanned() {
    let warm = start();
    let other = fs::read(code(&warm, 1)).unwrap();
    assert_ne!(fs::read(code(&warm, 0)).unwrap(), other, "the case");
    fs::write(code(&warm, 0), other).unwrap();
    let work = warm.run(VALIDATE);
    let expected = [
        ("file_reads", 1),
        ("hashes", 1),
        ("code_scans", 0),
        ("parses", 0),
    ];
    assert_counts(&work, &expected, "validate after the copy");
}

/// Code files edited, then a request that reads no code: no code file read, hashed or scanned,
/// nor the `.gitignore` on the way to them; the next request that reads code scans each edited
/// file once.
#[test]
fn code_edited_before_a_request_that_reads_no_code_is_scanned_by_the_next_that_does() {
    let warm = start();
    edit_code(&code(&warm, 0), "the first file");
    edit_code(&code(&warm, 1), "the second file");
    let work = warm.run(LIST);
    let mut expected = UNCHANGED.to_vec();
    expected.push(("code_scans", 0));
    assert_counts(&work, &expected, "a list after the edits");
    let work = warm.run(VALIDATE);
    let expected = [
        ("file_reads", 2),
        ("hashes", 2),
        ("code_scans", 2),
        ("parses", 0),
    ];
    assert_counts(&work, &expected, "validate after the list");
}

/// A document edited, then `relation list`, which reads code: no code scan.
#[test]
fn a_document_edited_then_relation_list_scans_no_code() {
    let warm = start();
    let path = document(&warm.corpus);
    let relations = ["relation", "list", warm.corpus.first_id.as_str()];
    let mut changed = SystemTime::now();
    decided("a document edited, then relation list", |run| {
        let seen = warm.settled(changed);
        edit_in_place(&path, run);
        changed = derived::changed(&[&path]);
        let (work, before) = warm.reports.around(seen, || warm.run(&relations))?;
        let reads = reads_in(Case::of(&before, changed), "the edited document");
        let expected = [
            ("file_reads", reads),
            ("hashes", reads),
            ("parses", 1),
            ("snapshot_builds", 1),
            ("code_scans", 0),
        ];
        assert_counts(&work, &expected, "relation list after the edit");
        Ok(())
    });
}

/// Python, declared beside the corpus's Rust, with two scripts of distinct contents.
const PYTHON: &str = "\n[code.languages.python]\ninclude = [\"scripts/**/*.py\"]\n\
                      comment_prefixes = [\"#\"]\n";

/// [`start`] on a corpus that declares [`PYTHON`] too, with `home` as the `HOME` of every run
/// when given.
fn start_with_python(home: Option<tempfile::TempDir>) -> Warm {
    let mut warm = Warm::new();
    gitignore_in_code(&warm);
    let config = warm.root().join(".mara/config.toml");
    let declared = fs::read_to_string(&config).unwrap() + PYTHON;
    fs::write(&config, declared).unwrap();
    fs::create_dir(warm.root().join("scripts")).unwrap();
    for name in ["one", "two"] {
        let script = format!("def {name}():\n    return 1\n");
        fs::write(warm.root().join(format!("scripts/{name}.py")), script).unwrap();
    }
    if let Some(home) = home {
        warm.with_home(home);
    }
    restore::settle(SystemTime::now());
    warm.start_daemon(warm.root());
    warm.run(VALIDATE);
    warm
}

/// A language's comment prefixes changed in the configuration: each distinct content of that
/// language's files scanned once, none of the other language's, and no document parsed.
#[test]
fn changed_comment_prefixes_scan_only_that_languages_files() {
    let warm = start_with_python(None);
    let config = warm.root().join(".mara/config.toml");
    let changes: [(&str, &str, &str, u64); 2] = [
        (
            "Rust",
            "comment_prefixes = [\"//\"]",
            "comment_prefixes = [\"//\", \"///\"]",
            warm.corpus.code_contents as u64,
        ),
        (
            "Python",
            "comment_prefixes = [\"#\"]",
            "comment_prefixes = [\"#\", \"##\"]",
            2,
        ),
    ];
    let mut changed = SystemTime::now();
    for (language, from, to, scans) in changes {
        warm.settled(changed);
        let text = fs::read_to_string(&config).unwrap();
        assert_eq!(text.matches(from).count(), 1, "{language}: {text}");
        fs::write(&config, text.replacen(from, to, 1)).unwrap();
        changed = derived::changed(&[&config]);
        let work = warm.run(VALIDATE);
        let expected = [("code_scans", scans), ("parses", 0)];
        assert_counts(&work, &expected, &format!("{language}'s prefixes changed"));
    }
}

/// A language's grammar added by `code add`, from a loopback repository (VER-CODE-ADD): each
/// distinct content of that language's files scanned once, at the grammar level, none of the
/// other language's, and no document parsed. The daemon finds the grammar in the cache of the
/// `HOME` it started with (DES-GRAMMAR-ASSETS).
#[test]
fn a_grammar_added_by_code_add_scans_only_that_languages_files() {
    let home = tempfile::Builder::new()
        .prefix("mara-home-")
        .tempdir()
        .expect("create a temporary directory");
    let warm = start_with_python(Some(home));
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm, tags);
    let url = server.url();
    let add = [
        "code",
        "add",
        "rust",
        "--repository",
        url.as_str(),
        "--version",
        version,
    ];
    warm.run(&add);
    let work = warm.run(VALIDATE);
    let expected = [
        ("code_scans", warm.corpus.code_contents as u64),
        ("parses", 0),
    ];
    assert_counts(&work, &expected, "validate after code add");
    // At the grammar level: a marker is owned by the function after it.
    let (line, _) = marker(&warm);
    let got = crate::cli::run_with(
        warm.root(),
        &["--format", "json", "relation", "list", &line],
        &warm.env(),
        0,
    );
    let got: serde_json::Value = serde_json::from_slice(&got.output.stdout).unwrap();
    assert_ne!(got["node"]["declaration"], serde_json::Value::Null, "{got}");
}

/// The location `<path>:<line>` of the first marker of the corpus's first code file that comes
/// right before a function, and the ID it names.
fn marker(warm: &Warm) -> (String, String) {
    let path = &warm.corpus.code_files[0];
    let text = fs::read_to_string(warm.root().join(path)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let at = (0..lines.len() - 1)
        .find(|&i| {
            lines[i].starts_with("// @mara implements ") && lines[i + 1].starts_with("pub fn")
        })
        .expect("a marker before a function");
    let id = lines[at]
        .trim_start_matches("// @mara implements ")
        .to_owned();
    (format!("{path}:{}", at + 1), id)
}

/// `item rename` that rewrites markers: the write scans no code, since it reads back no code file
/// (DES-CODE-READERS), and the next request that reads code scans each distinct content it
/// wrote into code once, and no other code file.
#[test]
fn a_rename_that_rewrites_markers_scans_only_the_code_it_wrote() {
    let warm = start();
    let (_, id) = marker(&warm);
    let renamed = format!("{id}-RENAMED");
    let mut work = WorkCounts::default();
    let rename = ["item", "rename", id.as_str(), renamed.as_str()];
    let written = Written::learn(warm.root(), || work = warm.run(&rename));
    assert!(written.code_files() > 1, "the rename rewrote markers");
    assert_counts(&work, &[("code_scans", 0)], "the rename");
    let distinct = written.distinct_code();
    let work = warm.run(VALIDATE);
    assert_counts(
        &work,
        &[("code_scans", distinct), ("parses", 0)],
        "validate after it",
    );
}

/// The row of a code file added, one removed and one renamed with its contents unchanged: the
/// new path is read and hashed, and only new contents are scanned.
#[test]
fn an_added_removed_or_renamed_code_file_is_scanned_only_when_its_contents_are_new() {
    let warm = start();
    let path = code(&warm, 0);
    let added = path.with_file_name("added.rs");
    let renamed = path.with_file_name("renamed.rs");
    let mut changed = SystemTime::now();
    let changes: [(&str, Change, u64, u64); 3] = [
        (
            "added",
            Box::new(|| fs::write(&added, "pub fn added() {}\n").unwrap()),
            1,
            1,
        ),
        (
            "removed",
            Box::new(|| fs::remove_file(&added).unwrap()),
            0,
            0,
        ),
        (
            "renamed",
            Box::new(|| fs::rename(&path, &renamed).unwrap()),
            1,
            0,
        ),
    ];
    for (what, change, reads, scans) in changes {
        warm.settled(changed);
        change();
        changed = SystemTime::now();
        let work = warm.run(VALIDATE);
        let expected = [
            ("file_reads", reads),
            ("hashes", reads),
            ("parses", 0),
            ("code_scans", scans),
        ];
        assert_counts(&work, &expected, &format!("a code file {what}"));
    }
}

/// `get` of a code location checks the file against the contents the session keeps of it that
/// a check keeps, those whose stat is the file's and whose check settled it: here a scan of the
/// file's new contents, after an earlier `get` kept the old ones.
#[test]
fn a_code_location_after_a_scan_of_its_new_contents_reads_nothing() {
    let warm = start();
    let path = &warm.corpus.code_files[0];
    let line = format!("{path}:2");
    let get = ["get", line.as_str()];
    warm.run(&get);
    let absolute = warm.root().join(path);
    edit_code(&absolute, "the scan");
    warm.settled(derived::changed(&[&absolute]));
    let work = warm.run(VALIDATE);
    assert_counts(&work, &[("file_reads", 1), ("code_scans", 1)], "the scan");
    let work = warm.run(&get);
    assert_eq!(work, WorkCounts::default(), "get after the scan");
}
