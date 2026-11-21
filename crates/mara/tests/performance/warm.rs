//! The warm rows of VER-WORK-COUNTERS (`docs/verification.mara.md`) at 1×, through the real CLI
//! against a daemon the test starts with its own runtime directory (VER-DAEMON-START), every
//! change made while it watches. Each row is its own test and starts settled ([`Warm::settled`]).
//!
//! A row's counts are those of the response after its situation, background derivations
//! included. The daemon reports each derivation to a file of the test (`MARA_TEST_DERIVED`, debug
//! builds only), so a test waits for the watcher instead of a deadline and learns which case of
//! its row a request is in (`derived::Case`): no derivation before it, one that checked the
//! change less than 2 s after it, or one that checked it later. The expected counts are the
//! row's for that case. A run in which a derivation that did work was reported while the request
//! ran cannot be judged and is repeated (`derived::decided`).

#![cfg(debug_assertions)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use mara_engine::work::WorkCounts;
use serde_json::json;

use crate::cli::{self, Reads};
use crate::corpus::Corpus;
use crate::daemons::Daemons;
use crate::derived::{self, Case, Reports, decided, until_settled};
use crate::restore::{self, Written};
use crate::{SEED, mcp};

const LIST: &[&str] = &["item", "list"];
const VALIDATE: &[&str] = &["project", "validate"];

/// A 1× corpus and a runtime directory of the test, whose daemons are killed when it is dropped.
struct Warm {
    /// Kills the daemons when the test ends, before the runtime directory goes.
    _daemons: Daemons,
    reports: Reports,
    corpus: Corpus,
    runtime: tempfile::TempDir,
}

impl Warm {
    /// A corpus whose files last changed 2 s ago, and no daemon yet.
    fn new() -> Warm {
        let corpus = Corpus::generate(1, SEED);
        // Short enough for the socket path; mode 0700, as DES-DAEMON-LIFECYCLE requires.
        let runtime = tempfile::Builder::new()
            .prefix("mara-warm-")
            .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .tempdir()
            .expect("create a temporary directory");
        restore::settle(SystemTime::now());
        Warm {
            _daemons: Daemons::new(runtime.path(), None),
            reports: Reports::new(runtime.path().join("derived")),
            corpus,
            runtime,
        }
    }

    /// [`Warm::new`] with a daemon started in the corpus, whose session has validated the
    /// project, every input read by a check 2 s after its last change.
    fn start() -> Warm {
        let warm = Warm::new();
        warm.start_daemon(warm.root());
        warm.run(VALIDATE);
        warm
    }

    fn root(&self) -> &Path {
        self.corpus.root()
    }

