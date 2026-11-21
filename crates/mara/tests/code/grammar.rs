//! VER-GRAMMAR-MARKERS: markers owned by declarations with the fixture grammars of Rust and
//! Python installed in a per-user cache under the test's own `HOME`, and the fallback to the
//! basic level of a grammar that cannot be used.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::{Project, documents, unchanged};

/// A Rust file with markers before declarations, inside and after code, in each kind of comment,
/// and marker-like lines in strings.
const RUST: &str = r##"//! Signing in.
// @mara implements REQ-AUDIT

use std::fmt;

// @mara implements REQ-LOGIN
/// Signs a user in.
#[test]
pub fn login() {
    check(); // @mara implements REQ-AUDIT
    // @mara implements REQ-LOGIN-AUDIT
    let raw = r#"
// @mara implements REQ-LOGIN
"#;
    let s = " // @mara implements REQ-LOGIN";
}

/// A session.
// @mara implements REQ-LOGIN
#[derive(Debug)]
struct Session;

struct S; // @mara implements REQ-AUDIT

impl Session {
    /// Renews it.
    /// @mara implements REQ-LOGIN
    fn renew(&self) {}
}

/*
 * @mara implements REQ-AUDIT
 */
fn blocked() {}
"##;

/// A Python script with markers before a function and a class, and inside a method.
const PYTHON: &str = "#!/usr/bin/env python3\n# @mara implements REQ-LOGIN\ndef login():\n    \
                      pass\n\n\n# @mara implements REQ-AUDIT\nclass Audit:\n    \
                      def record(self):\n        # @mara implements REQ-LOGIN-AUDIT\n        \
                      return 1\n";

/// A project with [`RUST`] and [`PYTHON`] and both grammars installed.
fn with_grammars() -> Project {
    let mut project = Project::new();
    project.write("src/login.rs", RUST);
    project.write("scripts/tool.py", PYTHON);
    project.install_grammar("rust");
    project.install_grammar("python");
    project
}

