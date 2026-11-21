//! `code add`: its row of VER-CODE-WORK-COUNTERS and its VER-SCALING case, from the loopback
//! repository of VER-CODE-ADD; and the case of VER-CODE-ADD that no other operation connects to
//! another machine.

use std::fs;
use std::path::{Path, PathBuf};

use mara_engine::work::WorkCounts;
use serde_json::{Map, Value, json};

use crate::cli::Code;
use crate::corpus::Corpus;
use crate::grammar_server::{GrammarServer, fixture};
use crate::restore::Written;
use crate::{PULL_REQUEST_SIZES, SEED, cli, mcp, scaling};

/// A server with the fixture Rust grammar, and its `code add` arguments.
fn repository() -> (GrammarServer, Vec<String>) {
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    server.serve_grammar("rust", version, wasm, tags);
    let args = [
        "code",
        "add",
        "rust",
        "--repository",
        &server.url(),
        "--version",
        version,
    ];
    (server, args.map(str::to_owned).to_vec())
}

fn temporary(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("create a temporary directory")
}

#[test]
fn code_add_counts() {
    let (server, args) = repository();
    for size in PULL_REQUEST_SIZES {
        let corpus = Corpus::generate(size, SEED);
        let home = temporary("mara-home-");
        let run = cli::run_with(corpus.root(), &args, &[("HOME", home.path())], 0);
        // The configuration: read and hashed for the edit, then checked at step 2 and again
        // before its rename (DES-WRITE-PROCEDURE). No document or code file is read, parsed or
        // scanned.
        let expected = WorkCounts {
            file_reads: 3,
            hashes: 3,
            ..WorkCounts::default()
        };
        assert_eq!(run.work, expected, "mara code add at {size}×");
    }
    assert_eq!(server.connections(), 2 * PULL_REQUEST_SIZES.len());
}

/// Times `code add` alone: the configuration it pinned is restored before each run, outside the
/// timed span. Each run trusts the repository anew, so it fetches and caches the files again.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "times a release build: cargo test --release --package mara --test performance"
)]
fn code_add_growth_per_doubling() {
    if cfg!(debug_assertions) {
        panic!("VER-SCALING times a release build");
    }
    let corpora: Vec<(usize, Corpus)> = PULL_REQUEST_SIZES
        .iter()
        .map(|&size| (size, Corpus::generate(size, SEED)))
        .collect();
    let (_server, args) = repository();
    let home = temporary("mara-home-");
    let env = [("HOME", home.path())];
    scaling::check_writes(
        &corpora,
        "mara code add",
        |_, corpus| {
            cli::run_with(corpus.root(), &args, &env, 0);
        },
        Corpus::root,
    );
}

/// One run of an operation in the no-download case: its arguments after its words, the exit
/// status it must give with the grammar unavailable, whether that is a failed request (an MCP
/// error) rather than a result, and whether it reads code.
struct Probe {
    args: Vec<String>,
    status: i32,
    fails: bool,
    code: bool,
}

