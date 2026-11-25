//! Each request through the CLI with `--format json` and through `mara mcp`, from the same files
//! at the same path: the same JSON and the same files (VER-CLI-MCP-PARITY).

use std::fs;

use serde_json::{Value, json};

use crate::{
    CliRun, Dir, Server, Usage, cli, engineering_project, mcp_request, no_work, pair_mids, usage,
};

/// A project, the MCP server started in it, and what turns a CLI request into its tool call.
struct Parity {
    dir: Dir,
    server: Server,
    usages: Vec<Usage>,
    tools: Vec<Value>,
}

impl Parity {
    fn new(dir: Dir) -> Parity {
        let mut server = Server::start(&dir.root(), &[]);
        let tools = server.tools();
        Parity {
            dir,
            server,
            usages: usage(),
            tools,
        }
    }

    /// Runs `args` through the CLI, restores the files, and calls its tool. After the MIDs that
    /// the MCP run generated are replaced by those the CLI run generated at the same path and
    /// line, the tool's text is what the CLI printed and the files are the same. Leaves the
    /// project as the CLI left it.
    fn request(&mut self, args: &[&str]) -> CliRun {
        let before = self.dir.files();
        let run = cli(&self.dir.root(), args);
        let cli_files = self.dir.files();
        self.dir.restore(&before);
        let (tool, arguments) = mcp_request(&self.usages, &self.tools, args, &self.dir.root());
        let call = self.server.call(&tool, arguments);
        let (text, mcp_files) = pair_mids(args, &before, &cli_files, &call.text, self.dir.files());
        assert_eq!(run.stdout, format!("{text}\n"), "{args:?}");
        assert_eq!(
            call.is_error,
            run.json().get("error").is_some(),
            "{args:?}: {call:?}"
        );
        if mcp_files != cli_files {
            let differ: Vec<&String> = (cli_files.keys().chain(mcp_files.keys()))
                .filter(|path| cli_files.get(*path) != mcp_files.get(*path))
                .collect();
            panic!("{args:?}: the files differ: {differ:?}");
        }
        self.dir.restore(&cli_files);
        run
    }

    /// [`Parity::request`] with exit status `status` on the CLI.
    fn expect(&mut self, args: &[&str], status: i32) -> Value {
        let run = self.request(args);
        assert_eq!(run.status, status, "{args:?}: {run:#?}");
        run.json()
    }

    /// A failure with `code`.
    fn refused(&mut self, args: &[&str], code: &str) {
        let json = self.expect(args, 1);
        assert_eq!(json["error"]["code"], code, "{args:?}: {json}");
    }

    /// A required option left out (DES-MCP-SERVER): the CLI's operation rejects it with exit
    /// status 2, and `mara mcp` before the operation runs, in a message that names the tool's
    /// parameter, `missing`. Neither changes a file.
    fn missing(&mut self, args: &[&str], missing: &str) {
        let before = self.dir.files();
        let run = cli(&self.dir.root(), args);
        assert_eq!(run.status, 2, "{args:?}: {run:#?}");
        assert_eq!(run.json()["error"]["code"], "invalid-argument", "{args:?}");
        assert_eq!(self.dir.files(), before, "{args:?}");
        let (tool, arguments) = mcp_request(&self.usages, &self.tools, args, &self.dir.root());
        let call = self.server.call(&tool, arguments);
        assert!(call.is_error, "{args:?}: {call:?}");
        assert_eq!(
            call.json()["error"],
            json!({
                "code": "invalid-argument",
                "message": format!("`{tool}` needs `{missing}`"),
                "diagnostics": [],
            }),
            "{args:?}"
        );
        assert_eq!(self.dir.files(), before, "{args:?}");
    }

    /// Arguments that the operation rejects: exit status 2 on the CLI.
    fn rejected(&mut self, args: &[&str]) {
        let json = self.expect(args, 2);
        assert_eq!(
            json["error"]["code"], "invalid-argument",
            "{args:?}: {json}"
        );
    }

    fn finish(self) {
        let (status, _) = self.server.finish();
        assert!(status.success());
    }
}

