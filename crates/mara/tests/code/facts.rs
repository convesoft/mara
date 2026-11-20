//! VER-CODE-FACTS at the basic level: a program copies the code-link relation of DES-FACT-BRIDGE
//! and `source_complete` into relations headed by an item's MID, read through `query execute`
//! and compared with what `relation list` gives. The grammar-level run waits for mara-s9gak4.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::{Value, json};

use crate::{CODE, Project, mid};

/// Uses code links: `r_link` copies `code_link`, `r_complete` copies `source_complete` for each
/// item.
const LINKS: &str = r#".include "mara.dl"
.decl r_link(dst: Mid, rel: Rel, path: Path, line: number, marker_line: number, declaration: symbol)
r_link(d, r, p, l, m, n) :- code_link(r, d, p, l, m, n).
.decl r_complete(mid: Mid, source: symbol)
r_complete(m, s) :- item(m, _, _, _, _, _), source_complete(s).
"#;

/// Names no code link: the items' IDs, and `source_complete` for each item.
const PLAIN: &str = r#".include "mara.dl"
.decl p_item(mid: Mid, id: Id)
p_item(m, i) :- item(m, _, i, _, _, _).
.decl p_complete(mid: Mid, source: symbol)
p_complete(m, s) :- item(m, _, _, _, _, _), source_complete(s).
"#;

/// Rows, each value as its JSON text.
type Rows = BTreeSet<Vec<String>>;

/// `values` as a row of [`Rows`].
fn row(values: &[Value]) -> Vec<String> {
    values.iter().map(Value::to_string).collect()
}

const QUERIES: [(&str, &str); 4] = [
    ("r_link", "links.dl"),
    ("r_complete", "links.dl"),
    ("p_item", "plain.dl"),
    ("p_complete", "plain.dl"),
];

/// Markers for REQ-LOGIN in two languages, one for NEED-ACCESS whose relation does not lead
/// from code to a Need, and one each that is unresolved and invalid.
const MARKERS: [(&str, &str); 2] = [
    (
        "src/login.rs",
        "fn login() {}\n// @mara implements REQ-LOGIN\n\nfn access() {} // @mara implements NEED-ACCESS\n\
         // @mara implements REQ-NONE-SUCH\n// @mara implements\n",
    ),
    (
        "scripts/audit.py",
        "#!/usr/bin/env python3\n# @mara implements REQ-LOGIN\ndef audit():\n    pass\n",
    ),
];

/// The project of VER-CODE-MARKERS with [`LINKS`] and [`PLAIN`] declared, a query on each of
/// their relations, and [`MARKERS`].
fn project() -> Project {
    let project = Project::new();
    project.write(".mara/rules/links.dl", LINKS);
    project.write(".mara/rules/plain.dl", PLAIN);
    let mut config = project.read(".mara/config.toml");
    for (relation, file) in QUERIES {
        config.push_str(&format!(
            "\n[queries.{relation}]\nprogram = \".mara/rules/{file}\"\n\
             relation = \"{relation}\"\ndescription = \"The rows of {relation}\"\n"
        ));
    }
    project.write(".mara/config.toml", config);
    for (path, contents) in MARKERS {
        project.write(path, contents);
    }
    project
}

/// The rows of `query execute <relation>`, sorted, and the request's work counts.
fn rows(project: &Project, relation: &str) -> (Rows, Value) {
    let args = ["--debug-stats", "--format", "json", "query", "execute"];
    let args = [&args[..], &[relation, "--limit", "100"]].concat();
    let output = project.run(&args, 0);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["next_cursor"], Value::Null, "{result}");
    let rows = result["rows"].as_array().unwrap().iter();
    let rows = rows.map(|values| row(values["values"].as_array().unwrap()));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stats = serde_json::from_str(stderr.lines().last().unwrap()).unwrap();
    (rows.collect(), stats)
}

