//! VER-CODE-EDITS at the basic level, the code row of VER-DELETE-ITEM and the code rows of
//! VER-INTERRUPTED-EDITS and VER-REFUSED-EDITS.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::process::{CommandExt, ExitStatusExt};

use serde_json::json;

use crate::{Project, item, paused, unchanged};

/// Each code file of the rename, with `{ID}` where a marker names REQ-LOGIN: before
/// declarations and after code, in two directories, a Python script with a shebang, a JSDoc
/// block, CRLF line endings, a file that is not UTF-8 and one without a final newline. Other uses
/// of REQ-LOGIN are no marker.
const CODE: [(&str, &[u8]); 7] = [
    (
        "src/one/a.rs",
        b"// @mara implements {ID}\npub fn login() {\n    check(); // @mara implements {ID}\n    \
          let s = \"REQ-LOGIN\";\n    // REQ-LOGIN is the login requirement\n}\n\
          // @mara implements REQ-LOGIN-AUDIT\npub fn audit() {}\n",
    ),
    (
        "src/two/b.rs",
        b"/// Signs out.\n// @mara implements {ID}\npub fn logout() {}\n",
    ),
    (
        "scripts/tool.py",
        b"#!/usr/bin/env python3\n# @mara implements {ID}\nprint('in')\n",
    ),
    (
        "web/app.ts",
        b"/**\n * Signs in.\n * @mara implements {ID}\n */\nexport function login() {}\n",
    ),
    (
        "src/crlf.rs",
        b"// @mara implements {ID}\r\nfn crlf() {}\r\n",
    ),
    (
        "src/latin1.rs",
        b"// Caf\xe9\n// @mara implements {ID}\nfn latin() {}\n",
    ),
    ("src/nofinal.rs", b"fn last() {} // @mara implements {ID}"),
];

fn with_id(template: &[u8], id: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(at) = rest.windows(4).position(|window| window == b"{ID}") {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(id.as_bytes());
        rest = &rest[at + 4..];
    }
    out.extend_from_slice(rest);
    out
}

