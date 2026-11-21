//! The `get` row of VER-CODE-WORK-COUNTERS (`docs/code/verification.mara.md`): `get` of a code
//! location parses no document and builds no snapshot, at the basic and at the grammar level,
//! cold in direct mode and warm through a daemon the test starts, whose session keeps its
//! snapshot and reads a code file as it is on disk now (REQ-NEVER-STALE).

use std::fs;
use std::path::Path;

use mara_engine::work::WorkCounts;
use serde_json::Value;

use crate::corpus::Corpus;
use crate::work_counters::install_rust_grammar;
use crate::{SEED, cli};

/// A code location inside a function body of the corpus's first code file, and the file.
fn addresses(corpus: &Corpus) -> (String, String) {
    let path = &corpus.code_files[0];
    let code = fs::read_to_string(corpus.root().join(path)).unwrap();
    let inside = code
        .lines()
        .position(|line| line.starts_with("    let mut value"));
    let line = format!("{path}:{}", inside.expect("a function body") + 1);
    (line, path.clone())
}

/// `HOME` with the Rust grammar pinned and cached at the grammar level, `None` at the basic.
fn level(corpus: &Corpus, grammar: bool) -> Option<tempfile::TempDir> {
    grammar.then(|| install_rust_grammar(corpus.root()))
}

#[test]
fn cold_get_of_code_reads_no_document() {
    for grammar in [false, true] {
        let corpus = Corpus::generate(1, SEED);
        let home = level(&corpus, grammar);
        let env: Vec<(&str, &Path)> = home.iter().map(|h| ("HOME", h.path())).collect();
        let (line, file) = addresses(&corpus);
        for address in [&line, &file] {
            let run = cli::run_with(
                corpus.root(),
                &["--format", "json", "get", address],
                &env,
                0,
            );
            // The configuration, the schema, the root `.gitignore` and that code file.
            let expected = WorkCounts {
                file_reads: 4,
                hashes: 4,
                code_scans: u64::from(grammar),
                ..WorkCounts::default()
            };
            assert_eq!(run.work, expected, "get {address}, grammar {grammar}");
            let result: Value = serde_json::from_slice(&run.output.stdout).unwrap();
            assert_eq!(result["node"]["kind"], "code", "{result}");
            if grammar && address == &line {
                assert!(result["node"]["declaration"].is_string(), "{result}");
            }
        }
        // An address that is no code file reads the documents as before: a document's path
        // costs what its item's ID does.
        let document = &corpus.ids[0].1;
        let by_path = cli::run_with(corpus.root(), &["get", document], &env, 0).work;
        let by_id = cli::run_with(corpus.root(), &["get", &corpus.first_id], &env, 0).work;
        assert_eq!(by_path, by_id, "get {document}, grammar {grammar}");
        assert_eq!(by_path.snapshot_builds, 1);
    }
}

// The warm case finds the daemons of a test in `/proc`.
#[cfg(target_os = "linux")]
#[test]
fn warm_get_of_code_reads_no_document() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::SystemTime;

    use crate::daemons::Daemons;
    use crate::restore::settle;

    // Short enough for the socket path.
    let runtime = tempfile::Builder::new()
        .prefix("mara-run-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .expect("create a temporary directory");
    let daemons = Daemons::new(runtime.path(), None);
    for grammar in [false, true] {
        let corpus = Corpus::generate(1, SEED);
        let home = level(&corpus, grammar);
        let home = home.as_ref().map_or(runtime.path(), |home| home.path());
        let env = [
            ("XDG_RUNTIME_DIR", runtime.path()),
            ("TMPDIR", runtime.path()),
            ("HOME", home),
        ];
        let run = |args: &[&str]| cli::run_with(corpus.root(), args, &env, 0);
        let get = |address: &str| {
            let run = run(&["--format", "json", "get", address]);
            let result: Value = serde_json::from_slice(&run.output.stdout).unwrap();
            (run.work, result)
        };
        settle(SystemTime::now());
        run(&["daemon", "start"]);
        // The session holds the snapshot.
        run(&["get", &corpus.first_id]);
        assert_eq!(run(&["get", &corpus.first_id]).work, WorkCounts::default());

        let (line, file) = addresses(&corpus);
        let (work, _) = get(&line);
        let expected = WorkCounts {
            file_reads: 1,
            hashes: 1,
            code_scans: u64::from(grammar),
            ..WorkCounts::default()
        };
        assert_eq!(work, expected, "warm get {line}, grammar {grammar}");

        // A code file changed on disk is read as it is now.
        let absolute = corpus.root().join(&file);
        let mut code = fs::read_to_string(&absolute).unwrap();
        code.push_str("// changed after the session opened\n");
        fs::write(&absolute, &code).unwrap();
        let (work, result) = get(&file);
        assert_eq!(
            work, expected,
            "warm get {file} after a change, grammar {grammar}"
        );
        assert!(
            result["content"]
                .as_str()
                .unwrap()
                .contains("changed after"),
            "{result}"
        );

        // The session kept its snapshot.
        let work = run(&["get", &corpus.first_id]).work;
        assert_eq!((work.parses, work.snapshot_builds), (0, 0), "{work:?}");
        run(&["daemon", "stop"]);
    }
    assert!(daemons.running().is_empty(), "a daemon runs after `stop`");
}