/// The `r_link` rows that `relation list` of each of `ids` gives: one per incoming code link,
/// with its relation, location, source line and declaration name.
fn listed(project: &Project, ids: &[&str]) -> Rows {
    let mut rows = BTreeSet::new();
    for id in ids {
        let result = project.json(&["relation", "list", id, "--limit", "100"], 0);
        let item = result["node"]["mid"].clone();
        for connection in result["connections"].as_array().unwrap() {
            let neighbour = &connection["neighbour"];
            if neighbour["kind"] != "code" {
                continue;
            }
            assert_eq!(connection["relation"], "implemented_by", "{connection}");
            let declaration = neighbour["declaration"].as_str().unwrap_or_default();
            rows.insert(row(&[
                item.clone(),
                json!("implements"),
                neighbour["path"].clone(),
                neighbour["line"].clone(),
                connection["source"]["line"].clone(),
                json!(declaration),
            ]));
        }
    }
    rows
}

/// A `(MID, source)` row for each of the four items.
fn complete(source: &str) -> Rows {
    (1..=4)
        .map(|n| row(&[json!(mid(n)), json!(source)]))
        .collect()
}

#[test]
fn each_code_link_is_a_row_with_what_relation_list_gives() {
    let project = project();
    let (links, _) = rows(&project, "r_link");
    let login = |path: &str, line: u64| {
        row(&[
            json!(mid(1)),
            json!("implements"),
            json!(path),
            json!(line),
            json!(line),
            json!(""),
        ])
    };
    let need = row(&[
        json!(mid(4)),
        json!("implements"),
        json!("src/login.rs"),
        json!(4),
        json!(4),
        json!(""),
    ]);
    // Three markers give rows, the disallowed one included; the unresolved and the invalid
    // marker give none.
    let expected: BTreeSet<_> =
        [login("scripts/audit.py", 2), login("src/login.rs", 2), need].into();
    assert_eq!(links, expected);
    assert_eq!(
        links,
        listed(&project, &["REQ-LOGIN", "NEED-ACCESS", "REQ-AUDIT"])
    );
    let problems: Vec<String> = project.problems().into_iter().map(|p| p.0).collect();
    for code in ["disallowed-target", "unresolved-marker", "invalid-marker"] {
        assert!(problems.iter().any(|p| p == code), "{code}: {problems:?}");
    }
}

#[test]
fn code_is_complete_when_every_code_file_was_read() {
    let project = project();
    assert_eq!(rows(&project, "r_complete").0, complete("code"));

    // A project that declares no code language has read all of its code, and has no link.
    let config = project.read(".mara/config.toml").replace(CODE, "");
    project.write(".mara/config.toml", config);
    assert_eq!(rows(&project, "r_complete").0, complete("code"));
    assert_eq!(rows(&project, "r_link").0, BTreeSet::new());
}

#[test]
fn a_code_file_that_cannot_be_read_leaves_code_incomplete() {
    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "this case needs a user without CAP_DAC_OVERRIDE"
    );
    let project = project();
    let readable = rows(&project, "r_link").0;
    let unreadable = project.path("scripts/audit.py");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let (complete_rows, _) = rows(&project, "r_complete");
    let (links, _) = rows(&project, "r_link");
    let result = project.json(&["query", "execute", "r_link"], 0);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(complete_rows, BTreeSet::new());
    let without: BTreeSet<_> = readable
        .into_iter()
        .filter(|row| row[2] != "\"scripts/audit.py\"")
        .collect();
    assert_eq!(links, without);
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "io-error" && d["path"] == "scripts/audit.py"),
        "{result}"
    );
}

#[test]
fn a_program_without_code_links_reads_no_code() {
    let project = project();
    let (items, with_code) = rows(&project, "p_item");
    let (complete_rows, _) = rows(&project, "p_complete");
    // No code is read, so code is not complete either.
    assert_eq!(complete_rows, BTreeSet::new());
    assert_eq!(with_code["code_scans"], 0, "{with_code}");
    // The counts of the same request with no code file.
    let (_, links_stats) = rows(&project, "r_link");
    assert_eq!(links_stats["code_scans"], 2, "{links_stats}");
    for (path, _) in MARKERS {
        fs::remove_file(project.path(path)).unwrap();
    }
    let (without_items, without_code) = rows(&project, "p_item");
    assert_eq!(items, without_items);
    assert_eq!(with_code, without_code);
    // `project validate` still scans code and reports the markers' problems.
    for (path, contents) in MARKERS {
        project.write(path, contents);
    }
    let problems: Vec<String> = project.problems().into_iter().map(|p| p.0).collect();
    assert!(
        problems.contains(&"invalid-marker".to_owned()),
        "{problems:?}"
    );
}

