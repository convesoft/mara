//! VER-CODE-MARKERS at the basic level, through the CLI and `mara mcp`.

use std::fs;
use std::os::unix::fs::symlink;

use serde_json::{Value, json};

use crate::{Project, documents, unchanged};

/// Markers as developers write them, each naming REQ-LOGIN, and their locations.
fn written() -> Project {
    let project = Project::new();
    project.write(
        "src/login.rs",
        "//! Signing in.\n\n// @mara implements REQ-LOGIN\npub fn login() {\n    \
         // @mara implements REQ-LOGIN\n    check(); // @mara implements REQ-LOGIN\n}\n\n\
         /// @mara implements REQ-LOGIN\n// @mara implements REQ-AUDIT\npub fn audit() {}\n",
    );
    project.write(
        "src/windows/crlf.rs",
        "// @mara implements REQ-LOGIN\r\nfn crlf() {}\r\n",
    );
    project.write(
        "scripts/tool.py",
        "#!/usr/bin/env python3\n# @mara implements REQ-LOGIN\nprint('in')\n",
    );
    project.write(
        "web/app.ts",
        "/**\n * Signs in.\n * @mara implements REQ-LOGIN\n */\nexport function login() {}\n",
    );
    project.write(
        "src/latin1.rs",
        b"// Caf\xe9 au lait\n// @mara implements REQ-LOGIN\nfn latin() {}\n",
    );
    project
}

const LOGIN_LINKS: [&str; 8] = [
    "scripts/tool.py:2",
    "src/latin1.rs:2",
    "src/login.rs:3",
    "src/login.rs:5",
    "src/login.rs:6",
    "src/login.rs:9",
    "src/windows/crlf.rs:1",
    "web/app.ts:3",
];

/// The connections of `node` as `<relation> <direction> <neighbour> @<path>:<line>`.
fn described(result: &Value) -> Vec<String> {
    let connections = result["connections"].as_array().unwrap();
    connections
        .iter()
        .map(|c| {
            let neighbour = &c["neighbour"];
            let name = match neighbour["id"].as_str() {
                Some(id) => id.to_owned(),
                None => format!(
                    "{}:{}",
                    neighbour["path"].as_str().unwrap(),
                    neighbour["line"]
                ),
            };
            format!(
                "{} {} {name} @{}:{}",
                c["relation"].as_str().unwrap(),
                c["direction"].as_str().unwrap(),
                c["source"]["path"].as_str().unwrap(),
                c["source"]["line"]
            )
        })
        .collect()
}

#[test]
fn links_markers_as_developers_write_them() {
    let project = written();
    let before = documents(&project.tree());
    assert_eq!(project.problems(), []);
    assert_eq!(project.code_links("REQ-LOGIN"), LOGIN_LINKS);
    assert_eq!(project.code_links("REQ-AUDIT"), ["src/login.rs:10"]);

    // A line, and a whole file.
    let line = project.json(&["relation", "list", "src/login.rs:6"], 0);
    assert_eq!(
        line["node"],
        json!({"kind": "code", "path": "src/login.rs", "line": 6, "declaration": null})
    );
    assert_eq!(
        described(&line),
        ["implements outgoing REQ-LOGIN @src/login.rs:6"]
    );
    let file = project.json(&["relation", "list", "src/login.rs"], 0);
    assert_eq!(file["node"]["line"], Value::Null);
    assert_eq!(
        described(&file),
        [3, 5, 6, 9]
            .map(|line| format!("implements outgoing REQ-LOGIN @src/login.rs:{line}"))
            .into_iter()
            .chain(["implements outgoing REQ-AUDIT @src/login.rs:10".to_owned()])
            .collect::<Vec<_>>()
    );
    let empty = project.json(&["relation", "list", "src/login.rs:4"], 0);
    assert_eq!(described(&empty), [] as [&str; 0]);
    let refused = project.json(&["relation", "list", "src/login.rs:99"], 1);
    assert_eq!(refused["error"]["code"], "unknown-node");

    // Narrowed by relation: only code links, or none of them.
    let narrowed = project.json(
        &[
            "relation",
            "list",
            "REQ-LOGIN",
            "--relation",
            "implemented_by",
        ],
        0,
    );
    assert_eq!(
        narrowed["connections"].as_array().unwrap().len(),
        LOGIN_LINKS.len()
    );
    let other = project.json(
        &[
            "relation",
            "list",
            "REQ-LOGIN",
            "--relation",
            "satisfied_by",
        ],
        0,
    );
    assert_eq!(other["connections"], json!([]));

    // `get` of a line: the file from that line; of a file: all of it.
    let got = project.json(&["get", "src/login.rs:9"], 0);
    assert_eq!(
        got["node"],
        json!({"kind": "code", "path": "src/login.rs", "line": 9, "declaration": null})
    );
    assert_eq!(
        got["content"],
        "/// @mara implements REQ-LOGIN\n// @mara implements REQ-AUDIT\npub fn audit() {}\n"
    );
    let whole = project.json(&["get", "src/login.rs"], 0);
    assert_eq!(whole["content"], project.read("src/login.rs"));
    let human = String::from_utf8(project.run(&["get", "src/login.rs:9"], 0).stdout).unwrap();
    assert!(human.starts_with("src/login.rs:9: (code)\n"), "{human}");

    // Through `mara mcp`: the JSON results of the CLI.
    let mut mcp = project.mcp();
    for node in [
        "REQ-LOGIN",
        "src/login.rs",
        "src/login.rs:6",
        "scripts/tool.py:2",
    ] {
        let cli = project.json(&["relation", "list", node], 0);
        assert_eq!(
            mcp.call("relation_list", json!({"node": node})),
            cli,
            "{node}"
        );
        let cli = project.json(&["get", node], 0);
        assert_eq!(mcp.call("get", json!({"node": node})), cli, "{node}");
    }
    let validated = mcp.call("project_validate", json!({}));
    assert_eq!(validated, project.json(&["project", "validate"], 0));
    drop(mcp);
    unchanged(&before, &documents(&project.tree()), "reading code");
}

