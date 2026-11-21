//! Grammar loads in the daemon, which every request and project it serves shares
//! (DES-GRAMMAR-ASSETS, VER-GRAMMAR-MARKERS).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{Test, finish};

/// A project in `name/` of the test's directory with an invariant over code links and the grammar
/// of VER-GRAMMAR-MARKERS for `language`, `rust` or `python`, pinned and in the per-user cache of
/// `home/` in the test's directory: its root and `HOME`.
pub(crate) fn grammar_project(test: &Test, name: &str, language: &str) -> (PathBuf, String) {
    grammar_project_with(test, name, language, None)
}

/// [`grammar_project`] with `tags` in place of the fixture's `tags.scm`, pinned with them.
fn grammar_project_with(
    test: &Test,
    name: &str,
    language: &str,
    tags: Option<&[u8]>,
) -> (PathBuf, String) {
    let (version, extension, prefix) = match language {
        "rust" => ("0.24.2", "rs", "//"),
        "python" => ("0.25.0", "py", "#"),
        _ => panic!("no fixture grammar for {language}"),
    };
    let project = test.project_in(name);
    let schema = format!(
        "{}\n[relations.implements]\ninverse = \"implemented_by\"\n\
         description = \"The code realizes the target.\"\n\
         targets = {{ code = [\"requirement\"] }}\n",
        crate::SCHEMA
    );
    fs::write(project.join(".mara/schema.toml"), schema).unwrap();
    let home = test.path().join("home");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grammars")
        .join(language);
    let wasm = fs::read(fixtures.join(format!("tree-sitter-{language}.wasm"))).unwrap();
    let tags = tags.map_or_else(
        || fs::read(fixtures.join("tags.scm")).unwrap(),
        <[u8]>::to_vec,
    );
    let checksum = mara::grammar::checksum(&wasm, &tags);
    let cache = home
        .join(".cache/mara/grammars")
        .join(language)
        .join(version)
        .join(checksum.to_string());
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join(mara::grammar::WASM_FILE), wasm).unwrap();
    fs::write(cache.join(mara::grammar::TAGS_FILE), tags).unwrap();
    fs::create_dir_all(project.join(".mara/rules")).unwrap();
    fs::write(
        project.join(".mara/rules/code.dl"),
        ".include \"mara.dl\"\n.decl linked(dst: Mid)\nlinked(d) :- code_link(_, d, _, _, _, _).\n",
    )
    .unwrap();
    let config = format!(
        "{}\n[rules]\nprograms = [\".mara/rules/*.dl\"]\n\n[invariants.implemented]\n\
         program = \".mara/rules/code.dl\"\nrelation = \"linked\"\nseverity = \"warning\"\n\
         message = \"Code implements this requirement\"\n\n[code]\nformat_version = 1\n\n\
         [code.languages.{language}]\ninclude = [\"src/**/*.{extension}\"]\n\
         comment_prefixes = [\"{prefix}\"]\n\
         grammar = {{ version = \"{version}\", checksum = \"blake3:{checksum}\" }}\n",
        crate::CONFIG
    );
    fs::write(project.join(".mara/config.toml"), config).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    let source = match language {
        "rust" => "// @mara implements REQ-ONE\npub fn one() {}\n",
        _ => "# @mara implements REQ-ONE\ndef one():\n    pass\n",
    };
    fs::write(project.join(format!("src/lib.{extension}")), source).unwrap();
    (project, home.to_str().unwrap().to_owned())
}

/// A tags query that names no node of the Rust grammar: its module compiles, and its load fails.
const BROKEN_TAGS: &[u8] = b"(no_such_node) @definition.function\n";

/// The load limit of the test's daemon: far longer than the test takes, so that a load ends
/// only when the test lets it.
const LIMIT: Duration = Duration::from_secs(120);

/// How many times the daemon recorded `event` in the directory `events`
/// (`MARA_TEST_GRAMMAR_EVENTS`).
fn count(events: &Path, event: &str) -> usize {
    let recorded = fs::read_to_string(events.join("events")).unwrap_or_default();
    recorded.lines().filter(|line| *line == event).count()
}