/// A proof leaf that is a code link is placed at its code file and the marker's line
/// (DES-RULE-EXECUTION).
#[test]
fn query_report_places_a_code_link_leaf_at_its_marker() {
    let project = project();
    let leaf = code_link_leaf(&project);
    assert_eq!(
        (&leaf["path"], &leaf["line"], &leaf["values"]),
        (
            &json!("src/login.rs"),
            &json!(4),
            &json!(["implements", mid(4), "src/login.rs", 4, 4, ""])
        ),
    );
}

/// The `code_link` leaf of the proof of `query report code-on-need --id NEED-ACCESS`, an
/// invariant added to `project` that fails for each Need a marker names.
fn code_link_leaf(project: &Project) -> Value {
    need_invariant(project);
    let report = project.json(
        &["query", "report", "code-on-need", "--id", "NEED-ACCESS"],
        0,
    );
    assert_eq!(report["report"]["outcome"], "failed", "{report}");
    let proof = &report["rows"][0]["proof"];
    let children = proof["children"].as_array().unwrap();
    let leaf = children
        .iter()
        .find(|child| child["relation"] == "code_link");
    leaf.unwrap_or_else(|| panic!("no code_link leaf: {report}"))
        .clone()
}

/// Adds the error invariant `code-on-need` to `project`: a row for each Need a marker names.
fn need_invariant(project: &Project) {
    project.write(
        ".mara/rules/needs.dl",
        ".include \"mara.dl\"\n.decl code_on_need(need: Mid, path: Path, line: number)\n\
         code_on_need(d, p, l) :- code_link(_, d, p, l, _, _), item(d, \"need\", _, _, _, _).\n",
    );
    let config = project.read(".mara/config.toml");
    project.write(
        ".mara/config.toml",
        format!(
            "{config}\n[invariants.code-on-need]\nprogram = \".mara/rules/needs.dl\"\n\
             relation = \"code_on_need\"\nseverity = \"error\"\n\
             message = \"A marker names a need\"\n"
        ),
    );
}

/// A Rust file for the grammar level: a marker at the top owned by the file, one before a
/// function, one after code inside it naming a Need, and a marker-like line in a raw string.
const GRAMMAR_RUST: &str = "//! Signing in.\n// @mara implements REQ-AUDIT\n\nuse std::fmt;\n\n\
                            // @mara implements REQ-LOGIN\n/// Signs a user in.\npub fn login() {\n    \
                            check(); // @mara implements NEED-ACCESS\n    let raw = r#\"\n\
                            // @mara implements REQ-LOGIN\n\"#;\n}\n";

/// [`project`] with only [`GRAMMAR_RUST`] as code and the Rust fixture grammar installed and
/// pinned; returns the grammar's cache directory and its pin's line too.
fn grammar_project() -> (Project, std::path::PathBuf, u64) {
    let mut project = project();
    for (path, _) in MARKERS {
        fs::remove_file(project.path(path)).unwrap();
    }
    project.write("src/login.rs", GRAMMAR_RUST);
    let (cache, line) = project.install_grammar("rust");
    (project, cache, line)
}

/// A `code_link` row of `src/login.rs`, as [`Rows`] holds it.
fn grammar_row(item: u32, line: u64, marker: u64, declaration: &str) -> Vec<String> {
    let values = [
        json!(mid(item)),
        json!("implements"),
        json!("src/login.rs"),
        json!(line),
        json!(marker),
        json!(declaration),
    ];
    row(&values)
}

#[test]
fn at_the_grammar_level_a_row_holds_the_owning_declaration() {
    let (project, ..) = grammar_project();
    let (links, stats) = rows(&project, "r_link");
    // The marker-like line in the raw string gives no row; the marker after code inside
    // `login` is owned by it, its relation's target disallowed or not.
    let expected: Rows = [
        grammar_row(2, 1, 2, ""),
        grammar_row(1, 8, 6, "login"),
        grammar_row(4, 8, 9, "login"),
    ]
    .into();
    assert_eq!(links, expected);
    assert_eq!(
        links,
        listed(&project, &["REQ-LOGIN", "REQ-AUDIT", "NEED-ACCESS"])
    );
    assert_eq!(stats["code_scans"], 1, "{stats}");
    assert_eq!(rows(&project, "r_complete").0, complete("code"));
}

