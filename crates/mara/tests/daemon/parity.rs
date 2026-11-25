//! The daemon case of VER-HOST-COMPARISON (`docs/verification.mara.md`), which verifies
//! REQ-SAME-RESULT-ON-EVERY-HOST: each document operation that the daemon serves, failing
//! requests and writes included, sent from the CLI and from `mara mcp` to a daemon of the test
//! whose session an earlier request opened, returns the JSON of CLI direct mode and leaves the
//! files that direct mode leaves. Generated MIDs are paired as VER-CLI-MCP-PARITY pairs them.
//! That daemon answered each request: `served` of `mara daemon status` rose by one across it
//! (VER-DAEMON-START). The project declares Rust at the basic level, so the operations that read
//! code (DES-CODE-READERS) read the markers of its code files.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[path = "../support/requests.rs"]
pub(crate) mod requests;

use requests::{Files, Usage, files, mcp_request, pair_mids, usage};

use crate::mcp::Server;
use crate::{Run, Test, settle};

/// The document operations that run in the client's process, never in the daemon
/// (DES-DAEMON-LIFECYCLE), so the daemon case leaves them out.
const IN_PROCESS: &[&str] = &["project_init", "project_convert"];

/// A project, a daemon of the test that holds its session, and a `mara mcp` that sends its
/// calls to that daemon.
struct Comparison<'a> {
    test: &'a Test,
    root: PathBuf,
    pid: u32,
    /// The operation requests that the daemon has answered, as `mara daemon status` last
    /// reported them.
    served: u64,
    server: Server,
    /// A `mara mcp --no-daemon`: what the server prints for arguments it rejects itself, which
    /// name the parameter, not the CLI's option (DES-MCP-SERVER).
    direct_server: Server,
    usages: Vec<Usage>,
    tools: Vec<Value>,
    /// The tools of the requests sent.
    sent: BTreeSet<String>,
}

