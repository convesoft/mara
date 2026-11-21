//! The CLI as a client of the daemon: the cases of VER-DAEMON-START (REQ-FIRST-CALL-IN-PROCESS,
//! DES-DAEMON-LIFECYCLE) through reads, and the result of a request the daemon serves, which is
//! that of direct mode, paths resolved from the client's directory included.

use std::fs::{self, File, TryLockError};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

use crate::grammar::grammar_project;
use crate::{Run, Test, copy_binary, finish, hold_lock, mara, private_directory, signal};

/// A read whose JSON result and stats line the tests compare.
const READ: &[&str] = &["--debug-stats", "--format", "json", "item", "list"];
/// How long a first call may take, both its pipes ended, while the daemon it starts waits for
/// the lock: far below the 10 s that `daemon start` waits, and far above the milliseconds a
/// read of the test's project takes on a loaded machine.
const FIRST_CALL: Duration = Duration::from_secs(3);

/// The run printed what direct mode prints: stdout, stderr with the stats line, exit status.
#[track_caller]
fn assert_direct(run: &Run, direct: &Run) {
    assert!(direct.status.success(), "{direct:?}");
    assert_eq!(run, direct);
}

/// The run printed what direct mode prints but for the counts of its stats line, which a
/// daemon's session reports for its own work.
#[track_caller]
pub(crate) fn assert_direct_but_counts(run: &Run, direct: &Run) {
    let without_stats = |run: &Run| {
        let mut lines: Vec<&str> = run.stderr.lines().collect();
        lines.pop();
        (run.status, run.stdout.clone(), lines.join("\n"))
    };
    assert_eq!(without_stats(run), without_stats(direct), "{run:?}");
}

#[test]
fn the_first_read_runs_in_process_and_starts_a_daemon_that_serves_the_next() {
    let test = Test::new();
    let project = test.project();
    let direct = test.direct(&project, READ);
    // Both pipes end when the call exits ([`finish`]): the daemon it starts holds neither.
    let started = Instant::now();
    let first = test.run_in(&project, READ, &[]);
    assert!(started.elapsed() < FIRST_CALL, "{:?}", started.elapsed());
    assert_direct(&first, &direct);
    let pid = test.wait_for_daemon();
    test.wait_for_session(&project);
    assert_eq!(test.sessions(), std::slice::from_ref(&project));
    // The daemon reports the opening of the session with its first response, a cold request:
    // it had answered nothing before, so the first call ran in-process.
    thread::sleep(Duration::from_secs(2));
    assert_direct(&test.run_in(&project, READ, &[]), &direct);
    // The same request again on unchanged files: only the session could do no work for it.
    thread::sleep(Duration::from_secs(2));
    let served = test.run_in(&project, READ, &[]);
    assert_direct_but_counts(&served, &direct);
    assert!(crate::nothing(&crate::stats(&served)), "{served:?}");
    assert_eq!(test.daemons(), [pid], "another daemon started");
}

#[test]
fn a_read_while_the_lock_is_held_runs_in_process() {
    let test = Test::new();
    let project = test.project();
    let (socket, lock) = test.paths_of(&mara());
    private_directory(socket.parent().unwrap());
    let held = hold_lock(&lock);
    let started = Instant::now();
    let run = test.run_in(&project, READ, &[]);
    let elapsed = started.elapsed();
    // The call does not wait for the daemon it starts, which waits up to 10 s for the lock:
    // it returns, and both its pipes end, well before.
    assert!(
        elapsed < FIRST_CALL,
        "the first call took {elapsed:?} while the lock was held"
    );
    // Returned while the lock is held: no daemon of the test can have answered it.
    let again = File::open(&lock).unwrap();
    assert!(
        matches!(again.try_lock(), Err(TryLockError::WouldBlock)),
        "the lock is no longer held"
    );
    assert_direct(&run, &test.direct(&project, READ));
    assert_eq!(test.status()["running"], false);
    drop(held);
}