#[test]
fn reports_markers_that_do_not_resolve() {
    let project = Project::new();
    project.write(
        "src/invalid.rs",
        "// @mara implements\n// @mara implements REQ-LOGIN REQ-AUDIT\n\
         // @mara implemnts REQ-LOGIN\nlet s = \" // @mara implements REQ-LOGIN\";\n",
    );
    project.write(
        "src/unresolved.rs",
        "// @mara implements REQ-LOGN\n// @mara implements REQ-AUDIT\n\
         // @mara implements REQ-LOGIN-AUDIT\n",
    );
    project.write("src/need.rs", "// @mara implements NEED-ACCESS\n");
    let before = documents(&project.tree());
    let at = |code: &str, path: &str, line: u64| (code.to_owned(), path.to_owned(), line);
    assert_eq!(
        project.problems(),
        [
            at("disallowed-target", "src/need.rs", 1),
            at("invalid-marker", "src/invalid.rs", 1),
            at("invalid-marker", "src/invalid.rs", 2),
            at("invalid-marker", "src/invalid.rs", 3),
            at("invalid-marker", "src/invalid.rs", 4),
            at("unresolved-marker", "src/unresolved.rs", 1),
        ]
    );
    let result = project.json(&["project", "validate"], 1);
    for diagnostic in result["diagnostics"].as_array().unwrap() {
        assert_eq!(diagnostic["id"], Value::Null, "{diagnostic}");
        assert_eq!(diagnostic["mid"], Value::Null, "{diagnostic}");
    }
    assert_eq!(project.code_links("REQ-LOGIN"), [] as [&str; 0]);

    // The ID of REQ-AUDIT changed by hand, and REQ-LOGIN-AUDIT removed by hand.
    let text = project.read("docs/req.mara.md");
    let text = text.replace("| ID | REQ-AUDIT |", "| ID | REQ-AUDITED |");
    let removed = text
        .find("---\n\n<a id=\"01M7JQ00000000000000000003\"")
        .unwrap();
    let end = removed + text[removed..].find("Audits log in.\n\n").unwrap() + 16;
    project.write(
        "docs/req.mara.md",
        format!("{}{}", &text[..removed], &text[end..]),
    );
    let unresolved: Vec<u64> = project
        .problems()
        .into_iter()
        .filter(|(code, _, _)| code == "unresolved-marker")
        .map(|(_, path, line)| {
            assert_eq!(path, "src/unresolved.rs");
            line
        })
        .collect();
    assert_eq!(unresolved, [1, 2, 3]);
    assert_eq!(project.code_links("REQ-AUDITED"), [] as [&str; 0]);
    let mut changed = before;
    changed.remove("docs/req.mara.md");
    let mut after = documents(&project.tree());
    after.remove("docs/req.mara.md");
    unchanged(&changed, &after, "reporting markers");
}

#[test]
fn ignores_what_is_no_marker_or_no_input() {
    let project = Project::new();
    project.write(
        "src/words.rs",
        "// see @mara implements REQ-LOGIN\nlet s = \"// @mara implements REQ-LOGIN\";\n",
    );
    project.write("src/notes.txt", "// @mara implements REQ-LOGIN\n");
    project.write(".gitignore", "/src/ignored.rs\n");
    project.write("src/ignored.rs", "// @mara implements REQ-LOGIN\n");
    project.write("src/nested/.mara/config.toml", "format_version = 1\n");
    project.write("src/nested/inner.rs", "// @mara implements REQ-LOGIN\n");
    project.write("extern/real.rs", "// @mara implements REQ-LOGIN\n");
    symlink("../extern/real.rs", project.path("src/link.rs")).unwrap();
    assert_eq!(project.problems(), []);
    assert_eq!(project.code_links("REQ-LOGIN"), [] as [&str; 0]);
    let refused = project.json(&["get", "src/link.rs"], 1);
    assert_eq!(refused["error"]["code"], "unknown-item");
}

