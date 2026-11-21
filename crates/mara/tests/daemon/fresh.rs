//! VER-NEVER-STALE (REQ-NEVER-STALE, DES-DAEMON-SESSIONS): a request that the daemon serves in
//! its project's session returns what CLI direct mode returns on the files as they are. Each
//! change is made by the test's own file operations; the requests are sent at once, without
//! waiting for the watcher, and again 2 s later, after the watcher re-derived.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use crate::mcp::Server;
use crate::{Run, Test, WAIT, finish, settle};

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                      format_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n\n[rules]\n\
                      programs = [\".mara/rules/*.dl\"]\ntime_limit_seconds = 5\n\n\
                      [invariants.flagged]\nprogram = \".mara/rules/flag.dl\"\n\
                      relation = \"r_flagged\"\nseverity = \"error\"\nmessage = \"flagged\"\n\n\
                      [queries.flagged-items]\nprogram = \".mara/rules/flag.dl\"\n\
                      relation = \"r_flagged\"\ndescription = \"Flagged items\"\n\n\
                      [queries.notes]\nprogram = \".mara/rules/notes.dl\"\n\
                      relation = \"r_note\"\ndescription = \"Notes\"\ninputs = [\"note\"]\n\n\
                      [code]\nformat_version = 1\n\n[code.languages.rust]\n\
                      include = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\n\
                      id_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n\
                      [relations.implements]\ninverse = \"implemented_by\"\n\
                      description = \"Code realizes the target.\"\n\
                      targets = { code = [\"requirement\"] }\n";

/// Flags the item `REQ-ONE`.
const FLAG_ONE: &str = ".include \"mara.dl\"\n.decl r_flagged(m: Mid)\nr_flagged(m) :- item(m, _, \"REQ-ONE\", _, _, _).\n";

const NOTES: &str = ".include \"mara.dl\"\n.decl note(text: symbol)\n.decl r_note(text: symbol)\n\
                     r_note(t) :- note(t).\n";

/// An item, its MID ending in `n`.
pub(crate) fn item(n: u32, id: &str, title: &str) -> String {
    format!(
        "<a id=\"01M7JQ{n:020}\"></a>\n\n## {title}\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | {id} |\n| Status | Draft |\n\nText of {title}.\n"
    )
}

/// A document of `items`.
fn document(items: &[(u32, &str, &str)]) -> String {
    let items: Vec<String> = (items.iter())
        .map(|&(n, id, title)| item(n, id, title))
        .collect();
    items.join("\n---\n\n")
}

/// A project in `name/` of the test's directory with programs, a query with an input, code
/// and two documents, its files settled.
pub(crate) fn project(test: &Test, name: &str) -> PathBuf {
    let root = test.path().join(name);
    for directory in [".mara/rules", "docs", "src"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    let files = [
        (".mara/config.toml", CONFIG.to_owned()),
        (".mara/schema.toml", SCHEMA.to_owned()),
        (".mara/rules/flag.dl", FLAG_ONE.to_owned()),
        (".mara/rules/notes.dl", NOTES.to_owned()),
        (
            "docs/reqs.mara.md",
            document(&[(1, "REQ-ONE", "One apple"), (2, "REQ-TWO", "Two pear")]),
        ),
        (
            "docs/more.mara.md",
            document(&[(3, "REQ-THREE", "Three plum")]),
        ),
        (
            "src/lib.rs",
            "// @mara implements REQ-TWO\npub fn two() {}\n".to_owned(),
        ),
        ("note.json", "[[\"first\"]]".to_owned()),
    ];
    for (path, contents) in files {
        fs::write(root.join(path), contents).unwrap();
    }
    settle();
    fs::canonicalize(root).unwrap()
}

/// The reads of the first cases.
const READS: &[&[&str]] = &[
    &["--format", "json", "item", "list"],
    &["--format", "json", "search", "apple"],
    &["--format", "json", "search", "fig"],
    &["--format", "json", "project", "validate"],
];

/// Sends `args` to the daemon `pid` in `directory`, then runs it in direct mode: both print the
/// same and exit the same.
#[track_caller]
fn same(test: &Test, pid: u32, directory: &Path, args: &[&str]) -> Run {
    let served = test.run_in(directory, args, &[]);
    let direct = test.direct(directory, args);
    assert_eq!(served, direct, "{args:?}");
    assert_eq!(test.daemons(), [pid], "the daemon ended");
    served
}

/// Makes `change`, then sends `reads` at once and again 2 s later.
#[track_caller]
fn after(test: &Test, pid: u32, root: &Path, reads: &[&[&str]], change: impl FnOnce()) {
    change();
    for args in reads {
        same(test, pid, root, args);
    }
    thread::sleep(Duration::from_secs(2));
    for args in reads {
        same(test, pid, root, args);
    }
}

/// Writes `contents` to a new file beside `path`, then renames it over `path`, as editors save.
fn save_by_rename(path: &Path, contents: &str) {
    let staged = path.with_extension("swp");
    fs::write(&staged, contents).unwrap();
    fs::rename(staged, path).unwrap();
}

/// A started daemon serving the project at `root`, whose session is open and settled.
pub(crate) fn daemon(test: &Test, root: &Path) -> u32 {
    let pid = test.start_in(root, &[]);
    test.wait_for_session(root);
    for args in READS {
        same(test, pid, root, args);
    }
    pid
}

#[test]
fn every_change_of_a_document_is_seen_by_the_next_request() {
    let test = Test::new();
    let root = project(&test, "project");
    let pid = daemon(&test, &root);
    let reqs = root.join("docs/reqs.mara.md");
    // In place, keeping the size: only the contents tell the edit.
    after(&test, pid, &root, READS, || {
        let text = fs::read_to_string(&reqs).unwrap();
        fs::write(&reqs, text.replace("apple", "fig!!")).unwrap();
    });
    after(&test, pid, &root, READS, || {
        let text = fs::read_to_string(&reqs).unwrap();
        save_by_rename(&reqs, &text.replace("fig!!", "apple"));
    });
    after(&test, pid, &root, READS, || {
        let added = document(&[(4, "REQ-FOUR", "Four fig")]);
        fs::write(root.join("docs/added.mara.md"), added).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::create_dir(root.join("docs/new")).unwrap();
        let added = document(&[(5, "REQ-FIVE", "Five apple")]);
        fs::write(root.join("docs/new/five.mara.md"), added).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::remove_file(root.join("docs/added.mara.md")).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::rename(
            root.join("docs/more.mara.md"),
            root.join("docs/new/more.mara.md"),
        )
        .unwrap();
    });
}

#[test]
fn a_changed_schema_configuration_or_gitignore_is_seen_by_the_next_request() {
    let test = Test::new();
    let root = project(&test, "project");
    let pid = daemon(&test, &root);
    // Every item's status becomes one the schema does not declare.
    after(&test, pid, &root, READS, || {
        let schema = SCHEMA.replace("[\"draft\"]", "[\"accepted\"]");
        fs::write(root.join(".mara/schema.toml"), schema).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::write(root.join(".mara/schema.toml"), SCHEMA).unwrap();
    });
    after(&test, pid, &root, READS, || {
        let config = CONFIG.replace("docs/**/*.mara.md", "docs/reqs.mara.md");
        fs::write(root.join(".mara/config.toml"), config).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::write(root.join(".mara/config.toml"), CONFIG).unwrap();
    });
    after(&test, pid, &root, READS, || {
        fs::write(root.join("docs/.gitignore"), "reqs.mara.md\n").unwrap();
    });
}

/// A branch checkout: the documents' directory replaced by another at once, with a document
/// changed, one added and one removed.
#[test]
fn a_replaced_documents_directory_is_seen_by_the_next_request() {
    let test = Test::new();
    let root = project(&test, "project");
    let pid = daemon(&test, &root);
    after(&test, pid, &root, READS, || {
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        let changed = document(&[(1, "REQ-ONE", "One fig"), (2, "REQ-TWO", "Two pear")]);
        fs::write(other.join("reqs.mara.md"), changed).unwrap();
        let added = document(&[(6, "REQ-SIX", "Six apple")]);
        fs::write(other.join("six.mara.md"), added).unwrap();
        fs::rename(root.join("docs"), root.join("old")).unwrap();
        fs::rename(&other, root.join("docs")).unwrap();
    });
}

/// A program file, a file a request supplies as an input, and a code file, each edited.
#[test]
fn an_edited_program_input_or_code_file_is_seen_by_the_next_request() {
    let test = Test::new();
    let root = project(&test, "project");
    let pid = daemon(&test, &root);
    let validate: &[&str] = &["--format", "json", "project", "validate"];
    let query: &[&str] = &["--format", "json", "query", "execute", "flagged-items"];
    after(&test, pid, &root, &[validate, query], || {
        let program = FLAG_ONE.replace("REQ-ONE", "REQ-TWO");
        fs::write(root.join(".mara/rules/flag.dl"), program).unwrap();
    });
    let notes: &[&str] = &[
        "--format",
        "json",
        "query",
        "execute",
        "notes",
        "--input",
        "note=@note.json",
    ];
    after(&test, pid, &root, &[notes], || {
        fs::write(root.join("note.json"), "[[\"second\"]]").unwrap();
    });
    after(&test, pid, &root, &[validate], || {
        let code = "// @mara implements REQ-NONE\npub fn two() {}\n";
        fs::write(root.join("src/lib.rs"), code).unwrap();
    });
}

/// The long program of VER-EVALUATION-TIME-LIMIT behind the invariant's relation.
pub(crate) const LONG: &str = ".include \"mara.dl\"\n.decl n(x: number)\nn(x) :- x = range(0, 1000).\n\
                    .decl bad(m: Mid, c: number)\n\
                    bad(m, c) :- item(m, _, _, _, _, _), \
                    c = count : { n(w), n(x), n(y), n(z), w + x + y + z < 0 }.\n\
                    .decl r_flagged(m: Mid)\nr_flagged(m) :- bad(m, _).\n";

/// A fast program with the relations of [`LONG`].
const FAST: &str = ".include \"mara.dl\"\n.decl n(x: number)\n.decl bad(m: Mid, c: number)\n\
                    bad(m, 0) :- item(m, _, \"REQ-ONE\", _, _, _).\n\
                    .decl r_flagged(m: Mid)\nr_flagged(m) :- bad(m, _).\n";

/// The process ID of the `souffle` process whose working directory lies in `temporary`, once
/// one runs.
pub(crate) fn souffle_in(temporary: &Path) -> u32 {
    let deadline = Instant::now() + WAIT;
    loop {
        for process in fs::read_dir("/proc").unwrap().flatten() {
            let Some(pid) = process.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            let (Ok(exe), Ok(cwd)) = (
                fs::read_link(process.path().join("exe")),
                fs::read_link(process.path().join("cwd")),
            ) else {
                continue;
            };
            if exe.file_name().is_some_and(|name| name == "souffle") && cwd.starts_with(temporary) {
                return pid;
            }
        }
        assert!(Instant::now() < deadline, "no souffle ran in {temporary:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Whether the process `pid` runs.
pub(crate) fn runs(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
fn a_validation_answers_for_the_files_it_read_and_the_next_for_the_files_after() {
    let test = Test::new();
    let root = project(&test, "project");
    let program = root.join(".mara/rules/flag.dl");
    fs::write(&program, LONG).unwrap();
    let validate: &[&str] = &["--format", "json", "project", "validate"];
    let before = test.direct(&root, validate);
    assert!(before.stdout.contains("time limit of 5 s"), "{before:?}");
    let pid = test.start_in(&root, &[]);
    test.wait_for_session(&root);
    let v1: Child = test.spawn_in(&root, validate);
    let souffle = souffle_in(test.path());
    fs::write(&program, FAST).unwrap();
    let v2 = test.run_in(&root, validate, &[]);
    assert!(runs(souffle), "V1's souffle ended before V2 answered");
    let after = test.direct(&root, validate);
    assert_eq!(v2, after);
    assert!(!after.stdout.contains("time limit"), "{after:?}");
    let v1 = finish(v1);
    assert_eq!(v1, before);
    let v3 = test.run_in(&root, validate, &[]);
    assert_eq!(v3, after);
    assert_eq!(test.daemons(), [pid]);
}

/// The idle case of VER-NEVER-STALE, with `MARA_TEST_IDLE_SECONDS` (ADR-TEST-IDLE-PERIOD).
#[test]
fn idle_sessions_are_dropped_and_the_daemon_exits_with_the_last() {
    let test = Test::new();
    let a = project(&test, "a");
    let b = project(&test, "b");
    let read: &[&str] = &["--debug-stats", "--format", "json", "item", "list"];
    let pid = test.start_in(&a, &[("MARA_TEST_IDLE_SECONDS", "4")]);
    test.wait_for_session(&a);
    let cold_b = test.direct(&b, read);
    test.run_in(&b, read, &[]).succeeded();
    assert_eq!(test.sessions(), [a.clone(), b.clone()]);
    let mut server = Server::start(&test, &a, &[], &[]);
    let server_process = server.child.id();
    let tool_result = server.call_and_wait();
    for _ in 0..8 {
        crate::client::assert_direct_but_counts(
            &test.run_in(&a, read, &[]),
            &test.direct(&a, read),
        );
        thread::sleep(Duration::from_secs(1));
    }
    assert_eq!(
        test.sessions(),
        std::slice::from_ref(&a),
        "B's session outlived its idle period"
    );
    // B's session opens again: a cold request.
    let reopened = test.run_in(&b, read, &[]);
    assert_eq!(reopened, cold_b);
    let last = Instant::now();
    let deadline = last + Duration::from_secs(10);
    while test.daemons().contains(&pid) {
        assert!(
            Instant::now() < deadline,
            "the daemon outlived its sessions"
        );
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(test.daemons(), [server_process]);
    // No daemon runs: the server serves its next call in-process, and starts one.
    assert_eq!(server.call_and_wait(), tool_result);
    let mut others = test.daemons();
    others.retain(|process| *process != server_process);
    assert!(others.iter().all(|process| *process != pid), "{others:?}");
    server.finish();
}

/// Two copies of one project, as two Git worktrees are: the same relative paths, other
/// contents, two sessions.
#[test]
fn two_copies_of_a_project_are_answered_each_from_its_own_files() {
    let test = Test::new();
    let one = project(&test, "one");
    let two = project(&test, "two");
    fs::write(
        two.join("docs/reqs.mara.md"),
        document(&[(1, "REQ-ONE", "One fig"), (2, "REQ-TWO", "Two pear")]),
    )
    .unwrap();
    let pid = test.start_in(test.path(), &[]);
    for _ in 0..3 {
        for root in [&one, &two] {
            for args in READS {
                same(&test, pid, root, args);
            }
        }
    }
    assert_eq!(test.sessions(), [one, two]);
}

/// A program over code links (DES-CODE-READERS): the code files it reads are inputs of the
/// session like any other.
const LINKED: &str = ".include \"mara.dl\"\n.decl linked(dst: Mid, id: Id, path: Path, line: number)\n\
                      linked(d, i, p, l) :- code_link(_, d, p, l, _, _), item(d, _, i, _, _, _).\n";

/// A changed marker, an added and a removed code file, a changed `[code]` table, and a code
/// file edited twice within one second are each seen by the next query, report and item
/// validation over code links. An edit that keeps the file's whole stat, within the timestamp
/// tick of the check that read it, is found by the 2 s rule of DES-INPUTS-AND-KEYS, which
/// `tracked::tests` checks with an injected clock: code files go through the same check.
#[test]
fn every_change_of_code_is_seen_by_the_next_request_over_code_links() {
    let test = Test::new();
    let root = project(&test, "project");
    fs::write(root.join(".mara/rules/linked.dl"), LINKED).unwrap();
    let entries = "\n[queries.linked]\nprogram = \".mara/rules/linked.dl\"\nrelation = \"linked\"\n\
                   description = \"The code links\"\n\n[invariants.implemented]\n\
                   program = \".mara/rules/linked.dl\"\nrelation = \"linked\"\n\
                   severity = \"warning\"\nmessage = \"Code implements this\"\n";
    let config = format!("{CONFIG}{entries}");
    fs::write(root.join(".mara/config.toml"), &config).unwrap();
    settle();
    let reads: &[&[&str]] = &[
        &["--format", "json", "query", "execute", "linked"],
        &[
            "--format",
            "json",
            "query",
            "report",
            "implemented",
            "--id",
            "REQ-TWO",
        ],
        &["--format", "json", "item", "validate", "REQ-ONE"],
    ];
    let pid = test.start_in(&root, &[]);
    test.wait_for_session(&root);
    for args in reads {
        same(&test, pid, &root, args);
    }
    let lib = root.join("src/lib.rs");
    after(&test, pid, &root, reads, || {
        fs::write(&lib, "// @mara implements REQ-ONE\npub fn two() {}\n").unwrap();
    });
    after(&test, pid, &root, reads, || {
        let more = "fn more() {}\n// @mara implements REQ-TWO\n";
        fs::write(root.join("src/more.rs"), more).unwrap();
    });
    after(&test, pid, &root, reads, || {
        fs::remove_file(root.join("src/more.rs")).unwrap();
    });
    after(&test, pid, &root, reads, || {
        let other = config.replace("comment_prefixes = [\"//\"]", "comment_prefixes = [\"#\"]");
        fs::write(root.join(".mara/config.toml"), other).unwrap();
    });
    after(&test, pid, &root, reads, || {
        fs::write(root.join(".mara/config.toml"), &config).unwrap();
    });
    // Twice within one second, the same size: the request between them read the first.
    fs::write(&lib, "// @mara implements REQ-ONE\npub fn one() {}\n").unwrap();
    for args in reads {
        same(&test, pid, &root, args);
    }
    after(&test, pid, &root, reads, || {
        fs::write(&lib, "// @mara implements REQ-TWO\npub fn one() {}\n").unwrap();
    });
    // Requests for a program without code links read no code. A code file changed after one,
    // then a program over code links: the code is first read after it changed.
    let plain: &[&str] = &["--format", "json", "query", "execute", "flagged-items"];
    let linked: &[&str] = &["--format", "json", "query", "execute", "linked"];
    same(&test, pid, &root, plain);
    fs::write(&lib, "// @mara implements REQ-THREE\npub fn one() {}\n").unwrap();
    same(&test, pid, &root, linked);
    // Code read, then changed while only programs without code links are requested, then the
    // program over code links again.
    fs::write(&lib, "// @mara implements REQ-ONE\npub fn one() {}\n").unwrap();
    same(&test, pid, &root, plain);
    same(&test, pid, &root, &["--format", "json", "item", "list"]);
    same(&test, pid, &root, linked);
    // Nothing changed since a settled check: a warm request reads, hashes, parses, builds and
    // scans nothing (DES-PERFORMANCE-HARNESS).
    settle();
    for args in reads {
        let counted: Vec<&str> = ["--debug-stats"].iter().chain(*args).copied().collect();
        test.run_in(&root, &counted, &[]);
        let warm = test.run_in(&root, &counted, &[]);
        let stats = crate::stats(&warm);
        for counter in [
            "file_reads",
            "hashes",
            "parses",
            "snapshot_builds",
            "code_scans",
        ] {
            assert_eq!(stats[counter], 0, "{counter} of {args:?}: {stats}");
        }
    }
}