#[test]
fn ten_reads_at_once_leave_one_daemon() {
    let test = Test::new();
    let project = test.project();
    let direct = test.direct(&project, READ);
    let children: Vec<_> = (0..10).map(|_| test.spawn_in(&project, READ)).collect();
    // A read that a daemon one of them started served in its session may report less work.
    for child in children {
        assert_direct_but_counts(&finish(child), &direct);
    }
    test.wait_for_daemon();
}

#[test]
fn direct_mode_reaches_and_starts_no_daemon() {
    let test = Test::new();
    let project = test.project();
    let flagged: Vec<&str> = [&["--no-daemon"][..], READ].concat();
    let modes: [crate::Mode; 3] = [
        (READ, &[("CI", "true")]),
        (READ, &[("MARA_NO_DAEMON", "1")]),
        (&flagged, &[]),
    ];
    let direct = test.direct(&project, READ);
    for (args, env) in modes {
        let run = test.run_in(&project, args, env);
        assert_direct(&run, &direct);
        // A client starts the daemon before it serves its request, so that start would run.
        assert!(
            test.daemons().is_empty(),
            "{args:?} {env:?} started a daemon"
        );
    }
    thread::sleep(Duration::from_millis(300));
    assert!(test.daemons().is_empty());

    let (_, pid) = test.start();
    for (args, env) in modes {
        let run = test.in_process(pid, &project, args, env);
        assert_direct(&run, &direct);
    }
    assert_eq!(test.daemons(), [pid]);
}

/// DES-OPERATIONS: a command line means what it means in direct mode when the daemon serves it.
/// The direct-mode flag's text as an option's value is that value, and an option that does not
/// repeat, given twice, is rejected by the CLI before it sends a request, and the daemon refuses
/// a request that repeats it.
#[test]
fn arguments_mean_the_same_through_the_daemon_as_in_direct_mode() {
    use mara::daemon::{PROTOCOL_VERSION, read_frame, write_frame};
    let test = Test::new();
    let project = test.project();
    let pid = test.start_in(&project, &[]);
    test.wait_for_session(&project);

    // Served by the daemon (VER-DAEMON-START): the first call reports the opening of the
    // session; the same request again on unchanged files reports no work, which direct mode
    // never does.
    let value: &[&str] = &[
        "--debug-stats",
        "--format",
        "json",
        "item",
        "list",
        "--id",
        "--no-daemon",
    ];
    let direct = test.direct(&project, value);
    assert!(!crate::nothing(&crate::stats(&direct)), "{direct:?}");
    thread::sleep(Duration::from_secs(2));
    assert_direct_but_counts(&test.run_in(&project, value, &[]), &direct);
    thread::sleep(Duration::from_secs(2));
    let served = test.run_in(&project, value, &[]);
    assert_direct_but_counts(&served, &direct);
    assert!(crate::nothing(&crate::stats(&served)), "{served:?}");

    let rejected: [&[&str]; 2] = [
        &[
            "--format", "json", "item", "list", "--limit", "1", "--limit", "2",
        ],
        &[
            "--format",
            "json",
            "project",
            "validate",
            "--",
            "--no-daemon",
        ],
    ];
    for args in rejected {
        let direct = test.direct(&project, args);
        assert_eq!(direct.status.code(), Some(2), "{direct:?}");
        assert_eq!(direct.json()["error"]["code"], "invalid-argument");
        assert_eq!(test.in_process(pid, &project, args, &[]), direct);
    }

    // The daemon checks a request's arguments again by the same rule, and refuses one that
    // repeats `--limit` without running it.
    let request = json!({
        "version": PROTOCOL_VERSION,
        "build": crate::build_key(),
        "request": "operation",
        "words": ["item", "list"],
        "arguments": [],
        "options": [["limit", "1"], ["limit", "2"]],
        "supplied": [],
        "project": null,
        "cwd": project,
        "umask": 0o022,
        "temp_dir": test.path(),
        "json": true,
    });
    let (socket, _) = test.paths_of(&mara());
    let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
    write_frame(&mut stream, request.to_string().as_bytes()).unwrap();
    let answer = loop {
        let frame = read_frame(&mut stream).unwrap().expect("an answer");
        let frame: Value = serde_json::from_slice(&frame).unwrap();
        if frame["working"] != true {
            break frame;
        }
    };
    assert_eq!(answer["error"], "malformed-request", "{answer}");
    let reason = "`limit`: given twice, but it takes one value";
    assert_eq!(answer["message"], reason);
    let cli = test.direct(&project, rejected[0]).json();
    let message = cli["error"]["message"].as_str().unwrap();
    assert!(
        message.contains(&format!("`--{}", &reason[1..])),
        "{message}"
    );
    assert_eq!(test.daemons(), [pid]);
}