#[test]
fn a_grammar_that_falls_back_gives_basic_rows_and_code_stays_complete() {
    let (project, cache, line) = grammar_project();
    fs::remove_dir_all(&cache).unwrap();
    let (links, _) = rows(&project, "r_link");
    let expected: Rows = [
        grammar_row(2, 2, 2, ""),
        grammar_row(1, 6, 6, ""),
        grammar_row(4, 9, 9, ""),
        grammar_row(1, 11, 11, ""),
    ]
    .into();
    assert_eq!(links, expected);
    assert_eq!(
        links,
        listed(&project, &["REQ-LOGIN", "REQ-AUDIT", "NEED-ACCESS"])
    );
    // Every code file was read and scanned, at the basic level.
    assert_eq!(rows(&project, "r_complete").0, complete("code"));
    // `query execute` and `query report` say why.
    need_invariant(&project);
    let executed = project.json(&["query", "execute", "r_link"], 0);
    let args = ["query", "report", "code-on-need", "--id", "NEED-ACCESS"];
    let reported = project.json(&args, 0);
    for result in [executed, reported] {
        let unavailable: Vec<(&Value, &Value)> = (result["diagnostics"].as_array().unwrap())
            .iter()
            .filter(|d| d["code"] == "unavailable-grammar")
            .map(|d| (&d["path"], &d["line"]))
            .collect();
        assert_eq!(
            unavailable,
            [(&json!(".mara/config.toml"), &json!(line))],
            "{result}"
        );
    }
}

/// At the grammar level too, a `code_link` proof leaf shows the code file and the marker's
/// line, not the declaration's (DES-RULE-EXECUTION).
#[test]
fn at_the_grammar_level_a_code_link_leaf_is_at_the_marker() {
    let (project, ..) = grammar_project();
    let leaf = code_link_leaf(&project);
    assert_eq!(
        (&leaf["path"], &leaf["line"], &leaf["values"]),
        (
            &json!("src/login.rs"),
            &json!(9),
            &json!(["implements", mid(4), "src/login.rs", 8, 9, "login"])
        ),
    );
}

/// A program without code links loads no grammar; one with code links loads it once in its
/// process, and each request scans each code file once.
#[test]
fn a_process_loads_a_grammar_once_and_only_for_code_links() {
    let (project, ..) = grammar_project();
    let tmp = std::env::temp_dir();
    let mut mcp = project.mcp_with(&[("TMPDIR", tmp.as_path())]);
    let plain = mcp.call("query_execute", json!({"name": "p_item"}));
    assert!(plain["rows"].is_array(), "{plain}");
    assert_eq!(grammar_threads(mcp.pid()), Vec::<String>::new());
    let mut threads = Vec::new();
    for _ in 0..2 {
        let links = mcp.call("query_execute", json!({"name": "r_link"}));
        assert_eq!(links["rows"].as_array().unwrap().len(), 3, "{links}");
        threads.push(grammar_threads(mcp.pid()));
    }
    assert_eq!(threads[0].len(), 1, "{threads:?}");
    assert_eq!(threads[1], threads[0]);
    drop(mcp);
    // One request: one scan of the one code file, the plain program none.
    assert_eq!(rows(&project, "r_link").1["code_scans"], 1);
    assert_eq!(rows(&project, "p_item").1["code_scans"], 0);
}

/// The threads of the process `pid` that run a grammar.
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