/// The code links of `node` that `relation` gives, as (location's path, line and declaration,
/// source line).
fn links(project: &Project, node: &str, relation: &str) -> Vec<(String, u64, Value, u64)> {
    let args = [
        "relation",
        "list",
        node,
        "--relation",
        relation,
        "--limit",
        "100",
    ];
    let result = project.json(&args, 0);
    let connections = result["connections"].as_array().unwrap();
    connections
        .iter()
        .map(|connection| {
            let code = match connection["direction"].as_str().unwrap() {
                "incoming" => &connection["neighbour"],
                _ => &result["node"],
            };
            assert_eq!(code["kind"], "code", "{connection}");
            assert_eq!(connection["source"]["path"], code["path"], "{connection}");
            (
                code["path"].as_str().unwrap().to_owned(),
                code["line"].as_u64().unwrap(),
                code["declaration"].clone(),
                connection["source"]["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn at(path: &str, line: u64, declaration: Option<&str>, source: u64) -> (String, u64, Value, u64) {
    (path.to_owned(), line, json!(declaration), source)
}

#[test]
fn owns_markers_by_declaration() {
    let project = with_grammars();
    let before = documents(&project.tree());
    assert_eq!(project.problems(), []);
    let rust = "src/login.rs";
    let python = "scripts/tool.py";
    assert_eq!(
        links(&project, "REQ-LOGIN", "implemented_by"),
        [
            at(python, 3, Some("login"), 2),
            at(rust, 9, Some("login"), 6),
            at(rust, 21, Some("Session"), 19),
            at(rust, 28, Some("renew"), 27),
        ]
    );
    assert_eq!(
        links(&project, "REQ-AUDIT", "implemented_by"),
        [
            at(python, 8, Some("Audit"), 7),
            at(rust, 1, None, 2),
            at(rust, 9, Some("login"), 10),
            at(rust, 23, Some("S"), 23),
            at(rust, 34, Some("blocked"), 32),
        ]
    );
    assert_eq!(
        links(&project, "REQ-LOGIN-AUDIT", "implemented_by"),
        [
            at(python, 9, Some("record"), 10),
            at(rust, 9, Some("login"), 11)
        ]
    );
    unchanged(&before, &documents(&project.tree()), "reading code");
}

#[test]
fn strings_hold_markers_only_at_the_basic_level() {
    let project = with_grammars();
    assert_eq!(project.problems(), []);
    let raw = links(&project, "src/login.rs:13", "implements");
    assert!(raw.iter().all(|(.., source)| *source != 13), "{raw:?}");

    // Without the Rust grammar: the raw string's line links, and the string literal is
    // `invalid-marker`, as VER-CODE-MARKERS has it.
    let config = project.read(".mara/config.toml");
    let pin = config.lines().find(|line| line.contains("0.24.2")).unwrap();
    project.write(".mara/config.toml", config.replace(&format!("{pin}\n"), ""));
    let at = |code: &str, line: u64| (code.to_owned(), "src/login.rs".to_owned(), line);
    assert_eq!(project.problems(), [at("invalid-marker", 15)]);
    let basic = links(&project, "REQ-LOGIN", "implemented_by");
    assert!(
        basic.contains(&("src/login.rs".to_owned(), 13, Value::Null, 13)),
        "{basic:?}"
    );
}

#[test]
fn names_the_declaration_that_holds_a_line() {
    let project = with_grammars();
    let before = documents(&project.tree());
    // A line inside a function names that function, and its markers.
    let inside = project.json(&["relation", "list", "src/login.rs:13"], 0);
    let login = json!({"kind": "code", "path": "src/login.rs", "line": 9, "declaration": "login"});
    assert_eq!(inside["node"], login);
    let sources: Vec<(u64, &str)> = inside["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            assert_eq!(c["direction"], "outgoing");
            let id = c["neighbour"]["id"].as_str().unwrap();
            (c["source"]["line"].as_u64().unwrap(), id)
        })
        .collect();
    assert_eq!(
        sources,
        [(6, "REQ-LOGIN"), (10, "REQ-AUDIT"), (11, "REQ-LOGIN-AUDIT")]
    );
    // A marker's line names the declaration that owns it, and a line outside every
    // declaration the file.
    let marker = project.json(&["relation", "list", "src/login.rs:19"], 0);
    assert_eq!(marker["node"]["declaration"], "Session");
    assert_eq!(marker["connections"].as_array().unwrap().len(), 1);
    let file = project.json(&["relation", "list", "src/login.rs:4"], 0);
    assert_eq!(
        file["node"],
        json!({"kind": "code", "path": "src/login.rs", "line": 1, "declaration": null})
    );
    let owned: Vec<u64> = file["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["source"]["line"].as_u64().unwrap())
        .collect();
    assert_eq!(owned, [2]);

    // `get` of a line inside a function: that function, from its first line to its last.
    let got = project.json(&["get", "src/login.rs:12"], 0);
    assert_eq!(got["node"], login);
    let lines: Vec<&str> = RUST.split_inclusive('\n').collect();
    assert_eq!(got["content"], lines[8..16].concat());
    let method = project.json(&["get", "src/login.rs:27"], 0);
    assert_eq!(method["node"]["declaration"], "renew");
    assert_eq!(method["content"], "    fn renew(&self) {}\n");
    let python = project.json(&["get", "scripts/tool.py:11"], 0);
    assert_eq!(python["node"]["declaration"], "record");
    assert_eq!(
        python["content"],
        "    def record(self):\n        # @mara implements REQ-LOGIN-AUDIT\n        return 1\n"
    );
    let whole = project.json(&["get", "src/login.rs"], 0);
    assert_eq!(whole["content"], RUST);
    let human = String::from_utf8(project.run(&["get", "src/login.rs:12"], 0).stdout).unwrap();
    assert!(
        human.starts_with("src/login.rs:9: login (code)\n"),
        "{human}"
    );

    // Through `mara mcp`: the JSON results of the CLI.
    let mut mcp = project.mcp();
    for node in [
        "REQ-LOGIN",
        "REQ-AUDIT",
        "src/login.rs",
        "src/login.rs:13",
        "src/login.rs:4",
        "scripts/tool.py:10",
    ] {
        let cli = project.json(&["relation", "list", node], 0);
        let called = mcp.call("relation_list", json!({"node": node}));
        assert_eq!(called, cli, "{node}");
        let cli = project.json(&["get", node], 0);
        assert_eq!(mcp.call("get", json!({"node": node})), cli, "{node}");
    }
    let validated = mcp.call("project_validate", json!({}));
    assert_eq!(validated, project.json(&["project", "validate"], 0));
    drop(mcp);
    unchanged(&before, &documents(&project.tree()), "reading code");
}

/// The `unavailable-grammar` diagnostics of `result`, as (path, line, message).
fn unavailable(result: &Value) -> Vec<(String, u64, String)> {
    let diagnostics = result["diagnostics"].as_array().unwrap();
    diagnostics
        .iter()
        .filter(|d| d["code"] == "unavailable-grammar")
        .map(|d| {
            (
                d["path"].as_str().unwrap().to_owned(),
                d["line"].as_u64().unwrap(),
                d["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Requires that every operation that reads code reports one `unavailable-grammar` at `line`
/// of the configuration whose message holds `reason`, and gives Rust markers file-and-line
/// endpoints while Python keeps its grammar.
fn falls_back(project: &Project, line: u64, reason: &str) {
    let validated = project.json(&["project", "validate"], 1);
    let relations = project.json(&["relation", "list", "REQ-AUDIT"], 0);
    let got = project.json(&["get", "src/login.rs:12"], 0);
    for result in [&validated, &relations, &got] {
        let found = unavailable(result);
        assert_eq!(found.len(), 1, "{result}");
        let (path, at, message) = &found[0];
        assert_eq!(
            (path.as_str(), *at),
            (".mara/config.toml", line),
            "{result}"
        );
        assert!(message.contains(reason), "{message}");
        assert!(message.contains("rust"), "{message}");
    }
    let mut mcp = project.mcp();
    let called = mcp.call("relation_list", json!({"node": "REQ-AUDIT"}));
    assert_eq!(called, relations);
    drop(mcp);
    assert_eq!(
        links(project, "REQ-AUDIT", "implemented_by"),
        [
            at("scripts/tool.py", 8, Some("Audit"), 7),
            at("src/login.rs", 2, None, 2),
            at("src/login.rs", 10, None, 10),
            at("src/login.rs", 23, None, 23),
            at("src/login.rs", 32, None, 32),
        ]
    );
    // At the basic level `get` gives the file from the line.
    assert_eq!(got["node"]["line"], 12);
    assert_eq!(got["node"]["declaration"], Value::Null);
}

#[test]
fn a_grammar_missing_from_the_cache_falls_back_to_the_basic_level() {
    let mut project = Project::new();
    project.write("src/login.rs", RUST);
    project.write("scripts/tool.py", PYTHON);
    let (rust, line) = project.install_grammar("rust");
    project.install_grammar("python");
    fs::remove_dir_all(&rust).unwrap();
    falls_back(&project, line, "is not in the grammar cache");
    let validated = project.json(&["project", "validate"], 1);
    let message = &unavailable(&validated)[0].2;
    assert!(message.contains("`mara code add rust`"), "{message}");
}

#[test]
fn a_damaged_grammar_falls_back_to_the_basic_level() {
    let mut project = Project::new();
    project.write("src/login.rs", RUST);
    project.write("scripts/tool.py", PYTHON);
    let (rust, line) = project.install_grammar("rust");
    project.install_grammar("python");
    // A truncated `.wasm`, pinned with its own checksum, so that it fails as it loads.
    let wasm = fs::read(rust.join(mara::grammar::WASM_FILE)).unwrap();
    let tags = fs::read(rust.join(mara::grammar::TAGS_FILE)).unwrap();
    let truncated = &wasm[..wasm.len() / 2];
    let checksum = mara::grammar::checksum(truncated, &tags);
    let moved = rust.with_file_name(checksum.to_string());
    fs::rename(&rust, &moved).unwrap();
    fs::write(moved.join(mara::grammar::WASM_FILE), truncated).unwrap();
    let config = project.read(".mara/config.toml");
    let old = rust.file_name().unwrap().to_str().unwrap();
    project.write(
        ".mara/config.toml",
        config.replace(old, &checksum.to_string()),
    );
    falls_back(&project, line, "cannot be loaded");

    // A cached file that does not match its pin is not loaded.
    fs::write(moved.join(mara::grammar::WASM_FILE), &wasm).unwrap();
    falls_back(&project, line, "does not match its pin");
}

/// The configuration line of the pin of `version`.
fn pin_line(project: &Project, version: &str) -> u64 {
    let config = project.read(".mara/config.toml");
    let index = config.lines().position(|line| line.contains(version));
    index.expect("the pin") as u64 + 1
}

#[test]
fn a_grammar_out_of_time_falls_back_for_the_file() {
    let project = with_grammars();
    let (rust, python) = (pin_line(&project, "0.24.2"), pin_line(&project, "0.25.0"));
    // A parse cancelled at its budget: each file falls back, named with its language.
    let budget = [("MARA_TEST_GRAMMAR_BUDGET_MS", Path::new("0"))];
    let output = project
        .mara(&["--format", "json", "project", "validate"], &budget)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let found = unavailable(&result);
    let place: Vec<(&str, u64)> = (found.iter())
        .map(|(path, line, _)| (path.as_str(), *line))
        .collect();
    assert_eq!(
        place,
        [(".mara/config.toml", rust), (".mara/config.toml", python)]
    );
    for ((.., message), (language, file)) in found
        .iter()
        .zip([("rust", "src/login.rs"), ("python", "scripts/tool.py")])
    {
        assert!(
            message.contains(language) && message.contains(file),
            "{message}"
        );
        assert!(message.contains("did not finish"), "{message}");
    }
    // The basic level's diagnostic of the string literal is reported too.
    let codes: Vec<&str> = (result["diagnostics"].as_array().unwrap().iter())
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"invalid-marker"), "{codes:?}");

    // A grammar that never returns from a file: the request ends, the file and the rest of
    // its language fall back, and the other language keeps its grammar.
    let stall = [
        ("MARA_TEST_GRAMMAR_STALL", Path::new("struct Session;")),
        ("MARA_TEST_GRAMMAR_BUDGET_MS", Path::new("200")),
    ];
    let started = std::time::Instant::now();
    let output = project
        .mara(
            &["--format", "json", "relation", "list", "REQ-AUDIT"],
            &stall,
        )
        .output()
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert_eq!(output.status.code(), Some(0));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let found = unavailable(&result);
    assert_eq!(found.len(), 1, "{found:?}");
    let (path, at, message) = &found[0];
    assert_eq!((path.as_str(), *at), (".mara/config.toml", rust));
    assert!(
        message.contains("rust") && message.contains("src/login.rs"),
        "{message}"
    );
    assert!(
        message.contains("no longer used in this process"),
        "{message}"
    );
    assert_eq!(message.matches(", so ").count(), 1, "{message}");
    let sources: Vec<(Value, Value)> = (result["connections"].as_array().unwrap().iter())
        .map(|c| {
            (
                c["neighbour"]["declaration"].clone(),
                c["source"]["line"].clone(),
            )
        })
        .collect();
    assert_eq!(
        sources,
        [
            (json!("Audit"), json!(7)),
            (Value::Null, json!(2)),
            (Value::Null, json!(10)),
            (Value::Null, json!(23)),
            (Value::Null, json!(32)),
        ]
    );
}

/// The environment of a grammar that never returns from a file holding `text`, within a
/// budget of 200 ms: the request waits 1.4 s for it.
fn stalling(text: &str) -> [(&'static str, &Path); 2] {
    [
        ("MARA_TEST_GRAMMAR_STALL", Path::new(text)),
        ("MARA_TEST_GRAMMAR_BUDGET_MS", Path::new("200")),
    ]
}

/// A Rust file whose raw string holds a line that is a marker only at the basic level.
const RAW: &str = "// @mara implements REQ-LOGIN\npub fn example() -> &'static str {\n    \
                   r#\"\n// @mara implements REQ-LOGIN\n\"#\n}\n";

/// A project with the Rust grammar, [`RAW`] in `src/a.rs` and a file that stalls the grammar in
/// `src/z.rs`, which the inventory reaches after `src/a.rs`.
fn stalled() -> Project {
    let mut project = Project::new();
    project.write("src/a.rs", RAW);
    project.write("src/z.rs", "// stall here\nfn z() {}\n");
    project.install_grammar("rust");
    project
}

#[test]
fn a_grammar_dropped_during_a_request_leaves_its_language_at_the_basic_level() {
    let project = stalled();
    let stall = stalling("stall here");
    // Without the stall, the raw string's line is no marker.
    assert_eq!(
        links(&project, "REQ-LOGIN", "implemented_by"),
        [at("src/a.rs", 2, Some("example"), 1)]
    );
    let output = project
        .mara(
            &["--format", "json", "relation", "list", "REQ-LOGIN"],
            &stall,
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let found = unavailable(&result);
    assert_eq!(found.len(), 1, "{found:?}");
    let message = &found[0].2;
    assert!(message.contains("src/z.rs"), "{message}");
    assert_eq!(message.matches(", so ").count(), 1, "{message}");
    // `src/a.rs`, scanned before the grammar was dropped, is read at the basic level too.
    let sources: Vec<(Value, Value)> = (result["connections"].as_array().unwrap().iter())
        .map(|c| {
            (
                c["neighbour"]["declaration"].clone(),
                c["source"]["line"].clone(),
            )
        })
        .collect();
    assert_eq!(sources, [(Value::Null, json!(1)), (Value::Null, json!(4))]);

    // A rename in such a request rewrites the markers of the basic level, as its diagnostic says.
    let output = project
        .mara(
            &[
                "--format",
                "json",
                "item",
                "rename",
                "REQ-LOGIN",
                "REQ-SIGN-IN",
            ],
            &stall,
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        project.read("src/a.rs"),
        RAW.replace("REQ-LOGIN", "REQ-SIGN-IN")
    );
}

#[test]
fn a_dropped_grammar_stays_dropped_in_its_process() {
    let project = stalled();
    let mut mcp = project.mcp_with(&stalling("stall here"));
    let mut threads = Vec::new();
    for index in 0..3 {
        let started = std::time::Instant::now();
        let answer = mcp.call("relation_list", json!({"node": "REQ-LOGIN"}));
        let elapsed = started.elapsed();
        let found = unavailable(&answer);
        assert_eq!(found.len(), 1, "call {index}: {answer}");
        let message = &found[0].2;
        assert!(message.contains("src/z.rs"), "call {index}: {message}");
        // The first call waits for the grammar until it drops it.
        if index == 0 {
            assert!(
                elapsed >= std::time::Duration::from_millis(1400),
                "{elapsed:?}"
            );
        }
        threads.push(grammar_threads(mcp.pid()));
    }
    // The grammar's one thread, left running by the first call, and no other: a later call
    // neither loads the grammar again nor starts a thread that would stall on `src/z.rs`.
    assert_eq!(threads[0].len(), 1, "{threads:?}");
    assert_eq!(threads[1], threads[0], "{threads:?}");
    assert_eq!(threads[2], threads[0], "{threads:?}");
}

#[test]
fn a_grammar_that_does_not_load_in_time_stays_dropped_in_its_process() {
    let project = stalled();
    // The first call waits at least the limit; a later call that loaded or waited again would
    // take that long too, so a bound of half the limit tells them apart on a busy machine.
    let limit = std::time::Duration::from_secs(6);
    let millis = limit.as_millis().to_string();
    let env = [
        ("MARA_TEST_GRAMMAR_LOAD_STALL", Path::new("rust")),
        ("MARA_TEST_GRAMMAR_LOAD_MS", Path::new(&millis)),
    ];
    let mut mcp = project.mcp_with(&env);
    let mut threads = Vec::new();
    for index in 0..3 {
        let started = std::time::Instant::now();
        let answer = mcp.call("relation_list", json!({"node": "REQ-LOGIN"}));
        let elapsed = started.elapsed();
        let found = unavailable(&answer);
        assert_eq!(found.len(), 1, "call {index}: {answer}");
        let message = &found[0].2;
        assert!(
            message.contains("did not load within 6.0 s")
                && message.contains("no longer used in this process"),
            "call {index}: {message}"
        );
        // The basic level: the raw string's line is a marker.
        assert_eq!(
            answer["connections"].as_array().unwrap().len(),
            2,
            "{answer}"
        );
        // Only the first call waits for the load; the others neither wait nor load.
        match index {
            0 => assert!(elapsed >= limit, "{elapsed:?}"),
            _ => assert!(elapsed < limit / 2, "call {index}: {elapsed:?}"),
        }
        threads.push(grammar_threads(mcp.pid()));
    }
    // The stalled load's thread, left running by the first call, and no other.
    assert_eq!(threads[0].len(), 1, "{threads:?}");
    assert_eq!(threads[1], threads[0], "{threads:?}");
    assert_eq!(threads[2], threads[0], "{threads:?}");
}

#[test]
fn each_file_that_falls_back_is_named() {
    let mut project = Project::new();
    let text = "// @mara implements REQ-LOGIN\nfn same() {}\n";
    project.write("src/a.rs", text);
    project.write("src/b.rs", text);
    let line = project.install_grammar("rust").1;
    let budget = [("MARA_TEST_GRAMMAR_BUDGET_MS", Path::new("0"))];
    let output = project
        .mara(&["--format", "json", "project", "validate"], &budget)
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut named: Vec<(u64, &str)> = unavailable(&result)
        .iter()
        .map(|(_, at, message)| {
            let file = ["src/a.rs", "src/b.rs"]
                .into_iter()
                .find(|file| message.contains(file))
                .unwrap_or_else(|| panic!("no file in {message}"));
            (*at, file)
        })
        .collect();
    named.sort();
    assert_eq!(named, [(line, "src/a.rs"), (line, "src/b.rs")]);
}

#[test]
fn a_module_that_is_no_grammar_falls_back() {
    let mut project = Project::new();
    project.write("src/login.rs", RUST);
    project.write("scripts/tool.py", PYTHON);
    let (rust, line) = project.install_grammar("rust");
    project.install_grammar("python");
    // A valid WebAssembly side module that exports `tree_sitter_rust` as a function without a
    // result: tree-sitter's store would assert on it and end the process.
    let module = [
        &b"\0asm\x01\0\0\0"[..],
        b"\x00\x0f\x08dylink.0\x01\x04\x00\x00\x00\x00",
        b"\x01\x04\x01\x60\x00\x00",
        b"\x03\x02\x01\x00",
        b"\x07\x14\x01\x10tree_sitter_rust\x00\x00",
        b"\x0a\x04\x01\x02\x00\x0b",
    ]
    .concat();
    let tags = fs::read(rust.join(mara::grammar::TAGS_FILE)).unwrap();
    let checksum = mara::grammar::checksum(&module, &tags);
    let moved = rust.with_file_name(checksum.to_string());
    fs::rename(&rust, &moved).unwrap();
    fs::write(moved.join(mara::grammar::WASM_FILE), &module).unwrap();
    let config = project.read(".mara/config.toml");
    let old = rust.file_name().unwrap().to_str().unwrap();
    project.write(
        ".mara/config.toml",
        config.replace(old, &checksum.to_string()),
    );
    falls_back(&project, line, "is no tree-sitter grammar");
}

/// The threads of the process `pid` that run a grammar, by thread ID.
fn grammar_threads(pid: u32) -> Vec<String> {
    let mut threads: Vec<String> = fs::read_dir(format!("/proc/{pid}/task"))
        .unwrap()
        .filter_map(|task| {
            let task = task.ok()?.path();
            let name = fs::read_to_string(task.join("comm")).ok()?;
            name.starts_with("mara-grammar")
                .then(|| task.file_name().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    threads.sort();
    threads
}

#[test]
fn one_process_loads_a_grammar_once() {
    let mut project = Project::new();
    project.write(
        "src/lib.rs",
        "// @mara implements REQ-LOGIN\npub fn one() {\n    two();\n}\n",
    );
    let (cache, line) = project.install_grammar("rust");
    let mut mcp = project.mcp();
    let mut threads = Vec::new();
    for _ in 0..2 {
        let got = mcp.call("get", json!({"node": "src/lib.rs:3"}));
        assert_eq!(got["node"]["declaration"], "one", "{got}");
        threads.push(grammar_threads(mcp.pid()));
    }
    // One grammar thread, the same for both calls: the second call loaded nothing.
    assert_eq!(threads[0].len(), 1, "{threads:?}");
    assert_eq!(threads[1], threads[0]);

    // A request still checks the cached files: one removed is seen.
    fs::remove_dir_all(&cache).unwrap();
    let got = mcp.call("get", json!({"node": "src/lib.rs:3"}));
    let found = unavailable(&got);
    assert_eq!(found.len(), 1, "{got}");
    assert_eq!(found[0].1, line);
    assert_eq!(got["node"]["declaration"], Value::Null);
}