#[test]
fn without_a_runtime_directory_a_read_runs_in_process_and_creates_nothing() {
    let missing = Test::with_base(|dir| dir.join("missing"));
    let long = Test::with_base(|dir| {
        // With `/mara/` and a socket name of 21 bytes, a base of 82 bytes passes 107.
        let base = dir.join("b".repeat(81 - dir.as_os_str().len()));
        private_directory(&base);
        base
    });
    for test in [&missing, &long] {
        let project = test.project();
        let run = test.run_in(&project, READ, &[]);
        assert_direct(&run, &test.direct(&project, READ));
        thread::sleep(Duration::from_millis(300));
        assert!(test.daemons().is_empty());
    }
    assert!(!missing.base.exists());
    assert_eq!(fs::read_dir(&long.base).unwrap().count(), 0);
}

#[test]
fn a_base_of_60_bytes_serves_reads_as_usual() {
    let test = Test::with_base(|dir| {
        let length = dir.as_os_str().len();
        assert!(length < 59, "the temporary directory is too long");
        let base = dir.join("b".repeat(60 - length - 1));
        private_directory(&base);
        base
    });
    assert_eq!(test.base.as_os_str().len(), 60);
    let project = test.project();
    let direct = test.direct(&project, READ);
    assert_direct(&test.run_in(&project, READ, &[]), &direct);
    let pid = test.wait_for_daemon();
    assert_direct(&test.served_by(pid, &project, READ), &direct);
}

#[test]
fn after_a_daemon_is_killed_the_next_read_runs_in_process_and_starts_another() {
    let test = Test::new();
    let project = test.project();
    let direct = test.direct(&project, READ);
    let (_, killed) = test.start();
    let (socket, _) = test.paths_of(&mara());
    signal(killed, libc::SIGKILL);
    test.wait_for_no_daemon(&[]);
    assert!(socket.exists(), "SIGKILL left no socket");
    assert_direct(&test.run_in(&project, READ, &[]), &direct);
    let pid = test.wait_for_daemon();
    assert_ne!(pid, killed);
    assert_direct(&test.served_by(pid, &project, READ), &direct);
}

#[test]
fn each_copy_of_the_binary_starts_its_own_daemon() {
    let test = Test::new();
    let project = test.project();
    let direct = test.direct(&project, READ);
    let (one, two) = (test.path().join("one/mara"), test.path().join("two/mara"));
    let now = SystemTime::now();
    for path in [&one, &two] {
        fs::create_dir(path.parent().unwrap()).unwrap();
        copy_binary(path, now);
    }
    // The first call of a build: no daemon of that build runs, so none served it.
    let first_call = |binary: &Path| {
        let run = crate::finish(crate::spawn(&mut test.command_in(
            &project,
            binary,
            READ,
            &[],
        )));
        assert_direct(&run, &direct);
        wait_for_daemon_of(&test, binary)
    };
    let first = first_call(&one);
    let second = first_call(&two);
    assert_ne!(first, second);
    copy_binary(&two, now + Duration::from_secs(3600));
    let third = first_call(&two);
    assert!(third != first && third != second);
    assert_eq!(test.status_of(&one)["pid"], first);
    let mut daemons = test.daemons();
    daemons.sort_unstable();
    let mut expected = vec![first, second, third];
    expected.sort_unstable();
    assert_eq!(daemons, expected);
}