#[test]
fn links_copies_and_repeated_ids_to_each() {
    let project = Project::new();
    let marker = "fn a() {}\n// @mara implements REQ-LOGIN\n";
    project.write("src/one/copy.rs", marker);
    project.write("src/two/copy.rs", marker);
    assert_eq!(project.problems(), []);
    assert_eq!(
        project.code_links("REQ-LOGIN"),
        ["src/one/copy.rs:2", "src/two/copy.rs:2"]
    );

    // A marker naming an ID that two items have links to each.
    let twin = crate::item(
        9,
        "Requirement",
        "REQ-LOGIN",
        "",
        "A second item with the ID.",
    );
    let text = project.read("docs/req.mara.md");
    project.write("docs/req.mara.md", format!("{text}{twin}"));
    let codes: Vec<String> = project
        .problems()
        .into_iter()
        .map(|(code, ..)| code)
        .collect();
    assert!(codes.iter().all(|code| code == "duplicate-id"), "{codes:?}");
    let file = project.json(&["relation", "list", "src/one/copy.rs:2"], 0);
    let mids: Vec<&str> = file["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["neighbour"]["mid"].as_str().unwrap())
        .collect();
    assert_eq!(mids, [crate::mid(1), crate::mid(9)]);
}

#[test]
fn follows_markers_as_they_change() {
    let project = written();
    let before = project.code_links("REQ-AUDIT");
    let cursor_of = |args: &[&str]| {
        let page = project.json(&[args, &["--limit", "1"]].concat(), 0);
        page["next_cursor"].as_str().unwrap().to_owned()
    };
    let relations = cursor_of(&["relation", "list", "REQ-LOGIN"]);
    let search = cursor_of(&["search", "log"]);

    // One marker added, one moved to another line, one removed.
    project.write("src/added.rs", "// @mara implements REQ-AUDIT\n");
    let text = project.read("src/login.rs");
    let text = text.replace(
        "// @mara implements REQ-AUDIT\npub fn audit() {}\n",
        "pub fn audit() {}\n// @mara implements REQ-AUDIT\n",
    );
    project.write("src/login.rs", text);
    fs::remove_file(project.path("scripts/tool.py")).unwrap();
    assert_eq!(before, ["src/login.rs:10"]);
    assert_eq!(
        project.code_links("REQ-AUDIT"),
        ["src/added.rs:1", "src/login.rs:11"]
    );
    assert_eq!(project.code_links("REQ-LOGIN"), LOGIN_LINKS[1..]);

    let stale = project.json(
        &["relation", "list", "REQ-LOGIN", "--cursor", &relations],
        1,
    );
    assert_eq!(stale["error"]["code"], "stale-cursor");
    project.json(&["search", "log", "--cursor", &search], 0);
}

#[test]
fn a_project_without_code_languages_reads_no_code() {
    let project = written();
    project.write("src/invalid.rs", "// @mara implemnts REQ-LOGIN\n");
    let config = project.read(".mara/config.toml");
    let (without, _) = config.split_once("\n[code]").unwrap();
    project.write(".mara/config.toml", without);
    assert_eq!(project.problems(), []);
    assert_eq!(project.code_links("REQ-LOGIN"), [] as [&str; 0]);
}

/// `[code]` without a language declares none: the project stays valid, and its documents are
/// read as usual.
#[test]
fn a_code_table_without_languages_declares_none() {
    let project = written();
    let config = project.read(".mara/config.toml");
    let (without, _) = config.split_once("\n[code]").unwrap();
    project.write(
        ".mara/config.toml",
        format!("{without}\n[code]\nformat_version = 1\n"),
    );
    assert_eq!(project.problems(), []);
    assert_eq!(project.code_links("REQ-LOGIN"), [] as [&str; 0]);
    project.json(&["search", "log"], 0);
}

/// A code file whose name ends in `.mara.md`, outside the document patterns, is read as code.
#[test]
fn a_code_file_may_end_in_mara_md() {
    let project = Project::new();
    let config = project.read(".mara/config.toml");
    project.write(
        ".mara/config.toml",
        config.replace("include = [\"src/**/*.rs\"]", "include = [\"src/**\"]"),
    );
    let text = "// @mara implements REQ-LOGIN\nfn example() {}\n";
    project.write("src/example.mara.md", text);
    assert_eq!(project.code_links("REQ-LOGIN"), ["src/example.mara.md:1"]);
    let whole = project.json(&["get", "src/example.mara.md"], 0);
    assert_eq!(whole["node"]["kind"], "code");
    assert_eq!(whole["content"], text);
    let line = project.json(&["get", "src/example.mara.md:2"], 0);
    assert_eq!(line["content"], "fn example() {}\n");
    // A document is still read as one.
    let document = project.json(&["get", "docs/req.mara.md"], 0);
    assert_eq!(document["node"]["kind"], "document");
}