    /// The environment of every run: the test's runtime directory.
    fn env(&self) -> [(&'static str, &Path); 2] {
        [
            ("XDG_RUNTIME_DIR", self.runtime.path()),
            ("TMPDIR", self.runtime.path()),
        ]
    }

    /// Starts the daemon in `directory`, reporting its derivations to [`Warm::reports`].
    fn start_daemon(&self, directory: &Path) {
        let mut env = self.env().to_vec();
        env.push(("MARA_TEST_DERIVED", self.reports.path()));
        cli::run_with(directory, &["daemon", "start"], &env, 0);
    }

    /// `mara <args>` in the corpus, sent to the daemon: the work it reports. It exits 0.
    fn run(&self, args: &[&str]) -> WorkCounts {
        self.run_in(self.root(), args, 0)
    }

    fn run_in(&self, directory: &Path, args: &[&str], status: i32) -> WorkCounts {
        cli::run_with(directory, args, &self.env(), status).work
    }

    /// Settles the files changed at `changed` and the session's record of them: a request 2 s
    /// after the change. The number of derivations reported then, after which the next
    /// response's begin.
    fn settled(&self, changed: SystemTime) -> usize {
        until_settled(changed);
        self.run(LIST);
        self.reports.seen()
    }
}

/// Each of `expected`, a counter's name and value, in `work`.
#[track_caller]
fn assert_counts(work: &WorkCounts, expected: &[(&str, u64)], what: &str) {
    for &(name, value) in expected {
        let actual = match name {
            "file_reads" => work.file_reads,
            "hashes" => work.hashes,
            "parses" => work.parses,
            "snapshot_builds" => work.snapshot_builds,
            "code_scans" => work.code_scans,
            "subprocess_spawns" => work.subprocess_spawns,
            "check_steps" => work.check_steps,
            "program_runs" => work.program_runs,
            _ => panic!("no counter {name}"),
        };
        assert_eq!(actual, value, "{name} of {what}: {work:?}");
    }
}

/// A change a test makes to the corpus.
type Change<'a> = Box<dyn Fn() + 'a>;

/// No file read or hashed, nothing parsed or built: a request on unchanged inputs.
const UNCHANGED: [(&str, u64); 4] = [
    ("file_reads", 0),
    ("hashes", 0),
    ("parses", 0),
    ("snapshot_builds", 0),
];

/// The arguments of each case of `cli::cases` that reads and does not write, with its exit
/// status.
fn reads(corpus: &Corpus) -> Vec<(Vec<String>, i32)> {
    (cli::cases(corpus).into_iter())
        .filter(|case| case.reads != Reads::Writes)
        .map(|case| (case.args, case.status))
        .collect()
}

/// A document of the first cluster, which every test edits.
fn document(corpus: &Corpus) -> PathBuf {
    corpus.root().join(&corpus.ids[0].1)
}

/// Appends a paragraph naming `run` to `path`, in place: new contents in every run.
fn edit_in_place(path: &Path, run: usize) {
    let mut contents = fs::read_to_string(path).unwrap();
    contents.push_str(&format!("\nEdited in run {run}.\n"));
    fs::write(path, contents).unwrap();
}

/// The same edit as [`edit_in_place`], saved as editors do: a new file renamed over `path`.
fn edit_by_rename(path: &Path, run: usize) {
    let mut contents = fs::read_to_string(path).unwrap();
    contents.push_str(&format!("\nSaved in run {run}.\n"));
    let staged = path.with_extension("swp");
    fs::write(&staged, contents).unwrap();
    fs::rename(&staged, path).unwrap();
}

/// The reads, and hashes, of an input that a request reads and that the watcher's derivation
/// read too when it checked the input before the request, less than 2 s after the change; one
/// that checked later settled it, and the request read it no more.
fn reads_in(case: Case, what: &str) -> u64 {
    match case {
        Case::None => 1,
        Case::Early(derived) => {
            assert_eq!(derived.file_reads, 1, "{what}: {derived:?}");
            2
        }
        Case::Late(derived) => {
            assert_eq!(derived.file_reads, 1, "{what}: {derived:?}");
            1
        }
    }
}

#[test]
fn the_same_read_request_again_does_no_work() {
    let warm = Warm::start();
    for (args, status) in reads(&warm.corpus) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        warm.run_in(warm.root(), &args, status);
        let again = warm.run_in(warm.root(), &args, status);
        // Every request reads a file that `--input` names, which is no input of the session
        // (DES-DAEMON-SESSIONS).
        let supplied = args.iter().filter(|arg| arg.contains("=@")).count() as u64;
        let expected = WorkCounts {
            file_reads: supplied,
            ..WorkCounts::default()
        };
        assert_eq!(again, expected, "mara {args:?} again");
    }
}

#[test]
fn another_request_on_unchanged_inputs_reads_nothing() {
    let warm = Warm::start();
    for (args, status) in reads(&warm.corpus) {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let work = warm.run_in(warm.root(), &args, status);
        let supplied = args.iter().filter(|arg| arg.contains("=@")).count() as u64;
        let mut expected = UNCHANGED;
        expected[0].1 = supplied;
        assert_counts(&work, &expected, &format!("mara {args:?}"));
        // The next case is another request again.
        warm.run(&["schema", "list"]);
    }
}

#[test]
fn a_request_finds_the_evaluations_that_an_earlier_request_ran_with_the_same_keys() {
    let warm = Warm::start();
    // `project validate` ran `knowledge.dl` with the snapshot's facts, as its queries do, and
    // compiled `readiness.dl` against empty facts (DES-RULE-EXECUTION): its first view runs it.
    let earlier: [(&[&str], u64); 5] = [
        (&["query", "execute", "requirement-basis"], 0),
        (&["query", "execute", "design-basis"], 0),
        (&["query", "view", "risk-treatment"], 1),
        (&["query", "execute", "risk-level"], 0),
        (&["query", "execute", "relevant-risks"], 0),
    ];
    for (args, runs) in earlier {
        let work = warm.run(args);
        assert_counts(&work, &[("program_runs", runs)], &format!("mara {args:?}"));
    }
}