/// The process ID of the daemon of `binary`'s build, once one answers.
fn wait_for_daemon_of(test: &Test, binary: &Path) -> u32 {
    let deadline = std::time::Instant::now() + crate::WAIT;
    loop {
        if let Some(pid) = test.status_of(binary)["pid"].as_u64() {
            return u32::try_from(pid).unwrap();
        }
        assert!(std::time::Instant::now() < deadline, "no daemon started");
        thread::sleep(Duration::from_millis(20));
    }
}

/// `--project` and `--input <name>=@<file>` are resolved from the client's directory, never
/// from the daemon's, which runs elsewhere; the result is that of direct mode, for `query
/// execute` and for `query report`, whose proof names the input the client's file supplied.
#[test]
fn the_daemon_resolves_paths_from_the_clients_directory() {
    let test = Test::new();
    let project = test.project();
    fs::create_dir(project.join(".mara/rules")).unwrap();
    fs::write(
        project.join(".mara/rules/notes.dl"),
        ".include \"mara.dl\"\n.decl note(text: symbol)\n.decl r_note(text: symbol)\n\
         r_note(t) :- note(t).\n.decl noted(m: Mid, t: symbol)\n\
         noted(m, t) :- item(m, _, _, _, _, _), note(t).\n",
    )
    .unwrap();
    let config = format!(
        "{}\n[rules]\nprograms = [\".mara/rules/*.dl\"]\n\n[queries.notes]\n\
         program = \".mara/rules/notes.dl\"\nrelation = \"r_note\"\n\
         description = \"The notes\"\ninputs = [\"note\"]\n\n[gates.noted]\n\
         program = \".mara/rules/notes.dl\"\ndescription = \"Items without notes\"\n\
         subjects = [\"noted\"]\nconditions = [\"noted\"]\ninputs = [\"note\"]\n",
        crate::CONFIG
    );
    fs::write(project.join(".mara/config.toml"), config).unwrap();
    let client = test.path().join("client");
    fs::create_dir(&client).unwrap();
    fs::write(client.join("note.json"), r#"[["from the client"]]"#).unwrap();
    // The daemon starts elsewhere, and works in `/`.
    let (_, pid) = test.start();
    for (index, args) in [
        &[
            "--debug-stats",
            "--format",
            "json",
            "--project",
            "../project",
            "query",
            "execute",
            "notes",
            "--input",
            "note=@note.json",
        ][..],
        &[
            "--project",
            "../project",
            "query",
            "execute",
            "notes",
            "--input",
            "note=@missing.json",
        ],
        &["--debug-stats", "--project", "../project", "search", "Two"],
        &[
            "--debug-stats",
            "--format",
            "json",
            "--project",
            "../project",
            "query",
            "report",
            "noted",
            "--id",
            "REQ-ONE",
            "--input",
            "note=@note.json",
        ],
        &[
            "--project",
            "../project",
            "query",
            "report",
            "noted",
            "--id",
            "REQ-ONE",
            "--input",
            "note=@note.json",
        ],
        &[
            "--project",
            "../project",
            "query",
            "report",
            "noted",
            "--id",
            "REQ-ONE",
            "--input",
            "note=@missing.json",
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let direct = test.direct(&client, args);
        let served = test.served_by(pid, &client, args);
        // The first request opens the session, a cold request; the others find it warm.
        match index {
            0 => assert_eq!(served, direct, "{args:?}"),
            _ => assert_direct_but_counts(&served, &direct),
        }
    }
    let run = test.direct(
        &client,
        &[
            "--format",
            "json",
            "--project",
            "../project",
            "query",
            "execute",
            "notes",
            "--input",
            "note=@note.json",
        ],
    );
    let rows: Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(rows["rows"][0]["values"][0], "from the client", "{rows}");
    let run = test.direct(
        &client,
        &[
            "--format",
            "json",
            "--project",
            "../project",
            "query",
            "report",
            "noted",
            "--id",
            "REQ-ONE",
            "--input",
            "note=@note.json",
        ],
    );
    let report: Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(report["report"]["outcome"], "failed", "{report}");
    let leaf = &report["rows"][0]["proof"]["children"][1];
    assert_eq!(
        *leaf,
        serde_json::json!({"input": "note", "values": ["from the client"]}),
        "{report}"
    );
}

/// A query whose program uses code links gives, through the daemon, the rows of direct mode,
/// and after a marker moves to another item the rows of the code as it is then
/// (DES-CODE-READERS); so does `query report` of an invariant over code links, whose proof
/// places the code link at its marker.
#[test]
fn a_query_over_code_links_gives_the_rows_of_direct_mode_after_a_marker_changes() {
    let test = Test::new();
    let project = test.project();
    let schema = format!(
        "{}\n[relations.implements]\ninverse = \"implemented_by\"\n\
         description = \"The code realizes the target.\"\n\
         targets = {{ code = [\"requirement\"] }}\n",
        crate::SCHEMA
    );
    fs::write(project.join(".mara/schema.toml"), schema).unwrap();
    fs::create_dir_all(project.join(".mara/rules")).unwrap();
    fs::write(
        project.join(".mara/rules/code.dl"),
        ".include \"mara.dl\"\n.decl linked(dst: Mid, id: Id, path: Path, line: number)\n\
         linked(d, i, p, l) :- code_link(_, d, p, l, _, _), item(d, _, i, _, _, _).\n",
    )
    .unwrap();
    let config = format!(
        "{}\n[rules]\nprograms = [\".mara/rules/*.dl\"]\n\n[queries.linked]\n\
         program = \".mara/rules/code.dl\"\nrelation = \"linked\"\n\
         description = \"The code links\"\n\n[invariants.implemented]\n\
         program = \".mara/rules/code.dl\"\nrelation = \"linked\"\nseverity = \"warning\"\n\
         message = \"Code implements this requirement\"\n\n[code]\nformat_version = 1\n\n\
         [code.languages.rust]\ninclude = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n",
        crate::CONFIG
    );
    fs::write(project.join(".mara/config.toml"), config).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    let code = project.join("src/lib.rs");
    fs::write(&code, "fn one() {}\n// @mara implements REQ-ONE\n").unwrap();
    let args = [
        "--debug-stats",
        "--format",
        "json",
        "query",
        "execute",
        "linked",
    ];
    let (_, pid) = test.start();
    let ids = |run: &Run| -> Vec<(String, u64)> {
        let rows = run.json()["rows"].as_array().unwrap().clone();
        rows.iter()
            .map(|row| {
                let values = &row["values"];
                (
                    values[1].as_str().unwrap().to_owned(),
                    values[3].as_u64().unwrap(),
                )
            })
            .collect()
    };
    let report = [
        "--debug-stats",
        "--format",
        "json",
        "query",
        "report",
        "implemented",
        "--id",
        "REQ-TWO",
    ];
    // The code link leaf of the report's one row, as (path, line).
    let leaf = |run: &Run| {
        let json = run.json();
        let children = json["rows"][0]["proof"]["children"].as_array().cloned();
        let children = children.unwrap_or_else(|| panic!("no proof: {json}"));
        let leaf = children
            .iter()
            .find(|child| child["relation"] == "code_link");
        let leaf = leaf.unwrap_or_else(|| panic!("no code_link leaf: {json}"));
        (leaf["path"].clone(), leaf["line"].clone())
    };
    // The first request opens the session, a cold request; the others find it warm and may
    // report less work for the same output.
    let served = test.served_by(pid, &project, &args);
    assert_direct(&served, &test.direct(&project, &args));
    assert_eq!(ids(&served), [("REQ-ONE".to_owned(), 2)]);
    let served = test.served_by(pid, &project, &report);
    assert_direct_but_counts(&served, &test.direct(&project, &report));
    assert_eq!(served.json()["report"]["outcome"], "passed", "{served:?}");
    fs::write(&code, "fn two() {}\n\n// @mara implements REQ-TWO\n").unwrap();
    let served = test.served_by(pid, &project, &args);
    assert_direct_but_counts(&served, &test.direct(&project, &args));
    assert_eq!(ids(&served), [("REQ-TWO".to_owned(), 3)]);
    let served = test.served_by(pid, &project, &report);
    assert_direct_but_counts(&served, &test.direct(&project, &report));
    assert_eq!(leaf(&served), (json!("src/lib.rs"), json!(3)));
}

/// A read dispatched to a daemon that is killed before it answers fails with `daemon-lost`
/// and reports no work; it is not run again in-process.
#[test]
fn a_read_whose_daemon_is_killed_fails_with_daemon_lost() {
    let test = Test::new();
    let project = test.project();
    let (_, pid) = test.start();
    signal(pid, libc::SIGSTOP);
    let child = test.spawn_in(&project, READ);
    thread::sleep(crate::STOPPED);
    signal(pid, libc::SIGKILL);
    let run = finish(child);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    let error = &run.json()["error"];
    assert_eq!(error["code"], "daemon-lost", "{run:?}");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains(&format!("process {pid}")),
        "{error}"
    );
    let stats: Value = serde_json::from_str(run.stderr.lines().last().unwrap()).unwrap();
    let counts = stats.as_object().unwrap();
    assert!(
        counts
            .iter()
            .all(|(name, value)| name == "format_version" || value == 0),
        "{stats}"
    );
}

/// The operations that read code give through the daemon what they give in direct mode, at the
/// grammar level too (DES-CODE-READERS): the daemon finds the grammar cache from its own
/// environment, which here has the client's `HOME`.
#[test]
fn code_operations_through_the_daemon_match_direct_mode() {
    let test = Test::new();
    let project = test.project();
    let schema = format!(
        "{}\n[relations.implements]\ninverse = \"implemented_by\"\n\
         description = \"The code realizes the target.\"\n\
         targets = {{ code = [\"requirement\"] }}\n",
        crate::SCHEMA
    );
    fs::write(project.join(".mara/schema.toml"), schema).unwrap();
    // The Rust grammar of VER-GRAMMAR-MARKERS in the per-user cache of `home/`, and pinned.
    let home = test.path().join("home");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grammars/rust");
    let wasm = fs::read(fixtures.join("tree-sitter-rust.wasm")).unwrap();
    let tags = fs::read(fixtures.join("tags.scm")).unwrap();
    let checksum = mara::grammar::checksum(&wasm, &tags);
    let cache = home
        .join(".cache/mara/grammars/rust/0.24.2")
        .join(checksum.to_string());
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join(mara::grammar::WASM_FILE), wasm).unwrap();
    fs::write(cache.join(mara::grammar::TAGS_FILE), tags).unwrap();
    let config = format!(
        "{}\n[code]\nformat_version = 1\n\n[code.languages.rust]\ninclude = [\"src/**/*.rs\"]\n\
         comment_prefixes = [\"//\"]\n\
         grammar = {{ version = \"0.24.2\", checksum = \"blake3:{checksum}\" }}\n",
        crate::CONFIG
    );
    fs::write(project.join(".mara/config.toml"), config).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "// @mara implements REQ-ONE\npub fn one() {\n    two(); // @mara implements REQ-TWO\n}\n",
    )
    .unwrap();
    let home = home.to_str().unwrap();
    let (_, pid) = test.start_of(&mara(), &[("HOME", home)]);
    let cases: [&[&str]; 6] = [
        &["--debug-stats", "--format", "json", "get", "src/lib.rs:3"],
        &["--debug-stats", "--format", "json", "get", "src/lib.rs"],
        &[
            "--debug-stats",
            "--format",
            "json",
            "relation",
            "list",
            "REQ-ONE",
        ],
        &[
            "--debug-stats",
            "--format",
            "json",
            "relation",
            "list",
            "src/lib.rs:3",
        ],
        &["--debug-stats", "--format", "json", "project", "validate"],
        &["--debug-stats", "get", "src/lib.rs:2"],
    ];
    for (index, args) in cases.into_iter().enumerate() {
        let env = [("MARA_NO_DAEMON", "1"), ("HOME", home)];
        let direct = test.run_in(&project, args, &env).succeeded();
        let client = test.command_in(&project, &mara(), args, &[("HOME", home)]);
        let served = crate::served_by(pid, client);
        // The first request opens the session, a cold request; the others find it warm.
        match index {
            0 => assert_eq!(served, direct, "{args:?}"),
            _ => assert_direct_but_counts(&served, &direct),
        }
    }
    let got = test.run_in(
        &project,
        &["--format", "json", "get", "src/lib.rs:3"],
        &[("HOME", home)],
    );
    assert_eq!(got.json()["node"]["declaration"], "one", "{got:?}");
}