/// The real workflow: an agent initializes a project, creates, relates, updates, reads, moves
/// and renames items, repairs hand edits with fix and validates clean, and meets failures.
#[test]
fn an_agent_works_through_mcp_as_through_the_cli() {
    let mut p = Parity::new(Dir::new());
    p.expect(&["project", "init"], 0);
    p.expect(&["schema", "list"], 0);
    p.expect(&["schema", "get", "requirement"], 0);
    p.expect(&["schema", "validate"], 0);
    let need = [
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
    assert_eq!(p.expect(&need, 0)["item"]["id"], "NEED-FAST-ANSWERS");
    // A document begun by hand with a heading and a sentence.
    p.dir.write(
        "docs/reqs.mara.md",
        "# Requirements\n\nPerformance obligations.\n",
    );
    p.expect(
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
    p.expect(
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
    p.expect(
        &[
            "relation",
            "add",
            "REQ-COLD-READ-TIME",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        0,
    );
    p.refused(
        &[
            "relation",
            "add",
            "NEED-FAST-ANSWERS",
            "satisfied_by",
            "REQ-WARM-READ",
        ],
        "inverse-relation",
    );
    p.expect(
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
    p.expect(&["get", "REQ-COLD-READ-TIME"], 0);
    p.expect(&["get", "docs/reqs.mara.md"], 0);
    p.expect(&["relation", "list", "NEED-FAST-ANSWERS"], 0);
    p.expect(&["search", "cold read", "--limit", "5"], 0);
    p.expect(
        &["search", "", "--type", "need", "--type", "requirement"],
        0,
    );
    p.expect(
        &["item", "list", "--field", "status=draft", "--path", "docs"],
        0,
    );
    p.expect(&["item", "list", "--limit", "1"], 0);
    p.expect(
        &[
            "item",
            "move",
            "REQ-COLD-READ-TIME",
            "docs/perf/reqs.mara.md",
        ],
        0,
    );
    p.expect(
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
    p.expect(
        &[
            "item",
            "rename",
            "REQ-COLD-READ-TIME",
            "REQ-COLD-READ-BUDGET",
        ],
        0,
    );
    p.refused(
        &["item", "rename", "REQ-COLD-READ-BUDGET", "NEED-WRONG"],
        "wrong-id-prefix",
    );

    // Hand edits: a document moved by hand, and an item written without an anchor whose
    // relation names its target by ID.
    let root = p.dir.root();
    fs::create_dir_all(root.join("docs/product")).unwrap();
    fs::rename(
        root.join("docs/needs.mara.md"),
        root.join("docs/product/needs.mara.md"),
    )
    .unwrap();
    p.dir.write(
        "docs/hand.mara.md",
        "# Hand notes\n\n---\n\n## Hand written\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-HAND-WRITTEN |\n| Status | Draft |\n| Satisfies | NEED-FAST-ANSWERS |\n\n\
         Written without an anchor.\n",
    );
    let invalid = p.expect(&["project", "validate"], 1);
    assert_ne!(invalid["diagnostics"], json!([]));
    let fixed = p.expect(&["project", "fix"], 0);
    assert_ne!(fixed["repairs"], json!([]));
    assert_eq!(
        p.expect(&["project", "validate"], 0)["diagnostics"],
        json!([])
    );
    p.expect(&["item", "validate", "REQ-HAND-WRITTEN"], 0);
    p.expect(&["relation", "list", "docs/hand.mara.md"], 0);

    // Requests that fail.
    p.refused(&["get", "REQ-NOPE"], "unknown-item");
    p.refused(&["get", "docs/none.mara.md:3"], "unknown-node");
    p.refused(&["schema", "get", "nothing"], "unknown-declaration");
    p.refused(&["item", "delete", "NEED-FAST-ANSWERS"], "targeted-item");
    p.missing(
        &[
            "item",
            "create",
            "requirement",
            "docs/x.mara.md",
            "--title",
            "No body",
            "--field",
            "status=draft",
        ],
        "body",
    );
    p.rejected(&["item", "update", "REQ-WARM-READ"]);
    p.refused(
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
    p.refused(
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
    p.expect(
        &[
            "relation",
            "remove",
            "REQ-HAND-WRITTEN",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        0,
    );
    p.refused(
        &[
            "relation",
            "remove",
            "REQ-HAND-WRITTEN",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        "missing-relation",
    );
    p.expect(&["item", "delete", "REQ-HAND-WRITTEN"], 0);
    p.refused(&["project", "convert"], "not-a-legacy-project");
    p.refused(&["project", "init"], "existing-file");
    assert_eq!(
        p.expect(&["project", "validate"], 0)["diagnostics"],
        json!([])
    );

    // A schema broken by hand: every operation but validation refuses the project.
    p.dir
        .write(".mara/schema.toml", "format_version = 1\nstatuses = [\n");
    p.refused(&["item", "list"], "invalid-project");
    p.refused(&["project", "fix"], "invalid-project");
    p.refused(
        &[
            "relation",
            "add",
            "REQ-WARM-READ",
            "satisfies",
            "NEED-FAST-ANSWERS",
        ],
        "invalid-project",
    );
    p.expect(&["project", "validate"], 1);
    p.expect(&["schema", "validate"], 1);
    p.finish();
}

/// A 0.3.0 project: refused by every operation but the conversion, then converted.
#[test]
fn a_conversion_through_mcp_as_through_the_cli() {
    let dir = Dir::new();
    dir.write(
        ".mara/project.toml",
        "format_version = 3\n\n[project]\nname = \"legacy\"\nschema = \".mara/schema.yaml\"\n\n\
         [content]\ninclude = [\"docs/**/*.mara.md\"]\n",
    );
    dir.write(
        ".mara/schema.yaml",
        "format_version: 3\nflavours:\n  requirement:\n    description: An obligation.\n    \
         use_when: [x]\n    avoid_when: []\n    distinguish_from: {}\n    id_prefix: REQ-\n    \
         body: required\n    fields:\n      status: {type: enum, values: [draft]}\nrelations:\n  \
         depends_on:\n    description: Needs.\n    source: [requirement]\n    target: \
         [requirement]\n    inverse: required_by\n",
    );
    dir.write(
        "docs/a.mara.md",
        "# A\n\n:::mara requirement REQ-A\n:mid: 01M7FX0E000000000000000001\n:title: A\n\
         :depends_on: REQ-B\n\nNames [[REQ-B]].\n:::\n",
    );
    dir.write(
        "docs/sub/b.mara.md",
        "# B\n\n:::mara requirement REQ-B\n:mid: 01M7FX0E000000000000000002\n:title: B\n\
         :status: draft\n\nBody.\n:::\n",
    );
    let mut p = Parity::new(dir);
    p.refused(&["item", "list"], "legacy-project");
    p.rejected(&["project", "convert"]);
    p.expect(&["project", "convert", "--status", "draft"], 0);
    p.expect(&["item", "list"], 0);
    p.expect(&["project", "validate"], 0);
    p.finish();
}

/// A server given the project on each call runs each call against that project, and one
/// started with `--project` runs every call against it and takes no other.
#[test]
fn the_project_of_each_call_or_of_the_server() {
    let a = engineering_project();
    let b = Dir::new();
    assert_eq!(
        cli(&b.root(), &["project", "init", "--template", "empty"]).status,
        0
    );
    let elsewhere = Dir::new();
    let usages = usage();
    let relative = format!(
        "../{}",
        a.root()
            .file_name()
            .unwrap()
            .to_str()
            .expect("a UTF-8 directory name")
    );
    let a_path = a.root().to_str().unwrap().to_owned();
    let b_path = b.root().to_str().unwrap().to_owned();
    let mut server = Server::start(&elsewhere.root(), &[]);
    let tools = server.tools();
    for project in [a_path.as_str(), relative.as_str(), b_path.as_str()] {
        for args in [
            &["item", "list"][..],
            &["schema", "list"],
            &["project", "validate"],
        ] {
            let mut with = vec!["--project", project];
            with.extend(args);
            let run = cli(&elsewhere.root(), &with);
            assert_eq!(run.status, 0, "{with:?}: {run:#?}");
            let (tool, arguments) = mcp_request(&usages, &tools, args, &elsewhere.root());
            let mut arguments = arguments;
            arguments["project"] = json!(project);
            let call = server.call(&tool, arguments);
            assert_eq!(run.stdout, format!("{}\n", call.text), "{with:?}");
        }
    }
    // Without a project, the call finds none from the server's working directory, as the CLI.
    let run = cli(&elsewhere.root(), &["item", "list"]);
    assert_eq!(run.status, 1);
    let call = server.call("item_list", json!({}));
    assert_eq!(run.stdout, format!("{}\n", call.text));
    assert!(call.is_error);
    assert!(server.finish().0.success());

    let mut bound = Server::start(&elsewhere.root(), &["--project", &relative]);
    let run = cli(&elsewhere.root(), &["--project", &relative, "item", "list"]);
    let call = bound.call("item_list", json!({}));
    assert_eq!(run.stdout, format!("{}\n", call.text));
    assert_eq!(call.json()["items"].as_array().unwrap().len(), 2);
    let before = (a.files(), b.files());
    let create = json!({
        "type": "requirement", "path": "docs/reqs.mara.md", "title": "Elsewhere",
        "field": ["status=draft"], "body": "Text.", "project": b_path,
    });
    let call = bound.call("item_create", create);
    assert!(call.is_error);
    assert_eq!(call.json()["error"]["code"], "invalid-argument");
    assert_eq!((a.files(), b.files()), before);
    let call = bound.call(
        "item_create",
        json!({"type": "need", "path": "docs/more.mara.md", "title": "More", "field": ["status=draft"], "body": "Text."}),
    );
    assert!(!call.is_error, "{call:?}");
    assert!(a.read("docs/more.mara.md").contains("## More"));
    assert_eq!(b.files(), before.1);
    assert!(bound.finish().0.success());
}

/// An undeclared, missing or malformed argument is rejected with `invalid-argument` before
/// anything runs: no work, no file changed. Each MCP case runs in its own session, whose stats
/// line is that call's.
#[test]
fn undeclared_or_malformed_arguments_are_rejected_before_anything_runs() {
    let dir = engineering_project();
    let before = dir.files();
    for args in [
        &["item", "list", "--no-such-option", "x"][..],
        &[
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--colour",
            "red",
        ],
        &["item", "validate"],
        &["item", "list", "--limit", "0"],
    ] {
        let run = cli(&dir.root(), args);
        assert_eq!(run.status, 2, "{args:?}: {run:#?}");
        assert_eq!(run.json()["error"]["code"], "invalid-argument");
        assert_eq!(run.stats, no_work(), "{args:?}");
        assert_eq!(dir.files(), before, "{args:?}");
    }
    let create = |extra: Value| {
        let mut arguments = json!({
            "type": "requirement", "path": "docs/reqs.mara.md", "title": "Rejected",
            "field": ["status=draft"], "body": "Text.",
        });
        for (key, value) in extra.as_object().unwrap() {
            arguments[key] = value.clone();
        }
        arguments
    };
    for (tool, arguments) in [
        ("item_list", json!({"no-such-option": "x"})),
        ("item_create", create(json!({"colour": "red"}))),
        ("item_create", create(json!({"line": "1"}))),
        ("item_create", create(json!({"line": 1.5}))),
        ("item_create", create(json!({"line": 0}))),
        ("item_create", create(json!({"field": "status=draft"}))),
        ("item_create", create(json!({"field": [1]}))),
        ("item_create", create(json!({"field": ["type=need"]}))),
        ("item_create", create(json!({"title": 5}))),
        ("item_validate", json!({})),
        (
            "item_create",
            json!({"type": "requirement", "path": "docs/reqs.mara.md", "body": "Text."}),
        ),
        ("query_gate", json!({"name": "verified", "id": []})),
        ("query_report", json!({"name": "verified"})),
        ("item_list", json!({"limit": 0})),
        ("item_list", json!({"cursor": "x"})),
        ("item_list", json!({"type": "need"})),
        ("item_list", json!({"path": ["/etc"]})),
    ] {
        let mut server = Server::start(&dir.root(), &[]);
        let call = server.call(tool, arguments.clone());
        assert!(call.is_error, "{tool} {arguments}: {call:?}");
        assert_eq!(
            call.json()["error"]["code"],
            "invalid-argument",
            "{tool} {arguments}"
        );
        let (status, stats) = server.finish();
        assert!(status.success());
        assert_eq!(stats, no_work(), "{tool} {arguments}");
        assert_eq!(dir.files(), before, "{tool} {arguments}");
    }
    // Every tool refuses an undeclared parameter.
    let mut server = Server::start(&dir.root(), &[]);
    for tool in server.tools() {
        let name = tool["name"].as_str().unwrap();
        let call = server.call(name, json!({"no-such-parameter": true}));
        assert_eq!(call.json()["error"]["code"], "invalid-argument", "{name}");
    }
    let (_, stats) = server.finish();
    assert_eq!(stats, no_work());
    assert_eq!(dir.files(), before);
    // A null stands for a parameter left out.
    let mut server = Server::start(&dir.root(), &[]);
    let call = server.call(
        "item_list",
        json!({"limit": null, "cursor": null, "type": null}),
    );
    assert!(!call.is_error, "{call:?}");
    assert_eq!(call.json()["items"].as_array().unwrap().len(), 2);
    assert!(server.finish().0.success());
}

/// The program of [`the_query_operations_through_mcp_as_through_the_cli`]: every item is a
/// subject; an item fails with a `fail` result, or without one when the results are complete,
/// and is unavailable without one otherwise.
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

/// The `query` operations, failing requests included, with the inputs a file holds on the CLI
/// and passed as rows through MCP (VER-CLI-MCP-PARITY, VER-REQUEST-INPUTS).
#[test]
fn the_query_operations_through_mcp_as_through_the_cli() {
    let dir = engineering_project();
    // The template's configuration without its own entries, then these.
    let template = dir.read(".mara/config.toml");
    let example = &template[..template.find("\n[rules]\n").expect("the template's rules") + 1];
    dir.write(".mara/config.toml", &(example.to_owned() + QUERY_ENTRIES));
    dir.write(".mara/rules/gate.dl", GATE_PROGRAM);
    dir.write(
        ".mara/rules/broken.dl",
        ".include \"mara.dl\"\n.decl bad(m: Mid)\nbad(m) :- missing(m).\n",
    );
    let need = cli(&dir.root(), &["get", "NEED-FAST-ANSWERS"]).json()["node"]["mid"].clone();
    let results = json!([[need, "pass", -2147483648_i64]]);
    dir.write("results.json", &results.to_string());
    dir.write("wrong.json", &json!([[need, "pass", 1.5]]).to_string());
    let mut p = Parity::new(dir);
    let gate = |extra: &[&'static str]| {
        let mut args = vec![
            "query",
            "gate",
            "passed",
            "--id",
            "NEED-FAST-ANSWERS",
            "--id",
            "REQ-COLD-READ-TIME",
            "--input",
            "candidate=abc",
            "--input",
            "result=@results.json",
        ];
        args.extend(extra);
        args
    };

    let list = p.expect(&["query", "list"], 0);
    let names: Vec<&str> = (list["entries"].as_array().unwrap().iter())
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    // By table, then by name.
    assert_eq!(
        names,
        ["broken", "listed", "listed-view", "broken-gate", "passed"]
    );
    let rows = p.expect(&["query", "execute", "listed"], 0)["rows"].clone();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(p.expect(&["query", "view", "listed-view"], 0)["rows"], rows);
    p.expect(
        &[
            "query",
            "execute",
            "listed",
            "--id",
            "REQ-COLD-READ-TIME",
            "--limit",
            "1",
        ],
        0,
    );
    // The requirement has no result: unavailable, then failed once the results are complete.
    let unavailable = p.expect(&gate(&[]), 1);
    assert_eq!(
        unavailable["gate"]["outcome"], "unavailable",
        "{unavailable}"
    );
    let failed = p.expect(&gate(&["--complete", "result"]), 1);
    assert_eq!(failed["gate"]["outcome"], "failed", "{failed}");
    let outcomes: Vec<&Value> = (failed["items"].as_array().unwrap().iter())
        .map(|item| &item["outcome"])
        .collect();
    assert_eq!(outcomes, [&json!("passed"), &json!("failed")]);

    p.refused(&["query", "execute", "nothing"], "unknown-entry");
    p.refused(&["query", "execute", "listed-view"], "unknown-entry");
    p.refused(
        &["query", "execute", "listed", "--id", "REQ-NONE"],
        "unknown-item",
    );
    p.refused(
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
    p.refused(&gate(&["--input", "other=1"]), "unlisted-input");
    p.refused(&gate(&["--complete", "other"]), "unlisted-input");
    let mut wrong = gate(&[]);
    *wrong.last_mut().unwrap() = "result=@wrong.json";
    p.refused(&wrong, "invalid-input");
    p.refused(&["query", "execute", "broken"], "program-failed");
    let broken = p.expect(
        &["query", "gate", "broken-gate", "--id", "NEED-FAST-ANSWERS"],
        1,
    );
    assert_eq!(broken["gate"]["outcome"], "unavailable", "{broken}");
    assert_eq!(
        broken["diagnostics"][0]["code"], "program-error",
        "{broken}"
    );
    p.missing(&["query", "gate", "passed"], "id");

    // `query report` explains the failed item's row; `--complete` gave the leaf of
    // `source_complete`, the candidate the leaf of `candidate`.
    let report = p.expect(
        &[
            "query",
            "report",
            "passed",
            "--id",
            "REQ-COLD-READ-TIME",
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
    let proof = report["rows"][0]["proof"].to_string();
    for leaf in [
        json!({"input": "source_complete", "values": ["result"]}),
        json!({"input": "candidate", "values": ["abc"]}),
    ] {
        assert!(proof.contains(&leaf.to_string()), "{leaf} in {proof}");
    }
    let broken = p.expect(
        &[
            "query",
            "report",
            "broken-gate",
            "--id",
            "NEED-FAST-ANSWERS",
        ],
        1,
    );
    assert_eq!(broken["report"]["outcome"], "unavailable", "{broken}");
    assert_eq!(broken["rows"], json!([]), "{broken}");
    p.refused(
        &["query", "report", "listed", "--id", "REQ-COLD-READ-TIME"],
        "unknown-entry",
    );
    p.missing(&["query", "report", "passed"], "id");
    p.finish();
}