/// Waits until the daemon recorded `event` in `events`, while none of `running` has ended.
fn wait_for(events: &Path, event: &str, running: &mut [&mut Child]) {
    let deadline = Instant::now() + crate::WAIT;
    while count(events, event) == 0 {
        assert!(Instant::now() < deadline, "no event {event:?}");
        for child in running.iter_mut() {
            assert!(child.try_wait().unwrap().is_none(), "a request ended early");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// The messages of the `unavailable-grammar` diagnostics of `result`.
fn unavailable(result: &Value) -> Vec<String> {
    let diagnostics = result["diagnostics"].as_array().into_iter().flatten();
    (diagnostics.filter(|d| d["code"] == "unavailable-grammar"))
        .map(|d| d["message"].as_str().unwrap().to_owned())
        .collect()
}

/// While a request loads a grammar, the daemon serves a request of another project that needs
/// another grammar; a request of a third project for the same grammar waits for that load and
/// shares its failure; a later request tries the load again. Ordered by events, not by time:
/// the load of the Rust grammar stays held until the test lets it go.
#[test]
fn a_grammar_load_holds_up_only_requests_for_its_grammar() {
    let test = Test::new();
    let (first, home) = grammar_project_with(&test, "first", "rust", Some(BROKEN_TAGS));
    let (second, _) = grammar_project_with(&test, "second", "rust", Some(BROKEN_TAGS));
    let (other, _) = grammar_project(&test, "other", "python");
    let events = test.path().join("events");
    fs::create_dir(&events).unwrap();
    fs::write(events.join("hold"), "").unwrap();
    let limit = LIMIT.as_millis().to_string();
    let env = [
        ("HOME", home.as_str()),
        ("MARA_TEST_GRAMMAR_LOAD_STALL", "rust"),
        ("MARA_TEST_GRAMMAR_EVENTS", events.to_str().unwrap()),
        ("MARA_TEST_GRAMMAR_LOAD_MS", limit.as_str()),
    ];
    test.start_in(&first, &env);
    test.wait_for_session(&first);
    let validate = ["--format", "json", "project", "validate"];
    let mut loading = test.spawn_in(&first, &validate);
    wait_for(&events, "load rust", &mut [&mut loading]);
    let mut waiting = test.spawn_in(&second, &validate);
    wait_for(&events, "wait rust", &mut [&mut loading, &mut waiting]);

    // Another grammar loads and serves its request while the Rust load is held.
    let got = test.run_in(&other, &["--format", "json", "get", "src/lib.py:2"], &[]);
    let got = got.json();
    assert_eq!(got["node"]["declaration"], "one", "{got}");
    assert_eq!(unavailable(&got), Vec::<String>::new(), "{got}");
    assert!(loading.try_wait().unwrap().is_none());
    assert!(waiting.try_wait().unwrap().is_none());

    // Let go, the load fails for its query: both requests report that failure, from one load.
    fs::remove_file(events.join("hold")).unwrap();
    for run in [finish(loading), finish(waiting)] {
        let found = unavailable(&run.json());
        assert_eq!(found.len(), 1, "{run:?}");
        assert!(found[0].contains("tags.scm is no query"), "{}", found[0]);
    }
    assert_eq!(count(&events, "load rust"), 1);

    // A later request checks the files again and loads again: they might have been repaired.
    let again = test.run_in(&first, &validate, &[]);
    let found = unavailable(&again.json());
    assert_eq!(found.len(), 1, "{again:?}");
    assert!(found[0].contains("tags.scm is no query"), "{}", found[0]);
    assert_eq!(count(&events, "load rust"), 2);
}

/// `served` of `mara daemon status`: the operation requests that the test's daemon answered.
fn served(test: &Test) -> u64 {
    let status = test.status();
    status["served"]
        .as_u64()
        .unwrap_or_else(|| panic!("no daemon: {status}"))
}

/// A daemon loads a pinned grammar once for every request that needs it, and again only when
/// the cached files' stats change (DES-GRAMMAR-ASSETS): the events `MARA_TEST_GRAMMAR_EVENTS`
/// records count the loads, and each request is one that daemon served.
#[test]
fn the_daemon_loads_a_grammar_again_only_when_its_files_change() {
    let test = Test::new();
    let (project, home) = grammar_project(&test, "project", "rust");
    let events = test.path().join("events");
    fs::create_dir(&events).unwrap();
    let env = [
        ("HOME", home.as_str()),
        ("MARA_TEST_GRAMMAR_EVENTS", events.to_str().unwrap()),
    ];
    test.start_in(&project, &env);
    test.wait_for_session(&project);
    let requests: [&[&str]; 2] = [
        &["--format", "json", "get", "src/lib.rs:2"],
        &["--format", "json", "relation", "list", "REQ-ONE"],
    ];
    // Each request through the daemon: answered by it, at the grammar level.
    let send = |args: &[&str]| {
        let before = served(&test);
        let run = test.run_in(&project, args, &[]).succeeded();
        assert_eq!(
            served(&test),
            before + 1,
            "{args:?}: the daemon did not serve it"
        );
        assert_eq!(unavailable(&run.json()), Vec::<String>::new(), "{run:?}");
        run.json()
    };
    let got = send(requests[0]);
    assert_eq!(got["node"]["declaration"], "one", "{got}");
    send(requests[1]);
    send(requests[0]);
    assert_eq!(count(&events, "load rust"), 1, "loads of unchanged files");

    // The cached `.wasm` rewritten with the same bytes and another mtime: its stats change.
    let cache = Path::new(&home).join(".cache/mara/grammars/rust/0.24.2");
    let directory = fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let wasm = directory.join(mara::grammar::WASM_FILE);
    let file = fs::File::options().write(true).open(&wasm).unwrap();
    let modified = file.metadata().unwrap().modified().unwrap();
    file.set_modified(modified - Duration::from_secs(60))
        .unwrap();
    drop(file);
    send(requests[1]);
    assert_eq!(count(&events, "load rust"), 2, "the load after the change");
    send(requests[0]);
    assert_eq!(count(&events, "load rust"), 2, "loads after the change");
}