impl<'a> Comparison<'a> {
    /// Starts the daemon outside the project, opens the project's session with a read sent to
    /// it, and starts the MCP server.
    fn new(test: &'a Test, root: PathBuf) -> Comparison<'a> {
        let pid = test.start_in(test.path(), &[]);
        assert!(test.sessions().is_empty());
        let mut comparison = Comparison {
            test,
            server: Server::start(test, &root, &[], &[]),
            direct_server: Server::start(test, &root, &["--no-daemon"], &[]),
            root,
            pid,
            served: 0,
            usages: usage(),
            tools: Vec::new(),
            sent: BTreeSet::new(),
        };
        comparison.answered(&["item", "list"], 0);
        comparison
            .test
            .run_in(&comparison.root, &["--format", "json", "item", "list"], &[])
            .succeeded();
        comparison.answered(&["item", "list"], 1);
        assert_eq!(test.sessions(), [comparison.root.clone()]);
        comparison.tools = comparison.server.tools();
        comparison
    }

    /// Requires that the daemon of the test answered `count` operation requests since the
    /// previous call, the request `args` among them: it still runs, and `served` of `mara daemon
    /// status` rose by `count`.
    #[track_caller]
    fn answered(&mut self, args: &[&str], count: u64) {
        let status = self.test.status();
        assert_eq!(status["pid"], self.pid, "{args:?}: {status}");
        let served = status["served"].as_u64().expect("a count of requests");
        assert_eq!(
            served - self.served,
            count,
            "{args:?}: the requests the daemon answered"
        );
        self.served = served;
    }

    /// `mara --format json <args>` in direct mode, then through the daemon from the CLI and from
    /// `mara mcp`, each on the files that the project held before. Each prints what direct mode
    /// printed and leaves the files direct mode left, once the MIDs it generated are replaced by
    /// those of direct mode at the same path and line. The daemon answered each when `served`,
    /// and neither otherwise. Leaves the project as direct mode left it: the direct run's output.
    fn request(&mut self, args: &[&str], served: bool) -> Run {
        self.request_as(args, served, served)
    }

    /// [`Comparison::request`] whose CLI request the daemon answers when `cli_served`, and whose
    /// MCP call when `mcp_served`.
    fn request_as(&mut self, args: &[&str], cli_served: bool, mcp_served: bool) -> Run {
        let served = mcp_served;
        let cli: Vec<&str> = ["--format", "json"].iter().chain(args).copied().collect();
        let before = files(&self.root);
        let direct = self.test.direct(&self.root, &cli);
        let expected = files(&self.root);
        restore(&self.root, &before);
        let daemon = self.test.run_in(&self.root, &cli, &[]);
        self.answered(args, u64::from(cli_served));
        let (stdout, after) =
            pair_mids(args, &before, &expected, &daemon.stdout, files(&self.root));
        assert_eq!(
            (daemon.status, stdout.as_str(), daemon.stderr.as_str()),
            (
                direct.status,
                direct.stdout.as_str(),
                direct.stderr.as_str()
            ),
            "{args:?} from the CLI through the daemon"
        );
        same_files(&expected, &after, args, "the CLI through the daemon");

        restore(&self.root, &before);
        let (tool, arguments) = mcp_request(&self.usages, &self.tools, args, &self.root);
        let (text, is_error) = self.server.call_tool(&tool, arguments.clone());
        self.answered(args, u64::from(mcp_served));
        let (text, after) = pair_mids(args, &before, &expected, &text, files(&self.root));
        let direct_text = match served {
            true => direct.stdout.clone(),
            false => {
                let (text, is_error) = self.direct_server.call_tool(&tool, arguments);
                assert!(is_error, "{args:?}: {text}");
                format!("{text}\n")
            }
        };
        assert_eq!(
            format!("{text}\n"),
            direct_text,
            "{args:?} from MCP through the daemon"
        );
        assert_eq!(is_error, direct.json().get("error").is_some(), "{args:?}");
        same_files(&expected, &after, args, "MCP through the daemon");

        restore(&self.root, &expected);
        self.sent.insert(tool);
        direct
    }

    /// [`Comparison::request`] that the daemon answers, whose exit status in direct mode is
    /// `status`: its JSON.
    fn expect(&mut self, args: &[&str], status: i32) -> Value {
        let run = self.request(args, true);
        assert_eq!(run.status.code(), Some(status), "{args:?}: {run:?}");
        run.json()
    }

    /// A request that fails with `code`.
    fn refused(&mut self, args: &[&str], code: &str) {
        let json = self.expect(args, 1);
        assert_eq!(json["error"]["code"], code, "{args:?}: {json}");
    }

    /// Arguments that the operation rejects when it runs, in the daemon: options that it needs
    /// together.
    fn rejected(&mut self, args: &[&str]) {
        let json = self.expect(args, 2);
        assert_eq!(json["error"]["code"], "invalid-argument", "{args:?}");
    }

    /// A required option left out: the CLI's operation rejects it in the daemon, and `mara mcp`
    /// before it sends the call, naming the parameter (DES-MCP-SERVER).
    fn rejected_by_mcp(&mut self, args: &[&str]) {
        let run = self.request_as(args, true, false);
        assert_eq!(run.status.code(), Some(2), "{args:?}: {run:?}");
        assert_eq!(run.json()["error"]["code"], "invalid-argument", "{args:?}");
    }

    /// Arguments that the client rejects before it sends the request: a value that the option
    /// does not take (DES-OPERATIONS).
    fn rejected_by_client(&mut self, args: &[&str]) {
        let run = self.request(args, false);
        assert_eq!(run.status.code(), Some(2), "{args:?}: {run:?}");
        assert_eq!(run.json()["error"]["code"], "invalid-argument", "{args:?}");
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Requires that every document operation of the tool list but those of [`IN_PROCESS`] was
    /// sent, so that an operation added later is not left out, and that the daemon which opened
    /// the session is still the only daemon of the test.
    fn finish(self) {
        self.direct_server.finish();
        let operations: BTreeSet<String> = (self.tools.iter())
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .filter(|name| !IN_PROCESS.contains(&name.as_str()))
            .collect();
        assert_eq!(self.sent, operations, "the operations sent");
        assert_eq!(
            self.test.wait_for_daemon_beside(&[self.server.child.id()]),
            self.pid
        );
        self.server.finish();
    }
}

/// Makes `root` hold exactly `files`, writing only what differs, at the same path.
fn restore(root: &Path, files: &Files) {
    let now = self::files(root);
    // A directory's entries come after it, so in reverse they go before it.
    for path in now.keys().rev().filter(|path| !files.contains_key(*path)) {
        match path.strip_suffix('/') {
            Some(directory) => fs::remove_dir(root.join(directory)).unwrap(),
            None => fs::remove_file(root.join(path)).unwrap(),
        }
    }
    for (path, bytes) in files
        .iter()
        .filter(|(path, bytes)| now.get(*path) != Some(bytes))
    {
        match path.strip_suffix('/') {
            Some(directory) => fs::create_dir_all(root.join(directory)).unwrap(),
            None => fs::write(root.join(path), bytes).unwrap(),
        }
    }
}

#[track_caller]
fn same_files(expected: &Files, files: &Files, args: &[&str], host: &str) {
    let differ: Vec<&String> = (expected.keys().chain(files.keys()))
        .filter(|path| expected.get(*path) != files.get(*path))
        .collect();
    assert!(
        differ.is_empty(),
        "{args:?} from {host}: the files differ: {differ:?}"
    );
}

/// The program of the queries: every item is a subject; an item fails with a `fail` result, or
/// without one when the results are complete, and is unavailable without one otherwise.
const GATE_PROGRAM: &str = r#".include "mara.dl"
.decl listed(m: Mid, id: Id, status: Status)
listed(m, i, s) :- item(m, _, i, s, _, _).
.decl candidate(c: symbol)
.decl result(m: Mid, outcome: symbol, run: number)
.decl subject(m: Mid)
subject(m) :- item(m, _, _, _, _, _), candidate(_).
.decl has(m: Mid)
has(m) :- result(m, _, _).
.decl failed(m: Mid, outcome: symbol)
failed(m, o) :- subject(m), result(m, o, _), o = "fail".
failed(m, "missing") :- subject(m), !has(m), source_complete("result").
.decl missing(m: Mid)
missing(m) :- subject(m), !has(m), !source_complete("result").
"#;

const QUERY_ENTRIES: &str = r#"
[rules]
programs = [".mara/rules/*.dl"]

[queries.listed]
program = ".mara/rules/gate.dl"
relation = "listed"
description = "Every item with its ID and status"

[views.listed-view]
program = ".mara/rules/gate.dl"
relation = "listed"
description = "The same rows as a table"

[gates.passed]
program = ".mara/rules/gate.dl"
description = "Named items pass"
subjects = ["subject"]
conditions = ["failed"]
unavailable = ["missing"]
inputs = ["candidate", "result"]

[queries.broken]
program = ".mara/rules/broken.dl"
relation = "bad"
description = "A program that does not compile"

[gates.broken-gate]
program = ".mara/rules/broken.dl"
description = "A gate over it"
subjects = ["bad"]
conditions = ["bad"]
"#;

/// Rust at the basic level (DES-CODE-MARKERS), declared before the template's `[rules]`, which
/// the test later replaces by hand.
const CODE: &str = "[code]\nformat_version = 1\n\n[code.languages.rust]\n\
                    include = [\"src/**/*.rs\"]\ncomment_prefixes = [\"//\"]\n\n";

/// An invariant over code links: the template's `readiness.dl` names `code_link`, so an item
/// validation reads code (DES-CODE-READERS). Its views and gates read code too.
const IMPLEMENTED: &str = "\n[invariants.requirement-implemented]\n\
                           program = \".mara/rules/readiness.dl\"\nrelation = \"not_implemented\"\n\
                           severity = \"warning\"\nmessage = \"No code implements this requirement\"\n";

/// The invariant's code, which `project validate` reports for each requirement no code
/// implements.
const NOT_IMPLEMENTED: &str = "requirement-implemented";

/// A project of the engineering template with one need, the inputs of the gate below and Rust
/// declared ([`CODE`]), written in direct mode, its files last changed at least 2 s before this
/// returns.
fn project(test: &Test) -> PathBuf {
    let root = test.path().join("project");
    fs::create_dir(&root).unwrap();
    test.direct(&root, &["project", "init"]).succeeded();
    let config = root.join(".mara/config.toml");
    let template = fs::read_to_string(&config).unwrap();
    let rules = template.find("[rules]\n").expect("the template's rules");
    let declared = format!(
        "{}{CODE}{}{IMPLEMENTED}",
        &template[..rules],
        &template[rules..]
    );
    fs::write(&config, declared).unwrap();
    let need = [
        "--format",
        "json",
        "item",
        "create",
        "need",
        "docs/needs.mara.md",
        "--title",
        "Fast answers",
        "--field",
        "status=draft",
        "--body",
        "Agents get answers quickly.",
    ];
    let mid = test.direct(&root, &need).succeeded().json()["item"]["mid"].clone();
    let results = json!([[mid, "pass", -2_147_483_648_i64]]);
    fs::write(root.join("results.json"), results.to_string()).unwrap();
    fs::write(
        root.join("wrong.json"),
        json!([[mid, "pass", 1.5]]).to_string(),
    )
    .unwrap();
    fs::write(root.join("verified.json"), "[]").unwrap();
    settle();
    fs::canonicalize(root).unwrap()
}

/// An agent's work through the daemon as in direct mode: it creates, relates, updates, reads,
/// moves and renames items, repairs hand edits with fix, runs the queries, and meets failures,
/// up to a schema broken by hand.
#[test]
fn the_daemon_answers_each_document_operation_as_direct_mode() {
    let test = Test::new();
    let root = project(&test);
    let mut c = Comparison::new(&test, root);
    c.expect(&["project", "validate"], 0);
    c.expect(&["schema", "list"], 0);
    c.expect(&["schema", "get", "requirement"], 0);
    c.expect(&["schema", "validate"], 0);
    // A document begun by hand with a heading and a sentence.
    c.write(
        "docs/reqs.mara.md",
        "# Requirements\n\nPerformance obligations.\n",
    );
    c.expect(
        &[
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--title",
            "Cold read time",
            "--field",
            "status=draft",
            "--body",
            "A cold read takes at most 200 ms.",
        ],
        0,
    );
    c.expect(
        &[
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--id",
            "REQ-WARM-READ",
            "--title",
            "Warm read time",
            "--field",
            "status=draft",
            "--body",
            "A warm read takes at most 20 ms.",
            "--line",
            "3",
        ],
        0,
    );
    c.expect(
        &[
            "relation",
            "add",
            "REQ-COLD-READ-TIME",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        0,
    );
    c.refused(
        &[
            "relation",
            "add",
            "NEED-FAST-ANSWERS",
            "satisfied_by",
            "REQ-WARM-READ",
        ],
        "inverse-relation",
    );

    // Code that implements the requirements, a marker that does not resolve, and the
    // operations that read code (DES-CODE-READERS).
    c.write(
        "src/read.rs",
        "// @mara implements REQ-COLD-READ-TIME\npub fn cold() {\n    \
         warm(); // @mara implements REQ-WARM-READ\n}\n",
    );
    c.write(
        "src/typo.rs",
        "// @mara implements REQ-COLD-READ\nfn typo() {}\n",
    );
    let validated = c.expect(&["project", "validate"], 1);
    let codes: Vec<&Value> = (validated["diagnostics"].as_array().unwrap().iter())
        .map(|diagnostic| &diagnostic["code"])
        .filter(|code| *code != NOT_IMPLEMENTED)
        .collect();
    assert_eq!(codes, [&json!("unresolved-marker")], "{validated}");
    let linked = c.expect(&["relation", "list", "REQ-COLD-READ-TIME"], 0);
    let neighbours: Vec<&Value> = (linked["connections"].as_array().unwrap().iter())
        .map(|connection| &connection["neighbour"]["kind"])
        .collect();
    assert!(neighbours.contains(&&json!("code")), "{linked}");
    c.expect(
        &[
            "relation",
            "list",
            "REQ-WARM-READ",
            "--relation",
            "implemented_by",
        ],
        0,
    );
    c.expect(&["relation", "list", "src/read.rs:3"], 0);
    c.expect(&["relation", "list", "src/read.rs"], 0);
    c.refused(&["relation", "list", "src/read.rs:99"], "unknown-node");
    c.expect(&["get", "src/read.rs:2"], 0);
    c.expect(&["get", "src/read.rs"], 0);
    c.refused(&["get", "src/none.rs"], "unknown-item");
    c.expect(&["query", "execute", "implemented"], 0);
    // A view over code links: the requirement that the code implements counts its code link,
    // and counts none when code is not read.
    let view = c.expect(
        &[
            "query",
            "view",
            "requirement-verification",
            "--input",
            "candidate=abc",
            "--input",
            "verification_result=@verified.json",
        ],
        0,
    );
    let row = (view["rows"].as_array().unwrap().iter())
        .find(|row| row["item"]["id"] == "REQ-COLD-READ-TIME")
        .unwrap_or_else(|| panic!("no row: {view}"));
    assert_eq!(row["values"][2], 1, "the code links: {view}");
    c.refused(&["item", "delete", "REQ-WARM-READ"], "targeted-item");
    c.write(
        "src/typo.rs",
        "// @mara implements REQ-COLD-READ-TIME\nfn typo() {}\n",
    );
    c.expect(
        &[
            "item",
            "update",
            "REQ-COLD-READ-TIME",
            "--title",
            "Cold read budget",
            "--body",
            "A cold read takes at most 200 ms at 16×.",
        ],
        0,
    );
    c.expect(&["get", "REQ-COLD-READ-TIME"], 0);
    c.expect(&["get", "docs/reqs.mara.md"], 0);
    c.expect(&["relation", "list", "NEED-FAST-ANSWERS"], 0);
    c.expect(&["search", "cold read", "--limit", "5"], 0);
    c.expect(
        &["item", "list", "--field", "status=draft", "--path", "docs"],
        0,
    );
    c.expect(&["item", "list", "--limit", "1"], 0);
    c.expect(
        &[
            "item",
            "move",
            "REQ-COLD-READ-TIME",
            "docs/perf/reqs.mara.md",
        ],
        0,
    );
    c.expect(
        &[
            "item",
            "move",
            "REQ-WARM-READ",
            "docs/reqs.mara.md",
            "--line",
            "1",
        ],
        0,
    );
    c.expect(
        &[
            "item",
            "rename",
            "REQ-COLD-READ-TIME",
            "REQ-COLD-READ-BUDGET",
        ],
        0,
    );
    // The rename rewrote the markers that named the old ID.
    assert_eq!(
        fs::read_to_string(c.root.join("src/typo.rs")).unwrap(),
        "// @mara implements REQ-COLD-READ-BUDGET\nfn typo() {}\n"
    );
    c.expect(&["relation", "list", "src/read.rs:1"], 0);
    c.refused(
        &["item", "rename", "REQ-COLD-READ-BUDGET", "NEED-WRONG"],
        "wrong-id-prefix",
    );

    // Hand edits: a document moved by hand, and an item written without an anchor whose
    // relation names its target by ID.
    fs::create_dir_all(c.root.join("docs/product")).unwrap();
    fs::rename(
        c.root.join("docs/needs.mara.md"),
        c.root.join("docs/product/needs.mara.md"),
    )
    .unwrap();
    c.write(
        "docs/hand.mara.md",
        "# Hand notes\n\n---\n\n## Hand written\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-HAND-WRITTEN |\n| Status | Draft |\n| Satisfies | NEED-FAST-ANSWERS |\n\n\
         Written without an anchor.\n",
    );
    assert_ne!(
        c.expect(&["project", "validate"], 1)["diagnostics"],
        json!([])
    );
    assert_ne!(c.expect(&["project", "fix"], 0)["repairs"], json!([]));
    c.expect(&["project", "validate"], 0);
    // An item validation, a gate and its report over code links, of a requirement that no
    // marker names: with every code file read, `not_implemented` has its row, so the invariant
    // warns and the gate fails on that condition besides `not_accepted`. An operation that read
    // no code would give no such row (`readiness.dl`), whatever its host.
    let validated = c.expect(&["item", "validate", "REQ-HAND-WRITTEN"], 0);
    let warned: Vec<&Value> = (validated["diagnostics"].as_array().unwrap().iter())
        .map(|diagnostic| &diagnostic["code"])
        .collect();
    assert_eq!(warned, [&json!(NOT_IMPLEMENTED)], "{validated}");
    let inputs = [
        "--input",
        "candidate=abc",
        "--input",
        "verification_result=@verified.json",
    ];
    let one = ["--id", "REQ-HAND-WRITTEN"];
    let gate = [&["query", "gate", "verified"][..], &one, &inputs].concat();
    let gated = c.expect(&gate, 1);
    let report = [&["query", "report", "verified"][..], &one, &inputs].concat();
    let reported = c.expect(&report, 0);
    for result in [&gated["items"][0], &reported] {
        let relations: Vec<&Value> = (result["rows"].as_array().unwrap().iter())
            .map(|row| &row["relation"])
            .collect();
        let expected = [json!("not_accepted"), json!("not_implemented")];
        assert_eq!(relations, expected.iter().collect::<Vec<_>>(), "{result}");
    }
    c.expect(&["relation", "list", "docs/hand.mara.md"], 0);

    // Requests that fail.
    c.refused(&["get", "REQ-NOPE"], "unknown-item");
    c.refused(&["get", "docs/none.mara.md:3"], "unknown-node");
    c.refused(&["schema", "get", "nothing"], "unknown-declaration");
    c.refused(&["item", "validate", "REQ-NOPE"], "unknown-item");
    c.refused(&["relation", "list", "REQ-NOPE"], "unknown-item");
    c.refused(&["item", "delete", "NEED-FAST-ANSWERS"], "targeted-item");
    c.rejected(&["item", "update", "REQ-WARM-READ"]);
    c.rejected_by_client(&["item", "list", "--limit", "0"]);
    c.rejected_by_client(&[
        "item",
        "move",
        "REQ-WARM-READ",
        "docs/reqs.mara.md",
        "--line",
        "0",
    ]);
    c.refused(
        &[
            "item",
            "create",
            "requirement",
            "notes/x.md",
            "--title",
            "Outside",
            "--field",
            "status=draft",
            "--body",
            "Text.",
        ],
        "not-an-input",
    );
    c.refused(
        &[
            "item",
            "move",
            "REQ-WARM-READ",
            "docs/perf/reqs.mara.md",
            "--line",
            "999",
        ],
        "invalid-position",
    );
    c.expect(
        &[
            "relation",
            "remove",
            "REQ-HAND-WRITTEN",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        0,
    );
    c.refused(
        &[
            "relation",
            "remove",
            "REQ-HAND-WRITTEN",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        "missing-relation",
    );
    c.expect(&["item", "delete", "REQ-HAND-WRITTEN"], 0);

    // The queries, written by hand into the template's configuration in place of its own
    // entries, with the inputs a file holds.
    let template = fs::read_to_string(c.root.join(".mara/config.toml")).unwrap();
    let kept = &template[..template.find("\n[rules]\n").expect("the template's rules") + 1];
    c.write(".mara/config.toml", &(kept.to_owned() + QUERY_ENTRIES));
    c.write(".mara/rules/gate.dl", GATE_PROGRAM);
    c.write(
        ".mara/rules/broken.dl",
        ".include \"mara.dl\"\n.decl bad(m: Mid)\nbad(m) :- missing(m).\n",
    );
    let gate = |extra: &[&'static str]| {
        let mut args = vec![
            "query",
            "gate",
            "passed",
            "--id",
            "NEED-FAST-ANSWERS",
            "--id",
            "REQ-COLD-READ-BUDGET",
            "--input",
            "candidate=abc",
            "--input",
            "result=@results.json",
        ];
        args.extend(extra);
        args
    };
    c.expect(&["query", "list"], 0);
    c.expect(&["query", "execute", "listed"], 0);
    c.expect(&["query", "view", "listed-view"], 0);
    c.expect(
        &[
            "query",
            "execute",
            "listed",
            "--id",
            "REQ-COLD-READ-BUDGET",
            "--limit",
            "1",
        ],
        0,
    );
    assert_eq!(c.expect(&gate(&[]), 1)["gate"]["outcome"], "unavailable");
    let failed = c.expect(&gate(&["--complete", "result"]), 1);
    assert_eq!(failed["gate"]["outcome"], "failed", "{failed}");
    c.refused(&["query", "execute", "nothing"], "unknown-entry");
    c.refused(&["query", "view", "listed"], "unknown-entry");
    c.refused(
        &["query", "execute", "listed", "--id", "REQ-NONE"],
        "unknown-item",
    );
    c.refused(
        &[
            "query",
            "gate",
            "passed",
            "--id",
            "NEED-FAST-ANSWERS",
            "--input",
            "candidate=abc",
        ],
        "missing-input",
    );
    c.refused(&gate(&["--input", "other=1"]), "unlisted-input");
    let mut wrong = gate(&[]);
    *wrong.last_mut().unwrap() = "result=@wrong.json";
    c.refused(&wrong, "invalid-input");
    c.refused(&["query", "execute", "broken"], "program-failed");
    let broken = c.expect(
        &["query", "gate", "broken-gate", "--id", "NEED-FAST-ANSWERS"],
        1,
    );
    assert_eq!(broken["gate"]["outcome"], "unavailable", "{broken}");
    c.rejected_by_mcp(&["query", "gate", "passed"]);
    let report = c.expect(
        &[
            "query",
            "report",
            "passed",
            "--id",
            "REQ-COLD-READ-BUDGET",
            "--input",
            "candidate=abc",
            "--input",
            "result=@results.json",
            "--complete",
            "result",
        ],
        0,
    );
    assert_eq!(report["report"]["outcome"], "failed", "{report}");
    c.expect(
        &[
            "query",
            "report",
            "broken-gate",
            "--id",
            "NEED-FAST-ANSWERS",
        ],
        1,
    );
    c.refused(
        &["query", "report", "listed", "--id", "REQ-COLD-READ-BUDGET"],
        "unknown-entry",
    );

    // A schema broken by hand: every operation but validation refuses the project.
    let schema = fs::read_to_string(c.root.join(".mara/schema.toml")).unwrap();
    c.write(".mara/schema.toml", "format_version = 1\nstatuses = [\n");
    c.refused(&["item", "list"], "invalid-project");
    c.refused(&["get", "REQ-WARM-READ"], "invalid-project");
    c.refused(&["project", "fix"], "invalid-project");
    c.refused(
        &[
            "relation",
            "add",
            "REQ-WARM-READ",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        "invalid-project",
    );
    c.refused(&["query", "execute", "listed"], "invalid-project");
    c.expect(&["project", "validate"], 1);
    c.expect(&["schema", "validate"], 1);
    c.write(".mara/schema.toml", &schema);
    c.finish();
}