/// The situation of the row: an input that a check read less than 2 s after its change, then
/// the request 2 s after it.
#[test]
fn an_input_read_before_it_settled_is_read_again_and_nothing_else() {
    let warm = Warm::start();
    let path = document(&warm.corpus);
    let mut changed = SystemTime::now();
    decided("an unsettled input", |run| {
        let seen = warm.settled(changed);
        edit_in_place(&path, run);
        changed = derived::changed(&[&path]);
        warm.reports.around(seen, || warm.run(LIST))?;
        if changed + Duration::from_secs(2) <= SystemTime::now() {
            return Err("the request after the edit came 2 s after it".to_owned());
        }
        let seen = warm.reports.seen();
        until_settled(changed);
        let (work, before) = warm.reports.around(seen, || warm.run(LIST))?;
        // A derivation after that request's check reads the input only to settle it, 2 s or
        // more after the change, and then the request reads it no more.
        match Case::of(&before, changed) {
            Case::None => {}
            Case::Early(derived) => assert_eq!(derived.file_reads, 0, "{derived:?}"),
            Case::Late(derived) => assert_eq!(derived.file_reads, 1, "{derived:?}"),
        }
        let expected = [
            ("file_reads", 1),
            ("hashes", 1),
            ("parses", 0),
            ("snapshot_builds", 0),
        ];
        assert_counts(&work, &expected, "the request 2 s later");
        Ok(())
    });
}

#[test]
fn a_touched_input_is_read_again_and_nothing_else() {
    let warm = Warm::start();
    let path = document(&warm.corpus);
    let mut changed = SystemTime::now();
    decided("a touched input", |_| {
        let seen = warm.settled(changed);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now())
            .unwrap();
        changed = derived::changed(&[&path]);
        let (work, before) = warm.reports.around(seen, || warm.run(VALIDATE))?;
        let reads = reads_in(Case::of(&before, changed), "a touched input");
        let expected = [
            ("file_reads", reads),
            ("hashes", reads),
            ("parses", 0),
            ("snapshot_builds", 0),
            ("program_runs", 0),
        ];
        assert_counts(&work, &expected, "the request after the touch");
        Ok(())
    });
}

/// In place and by rename, each with the request sent at once and 2 s later, after the
/// watcher's derivation.
#[test]
fn an_edited_document_is_read_by_the_request_and_by_a_derivation_before_it() {
    let warm = Warm::start();
    let path = document(&warm.corpus);
    let mut changed = SystemTime::now();
    let mut run = 0;
    for (by_rename, later) in [(false, false), (true, false), (false, true), (true, true)] {
        let what = format!("an edit, by rename {by_rename}, 2 s later {later}");
        decided(&what, |_| {
            run += 1;
            let seen = warm.settled(changed);
            match by_rename {
                false => edit_in_place(&path, run),
                true => edit_by_rename(&path, run),
            }
            changed = derived::changed(&[&path]);
            if later {
                warm.reports.wait_after(seen);
                until_settled(changed);
            }
            let (work, before) = warm.reports.around(seen, || warm.run(VALIDATE))?;
            let case = Case::of(&before, changed);
            if later {
                assert_ne!(case, Case::None, "{what}: the watcher derived first");
            }
            let reads = reads_in(case, &what);
            let expected = [
                ("file_reads", reads),
                ("hashes", reads),
                ("parses", 1),
                ("snapshot_builds", 1),
                ("code_scans", 0),
            ];
            assert_counts(&work, &expected, &what);
            Ok(())
        });
    }
}

#[test]
fn an_added_removed_or_renamed_document_is_parsed_only_when_its_contents_are_new() {
    let warm = Warm::start();
    let path = document(&warm.corpus);
    let added = path.with_file_name("added.mara.md");
    let renamed = path.with_file_name("renamed.mara.md");
    let mut changed = SystemTime::now();
    let changes: [(&str, Change, u64); 3] = [
        (
            "added",
            Box::new(|| fs::write(&added, "# Added\n\nNew narrative.\n").unwrap()),
            1,
        ),
        ("removed", Box::new(|| fs::remove_file(&added).unwrap()), 0),
        (
            "renamed",
            Box::new(|| fs::rename(&path, &renamed).unwrap()),
            0,
        ),
    ];
    for (what, change, parses) in changes {
        warm.settled(changed);
        change();
        changed = SystemTime::now();
        let work = warm.run(LIST);
        let expected = [("parses", parses), ("snapshot_builds", 1)];
        assert_counts(&work, &expected, &format!("a document {what}"));
    }
}