/// [`Project::new`] with the code of [`CODE`], files that are no input with markers naming
/// REQ-LOGIN, and DES-SESSION, which satisfies REQ-LOGIN and mentions it by its ID.
fn renamed() -> Project {
    let project = Project::new();
    for (path, template) in CODE {
        project.write(path, with_id(template, "REQ-LOGIN"));
    }
    fs::set_permissions(
        project.path("scripts/tool.py"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let marker = "// @mara implements REQ-LOGIN\n";
    project.write("src/notes.txt", marker);
    project.write(".gitignore", "/src/ignored.rs\n");
    project.write("src/ignored.rs", marker);
    project.write("src/nested/.mara/config.toml", "format_version = 1\n");
    project.write("src/nested/inner.rs", marker);
    project.write("extern/real.rs", marker);
    symlink("../extern/real.rs", project.path("src/link.rs")).unwrap();
    let login = crate::mid(1);
    let design = item(
        5,
        "Design",
        "DES-SESSION",
        &format!("| Satisfies | [REQ-LOGIN](req.mara.md#{login}) |\n"),
        &format!("It keeps [REQ-LOGIN](req.mara.md#{login}) signed in."),
    );
    project.write("docs/design.mara.md", format!("# Design\n\n{design}"));
    project
}

#[test]
fn renames_markers_and_nothing_else_in_code() {
    let project = renamed();
    assert_eq!(project.problems(), []);
    let before = project.tree();
    let result = project.json(&["item", "rename", "REQ-LOGIN", "REQ-SIGN-IN"], 0);
    assert_eq!(
        result["files"],
        json!([
            "docs/design.mara.md",
            "scripts/tool.py",
            "src/crlf.rs",
            "src/latin1.rs",
            "src/nofinal.rs",
            "src/one/a.rs",
            "src/two/b.rs",
            "web/app.ts",
            "docs/req.mara.md"
        ])
    );
    for (path, template) in CODE {
        assert!(
            project.bytes(path) == with_id(template, "REQ-SIGN-IN"),
            "{path}"
        );
    }
    let mode = fs::metadata(project.path("scripts/tool.py"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o7777, 0o755);
    let after = project.tree();
    for path in [
        "src/notes.txt",
        "src/ignored.rs",
        "src/nested/inner.rs",
        "src/link.rs",
        "extern/real.rs",
    ] {
        assert_eq!(before[path], after[path], "{path}");
    }
    assert_eq!(project.problems(), []);
    assert_eq!(
        project.code_links("REQ-SIGN-IN"),
        [
            "scripts/tool.py:2",
            "src/crlf.rs:1",
            "src/latin1.rs:2",
            "src/nofinal.rs:1",
            "src/one/a.rs:1",
            "src/one/a.rs:3",
            "src/two/b.rs:2",
            "web/app.ts:3"
        ]
    );
    assert_eq!(project.code_links("REQ-LOGIN-AUDIT"), ["src/one/a.rs:7"]);

    // Through `mara mcp`: the same files and the CLI's JSON result.
    let other = renamed();
    let mut mcp = other.mcp();
    let answer = mcp.call(
        "item_rename",
        json!({"id-or-mid": "REQ-LOGIN", "new-id": "REQ-SIGN-IN"}),
    );
    drop(mcp);
    assert_eq!(answer, result);
    for (path, _) in CODE {
        assert!(other.bytes(path) == project.bytes(path), "{path}");
    }
}

/// Rust raw strings holding code examples with marker-like lines that name REQ-LOGIN: no
/// marker at the grammar level.
const RAW: &[u8] = b"/// Docs.\npub fn example() -> &'static str {\n    r#\"\n// @mara implements REQ-LOGIN\n\
                     fn shown() {}\n\"#\n}\n\nconst TOO: &str = r\"\n  /// @mara implements REQ-LOGIN\n\";\n";

/// The code links of `id`, as `<path>:<line>` of each marker, the source, in path and line
/// order.
fn sources(project: &Project, id: &str) -> Vec<String> {
    let args = [
        "relation",
        "list",
        id,
        "--relation",
        "implemented_by",
        "--limit",
        "100",
    ];
    let result = project.json(&args, 0);
    let connections = result["connections"].as_array().unwrap();
    let mut sources: Vec<String> = connections
        .iter()
        .map(|c| {
            format!(
                "{}:{}",
                c["source"]["path"].as_str().unwrap(),
                c["source"]["line"]
            )
        })
        .collect();
    sources.sort();
    sources
}

#[test]
fn renames_markers_and_nothing_else_with_grammars() {
    let installed = || {
        let mut project = renamed();
        project.write("src/raw.rs", RAW);
        project.install_grammar("rust");
        project.install_grammar("python");
        project
    };
    let project = installed();
    assert_eq!(project.problems(), []);
    let before = project.tree();
    let markers = sources(&project, "REQ-LOGIN");
    assert_eq!(
        markers,
        [
            "scripts/tool.py:2",
            "src/crlf.rs:1",
            "src/latin1.rs:2",
            "src/nofinal.rs:1",
            "src/one/a.rs:1",
            "src/one/a.rs:3",
            "src/two/b.rs:2",
            "web/app.ts:3"
        ]
    );
    // Each marker before a declaration, or after code in one, is owned by it.
    let owners = project.json(&["relation", "list", "src/one/a.rs:3"], 0);
    assert_eq!(owners["node"]["declaration"], "login");
    assert_eq!(owners["connections"].as_array().unwrap().len(), 2);

    let result = project.json(&["item", "rename", "REQ-LOGIN", "REQ-SIGN-IN"], 0);
    assert_eq!(
        result["files"],
        json!([
            "docs/design.mara.md",
            "scripts/tool.py",
            "src/crlf.rs",
            "src/latin1.rs",
            "src/nofinal.rs",
            "src/one/a.rs",
            "src/two/b.rs",
            "web/app.ts",
            "docs/req.mara.md"
        ])
    );
    for (path, template) in CODE {
        assert!(
            project.bytes(path) == with_id(template, "REQ-SIGN-IN"),
            "{path}"
        );
    }
    let after = project.tree();
    for path in [
        "src/raw.rs",
        "src/notes.txt",
        "src/ignored.rs",
        "src/nested/inner.rs",
        "src/link.rs",
        "extern/real.rs",
    ] {
        assert_eq!(before[path], after[path], "{path}");
    }
    let mode = fs::metadata(project.path("scripts/tool.py"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o7777, 0o755);
    assert_eq!(project.problems(), []);
    assert_eq!(sources(&project, "REQ-SIGN-IN"), markers);

    // Through `mara mcp`: the same files and the CLI's JSON result.
    let other = installed();
    let mut mcp = other.mcp();
    let answer = mcp.call(
        "item_rename",
        json!({"id-or-mid": "REQ-LOGIN", "new-id": "REQ-SIGN-IN"}),
    );
    drop(mcp);
    assert_eq!(answer, result);
    for (path, _) in CODE {
        assert!(other.bytes(path) == project.bytes(path), "{path}");
    }
    assert!(other.bytes("src/raw.rs") == RAW);
}

#[test]
fn refuses_a_rename_while_a_code_file_cannot_be_read() {
    let project = renamed();
    let unreadable = project.path("src/two/b.rs");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let before = project.tree_without("src/two/b.rs");
    let refused = project.json(&["item", "rename", "REQ-LOGIN", "REQ-SIGN-IN"], 1);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(refused["error"]["code"], "io-error", "{refused}");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("src/two/b.rs")
    );
    assert_eq!(refused["error"]["diagnostics"][0]["path"], "src/two/b.rs");
    unchanged(
        &before,
        &project.tree_without("src/two/b.rs"),
        "unreadable code file",
    );
}

#[test]
fn refuses_to_delete_an_item_that_a_marker_names() {
    let project = Project::new();
    project.write(
        "src/audit.rs",
        "fn audit() {}\n// @mara implements REQ-AUDIT\n",
    );
    let before = project.tree();
    let refused = project.json(&["item", "delete", "REQ-AUDIT"], 1);
    assert_eq!(refused["error"]["code"], "targeted-item");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(message.contains("src/audit.rs:2"), "{message}");
    let diagnostic = &refused["error"]["diagnostics"][0];
    assert_eq!(
        (&diagnostic["path"], &diagnostic["line"]),
        (&json!("src/audit.rs"), &json!(2))
    );
    unchanged(&before, &project.tree(), "refused delete");

    project.write("src/audit.rs", "fn audit() {}\n");
    project.run(&["item", "delete", "REQ-AUDIT"], 0);
    assert!(!project.read("docs/req.mara.md").contains("REQ-AUDIT |"));
    assert_eq!(project.problems(), []);
}

/// REQ-LOGIN named by markers in `src/a.rs` and `src/b.rs`, and by DES-SESSION's relation in
/// `docs/design.mara.md`, which has no mention: the rename writes the two code files, then the
/// `ID` row's document, then the design's link text.
fn two_code_files() -> Project {
    let project = Project::new();
    project.write("src/a.rs", "// @mara implements REQ-LOGIN\nfn a() {}\n");
    project.write("src/b.rs", "fn b() {} // @mara implements REQ-LOGIN\n");
    let design = item(
        5,
        "Design",
        "DES-SESSION",
        &format!(
            "| Satisfies | [REQ-LOGIN](req.mara.md#{}) |\n",
            crate::mid(1)
        ),
        "It keeps users signed in.",
    );
    project.write("docs/design.mara.md", format!("# Design\n\n{design}"));
    project
}

const RENAME: [&str; 4] = ["item", "rename", "REQ-LOGIN", "REQ-SIGN-IN"];
const ORDER: [&str; 4] = [
    "src/a.rs",
    "src/b.rs",
    "docs/req.mara.md",
    "docs/design.mara.md",
];

#[test]
fn refuses_a_rename_whose_code_file_changed_before_writing() {
    for (changed, at_check) in [("src/a.rs", false), ("src/b.rs", false), ("src/a.rs", true)] {
        let case = format!("{changed} changed, at its own pause: {at_check}");
        let project = two_code_files();
        let text = project.read(changed) + "// Changed by another program.\n";
        let before = project.tree_without(changed);
        let (output, pauses) = paused(&project, &RENAME, |held| match at_check {
            false if held.is_empty() => project.write(changed, &text),
            true if held == changed => project.write(changed, &text),
            _ => {}
        });
        assert_eq!(output.status.code(), Some(1), "{case}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(changed),
            "{case}"
        );
        assert!(pauses.len() <= 2, "{case}: {pauses:?}");
        assert_eq!(project.read(changed), text, "{case}");
        unchanged(&before, &project.tree_without(changed), &case);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Killed,
    TemporaryFails,
    Changed,
    RenameFails,
}

/// Stops the rename at each code file in each way (VER-INTERRUPTED-EDITS), then at the
/// design's link text after the `ID` row.
#[test]
fn stops_a_rename_at_each_code_file() {
    assert_ne!(
        // SAFETY: `geteuid` has no preconditions.
        unsafe { libc::geteuid() },
        0,
        "the rename-fails case needs a user without CAP_DAC_OVERRIDE"
    );
    let learned = two_code_files();
    let (output, pauses) = paused(&learned, &RENAME, |_| {});
    assert!(output.status.success(), "{output:?}");
    assert_eq!(pauses[1..], ORDER, "the write order");
    let new: BTreeMap<&str, Vec<u8>> = ORDER.iter().map(|p| (*p, learned.bytes(p))).collect();
    for index in [0, 1] {
        for stop in [
            Stop::Killed,
            Stop::TemporaryFails,
            Stop::Changed,
            Stop::RenameFails,
        ] {
            if stop == Stop::Changed && index == 0 {
                continue;
            }
            stopped(index, stop, &new);
        }
    }

    // Stopped after the `ID` row: every marker names the new ID and resolves.
    let project = two_code_files();
    let design = ORDER[3];
    let text = project.read(design) + "\nChanged by another program.\n";
    let (output, _) = paused(&project, &RENAME, |held| {
        if held == design {
            project.write(design, &text);
        }
    });
    assert_eq!(output.status.code(), Some(1));
    for path in &ORDER[..3] {
        assert!(project.bytes(path) == new[path], "{path}");
    }
    let problems = project.problems();
    assert!(
        problems
            .iter()
            .all(|(code, path, _)| code == "stale-link-text" && path == design),
        "{problems:?}"
    );
    assert_eq!(
        project.code_links("REQ-SIGN-IN"),
        ["src/a.rs:1", "src/b.rs:1"]
    );
}

/// The rename of [`two_code_files`] stopped at `ORDER[index]` by `stop`, then rerun.
fn stopped(index: usize, stop: Stop, new: &BTreeMap<&str, Vec<u8>>) {
    let file = ORDER[index];
    let case = format!("stopped at {file} ({stop:?})");
    let project = two_code_files();
    // F's new contents are larger than every other file's.
    let padding = "// A comment that makes this file the largest the rename writes.\n".repeat(200);
    let padded = project.read(file) + &padding;
    project.write(file, &padded);
    let old: BTreeMap<&str, Vec<u8>> = ORDER.iter().map(|p| (*p, project.bytes(p))).collect();
    let largest = ORDER
        .iter()
        .filter(|p| **p != file)
        .map(|p| new[p].len())
        .max()
        .unwrap();
    let limit = (largest + padded.len()) / 2;
    let mut changed = None;
    let output = match stop {
        Stop::Killed | Stop::TemporaryFails => {
            let mut command = project.mara(&RENAME, &[]);
            let ignore = stop == Stop::TemporaryFails;
            let limit = limit as libc::rlim_t;
            // SAFETY: the closure runs in the child before `exec` and calls only
            // `getrlimit`, `setrlimit` and `signal`, which are async-signal-safe.
            unsafe {
                command.pre_exec(move || {
                    let size = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(libc::RLIMIT_FSIZE, &size) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    // A soft core limit of 1 byte where the hard limit allows it: then a piped
                    // `core_pattern` records no crash for SIGXFSZ (DES-RULE-EXECUTION).
                    let mut core = libc::rlimit {
                        rlim_cur: 0,
                        rlim_max: 0,
                    };
                    if libc::getrlimit(libc::RLIMIT_CORE, &mut core) == 0 && core.rlim_max >= 1 {
                        core.rlim_cur = 1;
                        libc::setrlimit(libc::RLIMIT_CORE, &core);
                    }
                    let action = if ignore { libc::SIG_IGN } else { libc::SIG_DFL };
                    libc::signal(libc::SIGXFSZ, action);
                    Ok(())
                });
            }
            command.output().expect("run mara")
        }
        Stop::Changed => {
            let text = padded.clone() + "// Changed by another program.\n";
            changed = Some(text.clone().into_bytes());
            paused(&project, &RENAME, |held| {
                if held == file {
                    project.write(file, &text);
                }
            })
            .0
        }
        Stop::RenameFails => {
            let directory = project.path(file).parent().unwrap().to_owned();
            let set =
                |mode| fs::set_permissions(&directory, fs::Permissions::from_mode(mode)).unwrap();
            let output = paused(&project, &RENAME, |held| {
                if held == file {
                    set(0o555);
                }
            })
            .0;
            set(0o755);
            output
        }
    };
    match stop {
        Stop::Killed => assert_eq!(
            output.status.signal(),
            Some(libc::SIGXFSZ),
            "{case}: {output:?}"
        ),
        _ => {
            assert_eq!(output.status.code(), Some(1), "{case}: {output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(file),
                "{case}"
            );
        }
    }
    let temporary = project
        .tree()
        .into_keys()
        .find(|path| path.contains(".mara-tmp-"));
    match (stop, temporary) {
        (Stop::TemporaryFails, Some(left)) => panic!("{case}: {left} is left"),
        // A temporary file that a killed write leaves is no input, though `src/**` matches it.
        (_, Some(left)) => {
            let config = project.read(".mara/config.toml");
            let wide = config.replace("include = [\"src/**/*.rs\"]", "include = [\"src/**\"]");
            project.write(".mara/config.toml", wide);
            assert!(
                project.problems().iter().all(|(_, path, _)| *path != left),
                "{case}"
            );
            project.write(".mara/config.toml", config);
        }
        _ => {}
    }
    for (at, path) in ORDER.iter().enumerate() {
        let expected = match at {
            _ if at < index => new[path].clone(),
            _ if at == index => changed.clone().unwrap_or_else(|| old[path].clone()),
            _ => old[path].clone(),
        };
        assert!(project.bytes(path) == expected, "{case}: {path}");
    }
    // Before the rerun, the markers already written name the new ID and do not resolve yet.
    let problems = project.problems();
    let expected: Vec<(String, String, u64)> = ORDER[..index]
        .iter()
        .map(|path| ("unresolved-marker".to_owned(), (*path).to_owned(), 1))
        .collect();
    assert_eq!(problems, expected, "{case}");
    project.run(&RENAME, 0);
    assert_eq!(project.problems(), [], "{case}: after the rerun");
    assert_eq!(
        project.code_links("REQ-SIGN-IN"),
        ["src/a.rs:1", "src/b.rs:1"],
        "{case}"
    );
}

impl Project {
    /// [`Project::tree`] without `path`, which the test changes.
    fn tree_without(&self, path: &str) -> crate::Tree {
        let mut tree = self.tree();
        tree.remove(path);
        tree
    }
}

/// Every marker that names the ID refuses the delete, whatever its relation and whether its
/// target's type is allowed (REQ-DELETE-ITEM).
#[test]
fn refuses_to_delete_an_item_that_any_marker_names() {
    let project = Project::new();
    project.write("src/typo.rs", "// @mara implemnts REQ-LOGIN\n");
    project.write(
        "src/need.rs",
        "fn need() {}\n// @mara implements NEED-ACCESS\n",
    );
    let before = project.tree();
    for (id, path, line) in [
        ("REQ-LOGIN", "src/typo.rs", 1),
        ("NEED-ACCESS", "src/need.rs", 2),
    ] {
        let refused = project.json(&["item", "delete", id], 1);
        assert_eq!(refused["error"]["code"], "targeted-item", "{id}: {refused}");
        let diagnostic = &refused["error"]["diagnostics"][0];
        assert_eq!(
            (&diagnostic["path"], &diagnostic["line"]),
            (&json!(path), &json!(line)),
            "{id}"
        );
    }
    unchanged(&before, &project.tree(), "refused deletes");
}

/// A delete that cannot read every code file writes nothing and names what it could not read,
/// through the CLI and `mara mcp` (DES-CODE-READERS).
#[test]
fn refuses_to_delete_while_code_cannot_be_read() {
    type Hide = fn(&Project) -> &'static str;
    type Restore = fn(&Project);
    let cases: [(&str, Hide, Restore); 3] = [
        (
            "an unreadable code file",
            |project| {
                let file = project.path("src/a.rs");
                fs::set_permissions(file, fs::Permissions::from_mode(0o000)).unwrap();
                "src/a.rs"
            },
            |project| {
                let file = project.path("src/a.rs");
                fs::set_permissions(file, fs::Permissions::from_mode(0o644)).unwrap();
            },
        ),
        (
            "an unreadable directory",
            |project| {
                let directory = project.path("src");
                fs::set_permissions(directory, fs::Permissions::from_mode(0o000)).unwrap();
                "src"
            },
            |project| {
                let directory = project.path("src");
                fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
            },
        ),
        (
            "a `.gitignore` that cannot be applied",
            |project| {
                project.write("src/.gitignore", b"\xff\n");
                "src/.gitignore"
            },
            |_| {},
        ),
    ];
    for (case, hide, restore) in cases {
        for mcp in [false, true] {
            let project = Project::new();
            project.write("src/a.rs", "// @mara implements REQ-LOGIN\n");
            let hidden = hide(&project);
            let before = project.tree();
            let refused = match mcp {
                false => project.json(&["item", "delete", "REQ-LOGIN"], 1),
                true => {
                    let mut server = project.mcp();
                    let answer = server.call("item_delete", json!({"item": "REQ-LOGIN"}));
                    drop(server);
                    answer
                }
            };
            let after = project.tree();
            restore(&project);
            let case = format!("{case}, through MCP: {mcp}");
            assert_eq!(refused["error"]["code"], "io-error", "{case}: {refused}");
            let message = refused["error"]["message"].as_str().unwrap();
            assert!(message.contains(hidden), "{case}: {message}");
            assert_eq!(refused["error"]["diagnostics"][0]["path"], hidden, "{case}");
            unchanged(&before, &after, &case);
            assert!(
                project
                    .read("docs/req.mara.md")
                    .contains("| ID | REQ-LOGIN |"),
                "{case}"
            );
        }
    }
}