/// How long the grammar may take on the file that stalls it: the length of the scan.
const STALLED_SCAN: Duration = Duration::from_secs(10);

/// While a request over code links scans a file that keeps the grammar busy, a read of the
/// same project that needs no code is answered: the scan holds none of the session's inputs
/// (DES-DAEMON-SESSIONS, REQ-READS-DURING-VALIDATION). By order: the read returns while the
/// scanning request still runs, long before the grammar's time is up.
fn a_read_is_answered_while_a_code_scan_runs(scanning: &[&str]) {
    let test = Test::new();
    let (project, home) = grammar_project(&test, "project", "rust");
    let budget = STALLED_SCAN.as_millis().to_string();
    let env = [
        ("HOME", home.as_str()),
        ("MARA_TEST_GRAMMAR_STALL", "stall here"),
        ("MARA_TEST_GRAMMAR_BUDGET_MS", budget.as_str()),
    ];
    let pid = test.start_in(&project, &env);
    test.wait_for_session(&project);
    let read = ["--format", "json", "item", "list"];
    let direct = test.direct(&project, &read);
    assert_direct(&test.run_in(&project, &read, &[]), &direct);
    fs::write(project.join("src/stall.rs"), "// stall here\nfn z() {}\n").unwrap();
    let mut scanning = test.spawn_in(&project, scanning);
    // The grammar's thread exists once the scan has begun to use it.
    let deadline = Instant::now() + crate::WAIT;
    while !grammar_thread(pid) {
        assert!(
            Instant::now() < deadline,
            "the scan never loaded the grammar"
        );
        assert!(
            scanning.try_wait().unwrap().is_none(),
            "the scan ended early"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let started = Instant::now();
    let answered = test.run_in(&project, &read, &[]);
    let waited = started.elapsed();
    assert!(
        scanning.try_wait().unwrap().is_none(),
        "the read was answered only after the scan ended ({waited:?})"
    );
    assert!(waited < STALLED_SCAN / 2, "the read waited {waited:?}");
    assert_direct_but_counts(&answered, &direct);
    let scanned = finish(scanning);
    assert!(
        scanned.stderr.contains("unavailable-grammar")
            || scanned.stdout.contains("unavailable-grammar"),
        "{scanned:?}"
    );
}

/// Whether the daemon `pid` runs a grammar's thread.
fn grammar_thread(pid: u32) -> bool {
    let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) else {
        return false;
    };
    tasks.flatten().any(|task| {
        fs::read_to_string(task.path().join("comm"))
            .is_ok_and(|name| name.starts_with("mara-grammar"))
    })
}

#[test]
fn a_read_is_answered_while_a_program_scans_code() {
    a_read_is_answered_while_a_code_scan_runs(&["item", "validate", "REQ-ONE"]);
}

#[test]
fn a_read_is_answered_while_project_validate_scans_code() {
    a_read_is_answered_while_a_code_scan_runs(&["project", "validate"]);
}