/// The row of VER-CODE-WORK-COUNTERS (`docs/code/verification.mara.md`) of a code file added,
/// one removed and one renamed with its contents unchanged: the new path is read and hashed,
/// and only new contents are scanned.
#[test]
fn an_added_removed_or_renamed_code_file_is_scanned_only_when_its_contents_are_new() {
    let warm = Warm::start();
    let path = warm.root().join(&warm.corpus.code_files[0]);
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

#[test]
fn an_edited_schema_or_configuration_builds_the_snapshot_again_and_parses_nothing() {
    let warm = Warm::start();
    let mut changed = SystemTime::now();
    let schema = warm.root().join(".mara/schema.toml");
    let config = warm.root().join(".mara/config.toml");
    for (path, from, to) in [
        (
            &schema,
            "id_prefix = \"REQ-\"",
            "id_prefix = \"REQ-\"\n# A note.",
        ),
        (&config, "[documents]", "# A note.\n[documents]"),
    ] {
        warm.settled(changed);
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains(from), "{path:?}");
        fs::write(path, text.replacen(from, to, 1)).unwrap();
        changed = SystemTime::now();
        let work = warm.run(LIST);
        let expected = [("parses", 0), ("snapshot_builds", 1)];
        assert_counts(&work, &expected, &format!("{path:?} edited"));
    }
}

/// Each write case of `cli::cases`: new contents in each file it writes.
#[test]
fn a_write_parses_each_distinct_content_it_writes_and_no_other_file() {
    let warm = Warm::start();
    let mut changed = SystemTime::now();
    for case in cli::cases(&warm.corpus) {
        if case.reads != Reads::Writes {
            continue;
        }
        let args: Vec<&str> = case.args.iter().map(String::as_str).collect();
        warm.settled(changed);
        let mut work = WorkCounts::default();
        let written = Written::learn(warm.root(), || work = warm.run(&args));
        let expected = [
            ("parses", written.distinct_documents()),
            ("snapshot_builds", 1),
        ];
        assert_counts(&work, &expected, &format!("mara {args:?}"));
        written.restore();
        changed = SystemTime::now();
    }
}

/// A copy of the corpus's first document with one more item, `REQ-EXTRA`, after it: deleting that
/// item gives the copy the contents of the original. The copy's path.
fn copy_with_extra(corpus: &Corpus) -> PathBuf {
    let original = document(corpus);
    let contents = fs::read_to_string(&original).unwrap();
    let copy = original.with_file_name("copy.mara.md");
    let extra = "\n---\n\n<a id=\"01M7A0000000000000000000ZZ\"></a>\n\n### Extra\n\n\
                 | Type | Requirement |\n| --- | --- |\n| ID | REQ-EXTRA |\n| Status | Draft |\n\n\
                 Text.\n";
    fs::write(&copy, format!("{contents}{extra}")).unwrap();
    copy
}

/// The delete of the only item that tells two documents apart gives one the contents of the
/// other: the read-back takes the other's parse.
#[test]
fn a_write_that_gives_a_file_the_contents_of_another_input_parses_nothing() {
    let warm = Warm::new();
    let copy = copy_with_extra(&warm.corpus);
    restore::settle(SystemTime::now());
    warm.start_daemon(warm.root());
    warm.run(LIST);
    let work = warm.run(&["item", "delete", "REQ-EXTRA"]);
    let original = fs::read_to_string(document(&warm.corpus)).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), original, "the case");
    let expected = [("parses", 0), ("snapshot_builds", 1)];
    assert_counts(&work, &expected, "the delete");
}