/// Every other operation of the usage through the CLI, and every tool of `tools/list` through
/// `mara mcp`, as the harness's cases run them, with the corpus's one grammar pin naming the
/// counting loopback repository and the per-user cache, valid and empty, inside the test's `HOME`:
/// no connection, the expected exit status, and `unavailable-grammar` in the result of each one
/// that reads code and returns diagnostics (VER-CODE-ADD).
#[test]
#[cfg(target_os = "linux")]
fn no_other_operation_connects_to_another_machine() {
    let corpus = Corpus::generate(1, SEED);
    let server = GrammarServer::start();
    let (wasm, tags, version) = fixture("rust");
    let checksum = mara::grammar::checksum(&wasm, &tags);
    let config_path = corpus.root().join(".mara/config.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let header = "[code.languages.rust]\n";
    assert_eq!(config.matches("[code.languages.").count(), 1, "{config}");
    let pin = format!(
        "grammar = {{ version = \"{version}\", checksum = \"blake3:{checksum}\", repository = \
         \"{}\" }}\n",
        server.url()
    );
    fs::write(
        &config_path,
        config.replacen(header, &format!("{header}{pin}"), 1),
    )
    .unwrap();
    // The cache the binary reads on Linux without `XDG_CACHE_HOME` (DES-GRAMMAR-ASSETS).
    let home = temporary("mara-home-");
    let cache = home.path().join(".cache/mara/grammars");
    fs::create_dir_all(&cache).unwrap();
    // Mode 0700, as a runtime directory must be, and short enough for the socket path.
    let runtime = tempfile::Builder::new()
        .prefix("mara-run-")
        .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
        .tempdir()
        .expect("create a temporary directory");
    let daemons = crate::daemons::Daemons::new(runtime.path(), None);
    let outside = temporary("mara-targets-");

    let probes = probes(&corpus, outside.path());
    let usage = cli::run(corpus.root(), &[] as &[&str], 0);
    let usage = String::from_utf8(usage.output.stdout).unwrap();
    let operations: Vec<String> = (usage.lines())
        .filter_map(|line| line.strip_prefix("  mara "))
        .map(|line| {
            let words = line.split(' ');
            let words = words.take_while(|word| !word.starts_with(['<', '[']));
            words.collect::<Vec<_>>().join(" ")
        })
        .filter(|words| words != "code add")
        .collect();
    let env = [("HOME", home.path())];
    for words in &operations {
        if words == "mcp" {
            continue; // Its tools follow.
        }
        if words.starts_with("daemon ") {
            continue; // The daemon's cycle follows.
        }
        let found: Vec<&(String, Probe)> = probes.iter().filter(|(w, _)| w == words).collect();
        assert!(!found.is_empty(), "no probe of `mara {words}`: add one");
        for (_, probe) in found {
            let mut args = vec!["--format".to_owned(), "json".to_owned()];
            args.extend(words.split(' ').map(str::to_owned));
            args.extend(probe.args.iter().cloned());
            let mut text = String::new();
            let mut status = None;
            let written = Written::learn(corpus.root(), || {
                let run = cli::run_any(corpus.root(), &args, &env);
                status = run.output.status.code();
                text = String::from_utf8_lossy(&run.output.stdout).into_owned();
            });
            written.restore();
            clear(outside.path());
            let name = format!("mara {}", args.join(" "));
            assert_eq!(server.connections(), 0, "{name}");
            assert_eq!(status, Some(probe.status), "{name}: {text}");
            let failed = text.starts_with("{\"format_version\":1,\"error\"");
            assert_eq!(failed, probe.fails, "{name}: {text}");
            reports_fallback(&name, probe.code, &text);
        }
    }

    // The daemon's operations, and a request that reads code served by the daemon.
    let daemon_env = [
        ("HOME", home.path()),
        ("XDG_RUNTIME_DIR", runtime.path()),
        ("TMPDIR", runtime.path()),
    ];
    let validate = ["--format", "json", "project", "validate"];
    // How many requests the daemon has served: the probe it serves must raise it by one, so that
    // no direct-mode fallback answered it.
    let served = || {
        let run = cli::run_any(
            corpus.root(),
            &["--format", "json", "daemon", "status"],
            &daemon_env,
        );
        let status: Value = serde_json::from_slice(&run.output.stdout).unwrap();
        status["served"].as_u64().expect("a running daemon's count")
    };
    let mut before = None;
    for args in [
        &["daemon", "status"][..],
        &["daemon", "start"],
        &validate,
        &["daemon", "stop"],
    ] {
        if args == validate {
            before = Some(served());
        }
        let run = cli::run_any(corpus.root(), args, &daemon_env);
        if args == validate {
            assert_eq!(
                Some(served()),
                before.map(|count| count + 1),
                "the daemon served it"
            );
        }
        let text = String::from_utf8_lossy(&run.output.stdout).into_owned();
        let expected = if args == validate { 1 } else { 0 };
        assert_eq!(
            run.output.status.code(),
            Some(expected),
            "mara {args:?}: {text}{}",
            String::from_utf8_lossy(&run.output.stderr)
        );
        assert_eq!(server.connections(), 0, "mara {args:?}");
        reports_fallback(&format!("mara {args:?}"), args == validate, &text);
    }
    assert!(daemons.running().is_empty(), "a daemon runs after `stop`");
    for words in &operations {
        if words.starts_with("daemon ") {
            assert!(
                ["daemon status", "daemon start", "daemon stop"].contains(&words.as_str()),
                "no probe of `mara {words}`: add one"
            );
        }
    }

    // Every tool of `tools/list`, one session each, the server's `HOME` the same.
    let listing = mcp::session(corpus.root(), &env, &[]);
    assert!(
        listing.run.output.status.success(),
        "mara mcp: {:?}",
        listing.run.output
    );
    let listed = listing.tools;
    let tools = listed.as_array().expect("a tool list");
    assert!(!tools.is_empty());
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let words = name.replace('_', " ");
        let found: Vec<&(String, Probe)> = probes.iter().filter(|(w, _)| *w == words).collect();
        assert!(!found.is_empty(), "no probe of the tool `{name}`: add one");
        for (_, probe) in found {
            let arguments = arguments(tool, &probe.args, corpus.root());
            let mut response = Value::Null;
            let written = Written::learn(corpus.root(), || {
                let session = mcp::session(corpus.root(), &env, &[(name, &arguments)]);
                let output = &session.run.output;
                assert!(output.status.success(), "mara mcp for {name}: {output:?}");
                response = session.responses[0].clone();
            });
            written.restore();
            clear(outside.path());
            let case = format!("mara mcp {name} {arguments}");
            assert_eq!(server.connections(), 0, "{case}");
            let failed = response["result"]["isError"].as_bool();
            assert_eq!(failed, Some(probe.fails), "{case}: {response}");
            let text = response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("");
            reports_fallback(&case, probe.code, text);
        }
    }
    let left: Vec<PathBuf> = fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(left.is_empty(), "nothing cached: {left:?}");
}

