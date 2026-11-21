//! The `get` row of VER-CODE-WORK-COUNTERS (`docs/code/verification.mara.md`): `get` of a code
//! location parses no document and builds no snapshot, at the basic and at the grammar level,
//! cold in direct mode and warm through a daemon the test starts, whose session keeps its
//! snapshot and reads a code file as it is on disk now (REQ-NEVER-STALE). `get` of a document
//! reads no code, cold and warm (DES-CODE-READERS).

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
    }
}

/// Gives the corpus a code tree whose listing reads a file and reports a problem: a
/// `.gitignore` in the directory of its first code file, and a `.gitignore` that is a directory
/// below it. A request that lists code reads and hashes the first and reports the second.
fn code_tree_with_a_problem(corpus: &Corpus) {
    let path = &corpus.code_files[0];
    let directory = corpus
        .root()
        .join(path.rsplit_once('/').expect("a code directory").0);
    fs::write(directory.join(".gitignore"), "*.tmp\n").unwrap();
    fs::create_dir_all(directory.join("hidden/.gitignore")).unwrap();
}

/// The addresses of a `get` that reads no code: the path of the document of the corpus's first
/// item, a line in it, and a path that no code include pattern matches and that is no input.
/// Each with whether it succeeds.
fn document_addresses(corpus: &Corpus) -> [(String, bool); 3] {
    let document = &corpus.ids[0].1;
    [
        (document.clone(), true),
        (format!("{document}:1"), true),
        ("notes.txt".to_owned(), false),
    ]
}

/// The diagnostics of a JSON result or failure.
fn diagnostics(result: &Value) -> &Value {
    match result.get("error") {
        Some(error) => &error["diagnostics"],
        None => &result["diagnostics"],
    }
}

/// DES-CODE-READERS: `get` of a document path, of a line in a document or of a path that no
/// code include pattern matches reads the documents only, in a project with a code language
/// whose code tree holds a `.gitignore` and a problem, at the basic and at the grammar level.
/// It reads and hashes what `get` by ID does and reports no diagnostic of the code tree, which
/// a listing of the code files would report.
#[test]
fn cold_get_of_a_document_reads_no_code() {
    for grammar in [false, true] {
        let corpus = Corpus::generate(1, SEED);
        code_tree_with_a_problem(&corpus);
        let home = level(&corpus, grammar);
        let env: Vec<(&str, &Path)> = home.iter().map(|h| ("HOME", h.path())).collect();
        let validate = ["--format", "json", "project", "validate"];
        let validated = cli::run_any(corpus.root(), &validate, &env);
        let validated: Value = serde_json::from_slice(&validated.output.stdout).unwrap();
        assert!(
            validated
                .to_string()
                .contains("hidden/.gitignore is not a regular file"),
            "a listing of the code files reports the problem: {validated}"
        );
        let by_id = cli::run_with(corpus.root(), &["get", &corpus.first_id], &env, 0).work;
        assert_eq!(by_id.snapshot_builds, 1, "{by_id:?}, grammar {grammar}");
        for (address, succeeds) in document_addresses(&corpus) {
            let args = ["--format", "json", "get", &address];
            let run = cli::run_with(corpus.root(), &args, &env, i32::from(!succeeds));
            assert_eq!(run.work, by_id, "get {address}, grammar {grammar}");
            let result: Value = serde_json::from_slice(&run.output.stdout).unwrap();
            assert_eq!(diagnostics(&result), &Value::Array(Vec::new()), "{result}");
        }
    }
}

/// [`cold_get_of_a_document_reads_no_code`] through a daemon the test starts, at both levels:
/// once the session holds the snapshot and the inputs are settled, each `get` that the daemon
/// serves reads nothing, and none reports a diagnostic of the code tree.
#[cfg(target_os = "linux")]
#[test]
fn warm_get_of_a_document_reads_no_code() {
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
        code_tree_with_a_problem(&corpus);
        let home = level(&corpus, grammar);
        let home = home.as_ref().map_or(runtime.path(), |home| home.path());
        let env = [
            ("XDG_RUNTIME_DIR", runtime.path()),
            ("TMPDIR", runtime.path()),
            ("HOME", home),
        ];
        let run = |args: &[&str]| cli::run_any(corpus.root(), args, &env);
        let served = || {
            let status = run(&["--format", "json", "daemon", "status"]);
            let status: Value = serde_json::from_slice(&status.output.stdout).unwrap();
            status["served"].as_u64().expect("a running daemon's count")
        };
        settle(SystemTime::now());
        run(&["daemon", "start"]);
        // The session holds the snapshot.
        run(&["get", &corpus.first_id]);
        assert_eq!(run(&["get", &corpus.first_id]).work, WorkCounts::default());
        for (address, succeeds) in document_addresses(&corpus) {
            let before = served();
            let run = run(&["--format", "json", "get", &address]);
            assert_eq!(served(), before + 1, "the daemon served get {address}");
            assert_eq!(
                run.output.status.code(),
                Some(i32::from(!succeeds)),
                "get {address}, grammar {grammar}"
            );
            let message = format!("warm get {address}, grammar {grammar}");
            assert_eq!(run.work, WorkCounts::default(), "{message}");
            let result: Value = serde_json::from_slice(&run.output.stdout).unwrap();
            assert_eq!(diagnostics(&result), &Value::Array(Vec::new()), "{result}");
        }
        run(&["daemon", "stop"]);
    }
    assert!(daemons.running().is_empty(), "a daemon runs after `stop`");
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
        // The same request again: the session keeps the file's contents and its scan.
        let (work, _) = get(&line);
        assert_eq!(
            work,
            WorkCounts::default(),
            "warm get {line} again, grammar {grammar}"
        );

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