/// A write sent to a daemon that has no session of the project yet, giving the file it writes
/// new contents: the counts of a cold request, 1 hash of the written file and its parse by the
/// read-back, then the snapshot built from it.
#[test]
fn a_write_that_is_the_sessions_first_request_counts_a_cold_request_and_its_read_back() {
    let warm = Warm::new();
    warm.start_daemon(warm.runtime.path());
    let id = &warm.corpus.ids[0].0;
    let args = ["item", "update", id, "--title", "Updated by the harness"];
    let mut work = WorkCounts::default();
    let written = Written::learn(warm.root(), || work = warm.run(&args));
    assert_eq!(written.contents.len(), 1, "the update writes one file");
    let documents = warm.corpus.documents as u64;
    let read = documents + 3 + crate::corpus::PROGRAMS;
    let expected = WorkCounts {
        file_reads: read,
        hashes: read + 1,
        parses: documents + 1,
        snapshot_builds: 2,
        ..WorkCounts::default()
    };
    assert_eq!(work, expected, "the first request");
}

/// The same with a write that gives a file the contents of another document: no parse for it.
/// The delete reads and scans every code file, for markers that name the item.
#[test]
fn a_first_write_that_gives_a_file_the_contents_of_another_input_parses_nothing() {
    let warm = Warm::new();
    let copy = copy_with_extra(&warm.corpus);
    restore::settle(SystemTime::now());
    warm.start_daemon(warm.runtime.path());
    let work = warm.run(&["item", "delete", "REQ-EXTRA"]);
    let original = fs::read_to_string(document(&warm.corpus)).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), original, "the case");
    let corpus = &warm.corpus;
    let documents = corpus.documents as u64 + 1;
    let code = corpus.code_files.len() as u64;
    let read = documents + 3 + crate::corpus::PROGRAMS + code;
    let expected = WorkCounts {
        file_reads: read,
        hashes: read + 1,
        parses: documents,
        snapshot_builds: 2,
        code_scans: corpus.code_contents as u64,
        ..WorkCounts::default()
    };
    assert_eq!(work, expected, "the first request");
}

/// The request after a write, sent at once and 2 s later, after the watcher's derivation: the
/// written file is read again once, by the request's check when the derivation checked it less
/// than 2 s after the write, and by that derivation alone when it checked it later.
#[test]
fn the_request_after_a_write_reads_the_written_file_again() {
    let warm = Warm::start();
    let id = warm.corpus.ids[0].0.clone();
    let path = document(&warm.corpus);
    let mut changed = SystemTime::now();
    let mut run = 0;
    for later in [false, true] {
        let what = format!("the request after a write, 2 s later {later}");
        decided(&what, |_| {
            run += 1;
            warm.settled(changed);
            let title = format!("Updated in run {run}");
            let update = ["item", "update", &id, "--title", &title];
            let seen = warm.reports.seen();
            let write = warm.run(&update);
            let expected = WorkCounts {
                hashes: 1,
                parses: 1,
                snapshot_builds: 1,
                ..WorkCounts::default()
            };
            assert_eq!(write, expected, "the write");
            changed = derived::changed(&[&path]);
            if later {
                warm.reports.wait_after(seen);
                until_settled(changed);
            }
            let (work, before) = warm.reports.around(seen, || warm.run(LIST))?;
            match Case::of(&before, changed) {
                Case::None => {}
                Case::Early(derived) => {
                    assert_eq!(
                        derived.file_reads, 0,
                        "{what}: the watcher read {derived:?}"
                    );
                }
                Case::Late(derived) => {
                    assert_eq!(
                        derived.file_reads, 1,
                        "{what}: the watcher read {derived:?}"
                    );
                }
            }
            if later {
                assert_ne!(Case::of(&before, changed), Case::None, "{what}");
            }
            let expected = [
                ("file_reads", 1),
                ("hashes", 1),
                ("parses", 0),
                ("snapshot_builds", 0),
            ];
            assert_counts(&work, &expected, &what);
            Ok(())
        });
    }
}

#[test]
fn an_edited_program_runs_again_and_nothing_is_parsed() {
    let warm = Warm::start();
    // No other program includes it.
    let program = warm.root().join(".mara/rules/readiness.dl");
    warm.settled(SystemTime::now());
    let mut text = fs::read_to_string(&program).unwrap();
    text.push_str("// Edited.\n");
    fs::write(&program, text).unwrap();
    let work = warm.run(VALIDATE);
    let expected = [("program_runs", 1), ("parses", 0), ("snapshot_builds", 0)];
    assert_counts(&work, &expected, "validate after the edit");
}