/// A grammar dropped during the request, at a file after `src/login.rs`, leaves every Rust
/// file at the basic level for that request: the rows agree with `relation list` in the same
/// conditions, and code is complete.
#[test]
fn a_grammar_dropped_during_the_request_gives_basic_rows_for_every_file() {
    let (project, ..) = grammar_project();
    project.write("src/z.rs", "// stall here\nfn z() {}\n");
    // A grammar that never returns from `src/z.rs`, within a budget of 200 ms.
    let stall = [
        (
            "MARA_TEST_GRAMMAR_STALL",
            std::path::Path::new("stall here"),
        ),
        ("MARA_TEST_GRAMMAR_BUDGET_MS", std::path::Path::new("200")),
    ];
    let json = |args: &[&str]| -> Value {
        let mut all = vec!["--format", "json"];
        all.extend(args);
        let output = project.mara(&all, &stall).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let result = json(&["query", "execute", "r_link", "--limit", "100"]);
    let links: Rows = (result["rows"].as_array().unwrap().iter())
        .map(|values| row(values["values"].as_array().unwrap()))
        .collect();
    let expected: Rows = [
        grammar_row(2, 2, 2, ""),
        grammar_row(1, 6, 6, ""),
        grammar_row(4, 9, 9, ""),
        grammar_row(1, 11, 11, ""),
    ]
    .into();
    assert_eq!(links, expected, "{result}");
    let dropped = (result["diagnostics"].as_array().unwrap().iter())
        .filter(|d| d["code"] == "unavailable-grammar")
        .count();
    assert_eq!(dropped, 1, "{result}");
    let listed = json(&[
        "relation",
        "list",
        "REQ-LOGIN",
        "--relation",
        "implemented_by",
    ]);
    let sources: Vec<&Value> = (listed["connections"].as_array().unwrap().iter())
        .map(|c| &c["source"]["line"])
        .collect();
    assert_eq!(sources, [&json!(6), &json!(11)], "{listed}");
    let complete_rows = json(&["query", "execute", "r_complete", "--limit", "100"]);
    assert_eq!(
        complete_rows["rows"].as_array().unwrap().len(),
        4,
        "{complete_rows}"
    );
}

/// The (code, path, line) of each diagnostic of `item validate <id>`, with the result's
/// validity and the exit status.
fn item_validate(project: &Project, id: &str) -> (Vec<(String, String, u64)>, Value, i32) {
    let output = project
        .mara(&["--format", "json", "item", "validate", id], &[])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = (result["diagnostics"].as_array().unwrap().iter())
        .map(|d| {
            let text = |name: &str| d[name].as_str().unwrap().to_owned();
            (text("code"), text("path"), d["line"].as_u64().unwrap())
        })
        .collect();
    (
        diagnostics,
        result["valid"].clone(),
        output.status.code().unwrap(),
    )
}

/// `item validate` reports the problems of the code that an invariant's program made it read,
/// as `project validate` does: a grammar missing from the cache, and a code file that cannot be
/// read. An item validation that reads no code reports nothing about it.
#[test]
fn item_validate_reports_the_problems_of_the_code_it_read() {
    let (project, cache, line) = grammar_project();
    fs::remove_dir_all(&cache).unwrap();
    // The template's invariants use no code link: nothing about code.
    let (plain, valid, status) = item_validate(&project, "REQ-LOGIN");
    let about_code =
        |d: &(String, String, u64)| d.1 == ".mara/config.toml" || d.1.starts_with("src/");
    assert!(!plain.iter().any(about_code), "{plain:?}");
    assert_eq!((valid, status), (json!(true), 0), "{plain:?}");

    need_invariant(&project);
    let pin = (
        "unavailable-grammar".to_owned(),
        ".mara/config.toml".to_owned(),
        line,
    );
    for id in ["NEED-ACCESS", "REQ-LOGIN"] {
        let (found, valid, status) = item_validate(&project, id);
        assert!(found.contains(&pin), "{id}: {found:?}");
        assert_eq!((valid, status), (json!(false), 1), "{id}: {found:?}");
    }
    let validated: Vec<String> = project.problems().into_iter().map(|p| p.0).collect();
    assert!(validated.contains(&pin.0), "{validated:?}");

    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "this case needs a user without CAP_DAC_OVERRIDE"
    );
    project.write("src/hidden.rs", "fn hidden() {}\n");
    let hidden = project.path("src/hidden.rs");
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o000)).unwrap();
    let (found, ..) = item_validate(&project, "REQ-LOGIN");
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o644)).unwrap();
    let unreadable = ("io-error".to_owned(), "src/hidden.rs".to_owned(), 1);
    assert!(found.contains(&unreadable), "{found:?}");
}