/// The probes of every operation but `code add`, the daemon's and `mcp`, by their words: the
/// harness's cases, which read code as they say, then `project init` into a directory below `outside`,
/// which each run empties again, `project convert`, which refuses a project that is no 0.3.0 one, and `prime`.
fn probes(corpus: &Corpus, outside: &Path) -> Vec<(String, Probe)> {
    let operations = [
        "project validate",
        "project fix",
        "project init",
        "project convert",
        "item validate",
        "schema validate",
        "search",
        "get",
        "relation list",
        "item list",
        "schema list",
        "schema get",
        "query list",
        "query execute",
        "query view",
        "query gate",
        "query report",
        "item move",
        "item rename",
        "item create",
        "item update",
        "item delete",
        "relation add",
        "relation remove",
        "prime",
    ];
    let split = |args: &[String]| -> (String, Vec<String>) {
        let words = (operations.iter())
            .filter(|words| {
                let words: Vec<&str> = words.split(' ').collect();
                args.len() >= words.len() && args[..words.len()] == words[..]
            })
            .max_by_key(|words| words.len())
            .unwrap_or_else(|| panic!("no operation in {args:?}"));
        let count = words.split(' ').count();
        (words.to_string(), args[count..].to_vec())
    };
    let mut probes = Vec::new();
    for case in cli::cases(corpus) {
        let (words, args) = split(&case.args);
        // The corpus is valid but for the grammar, which `project validate` reports as an error.
        let status = if words == "project validate" {
            1
        } else {
            case.status
        };
        let code = case.code != Code::None;
        let probe = Probe {
            args,
            status,
            fails: false,
            code,
        };
        probes.push((words, probe));
    }
    let target = outside.join("init");
    let probe = Probe {
        args: vec!["--project".to_owned(), target.to_str().unwrap().to_owned()],
        status: 0,
        fails: false,
        code: false,
    };
    probes.push(("project init".to_owned(), probe));
    let refused = Probe {
        args: Vec::new(),
        status: 1,
        fails: true,
        code: false,
    };
    probes.push(("project convert".to_owned(), refused));
    let prime = Probe {
        args: Vec::new(),
        status: 0,
        fails: false,
        code: false,
    };
    probes.push(("prime".to_owned(), prime));
    probes
}

/// Removes what a run of `project init` wrote below `outside`.
fn clear(outside: &Path) {
    for entry in fs::read_dir(outside).unwrap() {
        fs::remove_dir_all(entry.unwrap().path()).unwrap();
    }
}

/// Requires `unavailable-grammar` in the JSON result `text` of an operation that reads code
/// when that result carries diagnostics, as a failure's `error` does too.
fn reports_fallback(name: &str, code: bool, text: &str) {
    let Ok(result) = serde_json::from_str::<Value>(text) else {
        return;
    };
    let diagnostics = match &result["diagnostics"] {
        Value::Array(diagnostics) => diagnostics,
        _ => match &result["error"]["diagnostics"] {
            Value::Array(diagnostics) => diagnostics,
            _ => return,
        },
    };
    if code {
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "unavailable-grammar"),
            "{name}: no unavailable-grammar in {text}"
        );
    }
}

/// The MCP arguments of the CLI arguments `args` of `tool`: positional arguments by the names
/// its schema requires first, each option by its name, a repeated one or one the schema types as
/// an array as an array, a whole number as a number, and each `--input <name>=<value>` in one
/// object, a value `@<file>` as the rows that file holds.
fn arguments(tool: &Value, args: &[String], root: &Path) -> Value {
    let schema = &tool["inputSchema"];
    let required: Vec<&str> = (schema["required"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let mut out = Map::new();
    let mut positional = 0;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let Some(option) = arg.strip_prefix("--") else {
            out.insert(required[positional].to_owned(), json!(arg));
            positional += 1;
            continue;
        };
        let value = iter.next().expect("an option's value").clone();
        let property = &schema["properties"][option];
        match property["type"].as_str() {
            Some("array") => {
                let entry = out.entry(option.to_owned()).or_insert_with(|| json!([]));
                entry.as_array_mut().unwrap().push(json!(value));
            }
            Some("integer") => {
                out.insert(option.to_owned(), json!(value.parse::<i64>().unwrap()));
            }
            Some("object") => {
                let (name, value) = value.split_once('=').expect("<name>=<value>");
                let value = match value.strip_prefix('@') {
                    Some(file) => {
                        let text = fs::read_to_string(root.join(file)).unwrap();
                        serde_json::from_str(&text).expect("a file of JSON rows")
                    }
                    None => json!(value),
                };
                let entry = out.entry(option.to_owned()).or_insert_with(|| json!({}));
                entry
                    .as_object_mut()
                    .unwrap()
                    .insert(name.to_owned(), value);
            }
            _ => {
                out.insert(option.to_owned(), json!(value));
            }
        }
    }
    Value::Object(out)
}