/// After a document edit, a program run with the snapshot's facts runs again, and one run
/// against empty facts, whose outcome the session keeps, does not.
#[test]
fn after_a_document_edit_only_programs_given_the_snapshots_facts_run_again() {
    let warm = Warm::start();
    let gate = gate(&warm.corpus, crate::corpus::CANDIDATE);
    let gate: Vec<&str> = gate.iter().map(String::as_str).collect();
    warm.run(&gate);
    let path = document(&warm.corpus);
    warm.settled(SystemTime::now());
    edit_in_place(&path, 1);
    let work = warm.run(VALIDATE);
    // `knowledge.dl` holds the invariants, given the snapshot's facts; `readiness.dl` is
    // compiled against empty facts (DES-RULE-EXECUTION).
    assert_counts(&work, &[("program_runs", 1)], "validate after the edit");
    let work = warm.run(&gate);
    assert_counts(&work, &[("program_runs", 1)], "gate after the edit");
    // The document restored byte for byte: the snapshot's key is that of the outcomes the
    // session still keeps.
    let edited = fs::read_to_string(&path).unwrap();
    let original = edited.strip_suffix("\nEdited in run 1.\n").unwrap();
    warm.settled(derived::changed(&[&path]));
    fs::write(&path, original).unwrap();
    let work = warm.run(VALIDATE);
    assert_counts(&work, &[("program_runs", 0)], "validate after the restore");
    let work = warm.run(&gate);
    assert_counts(&work, &[("program_runs", 0)], "gate after the restore");
}

/// The `query gate` case of `cli::cases`, naming `candidate`.
fn gate(corpus: &Corpus, candidate: &str) -> Vec<String> {
    let case = cli::cases(corpus)
        .into_iter()
        .find(|case| case.args[1] == "gate");
    let mut args = case.expect("a gate case").args;
    let at = args
        .iter()
        .position(|arg| arg.starts_with("candidate="))
        .unwrap();
    args[at] = format!("candidate={candidate}");
    args
}

#[test]
fn a_gate_with_other_inputs_runs_its_program() {
    let warm = Warm::start();
    let gate = gate(&warm.corpus, crate::corpus::CANDIDATE);
    let gate: Vec<&str> = gate.iter().map(String::as_str).collect();
    warm.run(&gate);
    // Another candidate, whose results the file does not hold: the gate fails.
    let other = self::gate(&warm.corpus, "def");
    let other: Vec<&str> = other.iter().map(String::as_str).collect();
    let work = warm.run_in(warm.root(), &other, 1);
    assert_counts(
        &work,
        &[("program_runs", 1)],
        "the gate of another candidate",
    );
}

/// Changes that invalidate nothing, each followed by the same request again.
#[test]
fn changes_that_are_no_inputs_invalidate_nothing() {
    let warm = Warm::start();
    let root = warm.root().to_owned();
    let write = |path: &str, contents: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    };
    let directory = Path::new(&warm.corpus.ids[0].1)
        .parent()
        .unwrap()
        .to_owned();
    let notes = directory.join("notes.txt");
    let notes = notes.to_str().unwrap();
    warm.run(LIST);
    let mut situations: Vec<(&str, Change)> = vec![
        (
            "a file beside the documents",
            Box::new(|| write(notes, "Not an input.\n")),
        ),
        (
            "ignored build output",
            Box::new(|| write("target/debug/app", "changed")),
        ),
        (
            "an ignored document",
            Box::new(|| write("docs/generated/index.mara.md", "Changed.\n")),
        ),
        (
            "an undeclared document",
            Box::new(|| write("notes/draft.mara.md", "Changed.\n")),
        ),
        (
            "a .git change",
            Box::new(|| write(".git/objects/00/new", "object")),
        ),
        (
            "a failed request",
            Box::new(|| {
                warm.run_in(warm.root(), &["get", "REQ-NO-SUCH"], 1);
            }),
        ),
    ];
    situations.push((
        "5 s without a request",
        Box::new(|| std::thread::sleep(Duration::from_secs(5))),
    ));
    for (what, situation) in situations {
        let seen = warm.reports.seen();
        situation();
        if what == "a file beside the documents" {
            // The watcher's derivation, in a directory it watches, finds nothing to do.
            for derived in warm.reports.wait_after(seen) {
                assert_eq!(derived.file_reads, 0, "{what}: {derived:?}");
            }
        }
        let work = warm.run(LIST);
        assert_eq!(work, WorkCounts::default(), "the request after {what}");
        // An evaluation that `project validate` ran is still kept.
        let work = warm.run(&["query", "execute", "requirement-basis"]);
        assert_eq!(work, WorkCounts::default(), "a query after {what}");
    }
}

#[test]
fn another_projects_session_leaves_this_one_unchanged() {
    let warm = Warm::start();
    warm.run(LIST);
    let other = Corpus::generate(1, SEED + 1);
    restore::settle(SystemTime::now());
    warm.run_in(other.root(), LIST, 0);
    edit_in_place(&document(&other), 1);
    warm.run_in(other.root(), LIST, 0);
    let work = warm.run(LIST);
    assert_eq!(
        work,
        WorkCounts::default(),
        "after the other project's edit"
    );
}

/// The request names the link with `--project`, from outside the project: the daemon serves it
/// in the session of the canonical root and opens no other.
#[test]
fn the_project_through_a_symbolic_link_is_the_same_session() {
    let warm = Warm::start();
    warm.run(LIST);
    let link = warm.runtime.path().join("link");
    std::os::unix::fs::symlink(warm.root(), &link).unwrap();
    let args = ["--project", link.to_str().unwrap(), "item", "list"];
    let work = warm.run_in(warm.runtime.path(), &args, 0);
    assert_eq!(work, WorkCounts::default(), "through the link");
    let status = ["--format", "json", "daemon", "status"];
    let status = cli::run_with(warm.runtime.path(), &status, &warm.env(), 0);
    let status: serde_json::Value = serde_json::from_slice(&status.output.stdout).unwrap();
    let root = fs::canonicalize(warm.root()).unwrap();
    assert_eq!(status["sessions"], json!([root]), "{status}");
}

/// `get` of a code location checks the file against the contents of the session's latest check
/// of it: here a scan of the file's new contents, after an earlier `get` kept the old ones.
#[test]
fn a_code_location_after_a_scan_of_its_new_contents_reads_nothing() {
    let warm = Warm::start();
    let path = &warm.corpus.code_files[0];
    let line = format!("{path}:2");
    let get = ["get", line.as_str()];
    warm.run(&get);
    let absolute = warm.root().join(path);
    let mut code = fs::read_to_string(&absolute).unwrap();
    code.push_str("// Edited.\n");
    fs::write(&absolute, code).unwrap();
    warm.settled(derived::changed(&[&absolute]));
    let work = warm.run(VALIDATE);
    assert_counts(&work, &[("file_reads", 1), ("code_scans", 1)], "the scan");
    let work = warm.run(&get);
    assert_eq!(work, WorkCounts::default(), "get after the scan");
}

#[test]
fn an_mcp_call_after_the_same_cli_request_does_no_work() {
    let warm = Warm::start();
    warm.run(LIST);
    let run = mcp::run_with(warm.root(), &[("item_list", &json!({}))], &warm.env());
    assert_eq!(
        run.work,
        WorkCounts::default(),
        "item_list through mara mcp"
    );
}

/// The `query report` case of `cli::cases` named `entry`, with its exit status.
fn report(corpus: &Corpus, entry: &str) -> (Vec<String>, i32) {
    let case = (cli::cases(corpus).into_iter())
        .find(|case| case.args[1] == "report" && case.args[2] == entry)
        .expect("a report case");
    (case.args, case.status)
}

impl Warm {
    /// `mara --format json <args>` sent to the daemon: what it printed and the work it reports.
    fn json(&self, args: &[&str], status: i32) -> (String, WorkCounts) {
        let args: Vec<&str> = ["--format", "json"].iter().chain(args).copied().collect();
        let run = cli::run_with(self.root(), &args, &self.env(), status);
        (String::from_utf8(run.output.stdout).unwrap(), run.work)
    }
}

/// The lines in `path` that the proofs of a `query report` response cite: those of its fact
/// leaves, never of the item it reports on.
fn cited_lines(response: &str, path: &str) -> Vec<u64> {
    fn walk(node: &serde_json::Value, path: &str, lines: &mut Vec<u64>) {
        if node["path"] == path
            && let Some(line) = node["line"].as_u64()
        {
            lines.push(line);
        }
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, path, lines);
        }
    }
    let response: serde_json::Value = serde_json::from_str(response).unwrap();
    let mut lines = Vec::new();
    for row in response["rows"].as_array().expect("rows") {
        walk(&row["proof"], path, &mut lines);
    }
    lines
}

/// A report after an edit that moves the lines its proof cites runs again and cites the new
/// lines; once the edit is undone, the session's first run answers it again.
#[test]
fn a_report_after_an_edit_to_its_proof_runs_again() {
    let warm = Warm::start();
    let (args, status) = report(&warm.corpus, "implementation-ready");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let retired = &warm.corpus.retired_requirement;
    let (_, relative) = (warm.corpus.ids.iter())
        .find(|(id, _)| id == retired)
        .expect("the retired requirement's document");
    let (before, _) = warm.json(&args, status);
    let cited = cited_lines(&before, relative);
    assert!(!cited.is_empty(), "a proof citing {relative}: {before}");
    let path = warm.root().join(relative);
    let original = fs::read_to_string(&path).unwrap();
    let (title, rest) = original.split_once('\n').unwrap();
    warm.settled(SystemTime::now());
    // Two lines more above every item of the document.
    fs::write(&path, format!("{title}\n\nInserted.\n{rest}")).unwrap();
    let (moved, work) = warm.json(&args, status);
    assert_counts(&work, &[("program_runs", 1)], "the report after the edit");
    let shifted: Vec<u64> = cited.iter().map(|line| line + 2).collect();
    assert_eq!(cited_lines(&moved, relative), shifted, "{moved}");
    warm.settled(derived::changed(&[&path]));
    fs::write(&path, &original).unwrap();
    let (restored, work) = warm.json(&args, status);
    assert_counts(
        &work,
        &[("program_runs", 0)],
        "the report after the restore",
    );
    assert_eq!(cited_lines(&restored, relative), cited);
    assert_eq!(restored, before);
}

/// A report of another item of the same entry, whose evaluation has the same key, explains
/// other rows and runs; so does one with other inputs. Each is kept beside the others, and a
/// page option, which chooses no part of the outcome, runs nothing.
#[test]
fn a_report_of_other_rows_or_inputs_runs_its_program() {
    let warm = Warm::start();
    let (args, status) = report(&warm.corpus, "implementation-ready");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (retired, _) = warm.json(&args, status);
    let mut other = args.clone();
    *other.last_mut().unwrap() = &warm.corpus.gate_requirement;
    let (accepted, work) = warm.json(&other, 0);
    assert_counts(&work, &[("program_runs", 1)], "the report of another item");
    assert_ne!(accepted, retired);
    for (args, expected) in [(&args, &retired), (&other, &accepted)] {
        let (again, work) = warm.json(args, status);
        assert_counts(
            &work,
            &[("program_runs", 0)],
            &format!("mara {args:?} again"),
        );
        assert_eq!(&again, expected, "mara {args:?} again");
    }
    let mut paged = args.clone();
    paged.extend(["--limit", "1"]);
    let work = warm.run_in(warm.root(), &paged, status);
    assert_counts(
        &work,
        &[("program_runs", 0)],
        "the report with a page option",
    );

    // The `verified` gate with its inputs, then without `--complete`, then for another
    // candidate.
    let gate = self::gate(&warm.corpus, crate::corpus::CANDIDATE);
    let mut gate: Vec<&str> = gate.iter().map(String::as_str).collect();
    gate[1] = "report";
    warm.run_in(warm.root(), &gate, 0);
    let incomplete = &gate[..gate.len() - 2];
    let work = warm.run_in(warm.root(), incomplete, 0);
    assert_counts(
        &work,
        &[("program_runs", 1)],
        "the report without `--complete`",
    );
    let other = self::gate(&warm.corpus, "def");
    let mut other: Vec<&str> = other.iter().map(String::as_str).collect();
    other[1] = "report";
    let work = warm.run_in(warm.root(), &other, 0);
    assert_counts(
        &work,
        &[("program_runs", 1)],
        "the report of another candidate",
    );
}
