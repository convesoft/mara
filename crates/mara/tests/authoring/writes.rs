//! The cases of docs/verification-writes.mara.md for the single-file operations:
//! VER-PRESERVED-BYTES, the refusals of VER-REFUSED-EDITS that need no pause point, and the CLI
//! cases of VER-CONCURRENT-WRITERS.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread;
use std::time::Duration;

use mara::write::WriteLock;

use crate::{Project, finish, item, mid, no_temporary_file};

/// A document with the line ending `eol`: three items with irregular table padding, `*` and
/// `_` emphasis and trailing spaces. With `crlf`, a byte order mark, front matter and no final
/// newline as well. The IDs end in `-C` or `-L` and the MIDs differ, so the documents can share
/// a project.
fn document(eol: &str, crlf: bool) -> String {
    let (tag, base) = if crlf { ("C", 0) } else { ("L", 10) };
    let mid = |n: u32| mid(base + n);
    let lines: Vec<String> = [
        "# Notes  ",
        "",
        "Some *emphasis* and _more_.  ",
        "",
        "---",
        "",
        &format!("<a id=\"{}\"></a>", mid(1)),
        "",
        "## Alpha",
        "",
        "|Type|Requirement|",
        "|---|---|",
        &format!("|  ID   |  REQ-ALPHA-{tag}  |"),
        "| Status|Draft |",
        &format!("|Satisfies|[NEED-BETA-{tag}](#{})|", mid(2)),
        "",
        "Body *of* alpha.  ",
        "",
        "---",
        "",
        &format!("<a id=\"{}\"></a>", mid(2)),
        "",
        "## Beta",
        "",
        "| Type   | Need |",
        "|:--|--:|",
        &format!("| ID | NEED-BETA-{tag} |"),
        "|Status| Draft|",
        "",
        "Beta _body_.",
        "",
        "---",
        "",
        &format!("<a id=\"{}\"></a>", mid(3)),
        "",
        "## Gamma",
        "",
        "|Type|Requirement|",
        "|---|---|",
        &format!("|ID|REQ-GAMMA-{tag}|"),
        "|Status|Draft|",
        "",
        "Gamma __body__.",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut text = lines.join(eol);
    if crlf {
        text = format!("\u{feff}---{eol}title: Notes{eol}---{eol}{text}");
    } else {
        text.push_str(eol);
    }
    text
}

/// Problems that `project fix` repairs, and some it never repairs (VER-FIX-CASES): a missing
/// anchor, a bare-ID value, a stale link path and stale link text, a missing separator, an
/// anchor without its blank line, a setext heading, a broken link.
fn broken() -> String {
    format!(
        "# Broken\n\n## No anchor\n\n| Type | Need |\n| --- | --- |\n| ID | NEED-NO-ANCHOR |\n\
         | Status | Draft |\n\nText.\n\n---\n\n<a id=\"{m4}\"></a>\n\n## Stale\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | REQ-STALE |\n| Status | Draft |\n\
         | Satisfies | NEED-NO-ANCHOR |\n| Refines | [REQ-OLD](old.mara.md#{m5}) |\n\n\
         Mentions [gone](#{m9}).\n\n<a id=\"{m5}\"></a>\n## Merged\n\n\
         | Type | Requirement |\n| --- | --- |\n| ID | REQ-MERGED |\n| Status | Draft |\n\n\
         Setext\n---\n",
        m4 = mid(4),
        m5 = mid(5),
        m9 = mid(9),
    )
}

/// The project of VER-PRESERVED-BYTES: the CRLF document at mode 0600, the LF one at 0644, and
/// a document with the problems of [`broken`].
fn preserved_project() -> Project {
    let project = Project::new();
    project.write("docs/crlf.mara.md", &document("\r\n", true));
    project.write("docs/lf.mara.md", &document("\n", false));
    project.write("docs/broken.mara.md", &broken());
    for (path, mode) in [("docs/crlf.mara.md", 0o600), ("docs/lf.mara.md", 0o644)] {
        fs::set_permissions(project.path(path), fs::Permissions::from_mode(mode)).unwrap();
    }
    project
}

#[test]
fn each_operation_changes_only_the_spans_it_requires() {
    for (path, eol, crlf) in [
        ("docs/crlf.mara.md", "\r\n", true),
        ("docs/lf.mara.md", "\n", false),
    ] {
        let old = document(eol, crlf);
        let (tag, base) = if crlf { ("C", 0) } else { ("L", 10) };
        let [alpha, beta, gamma] =
            ["REQ-ALPHA", "NEED-BETA", "REQ-GAMMA"].map(|id| format!("{id}-{tag}"));
        let beta_mid = mid(base + 2);
        let satisfies = format!("|Satisfies|[{beta}](#{beta_mid})|{eol}");
        let beta_end = old.find("Beta _body_.").unwrap() + "Beta _body_.".len() + eol.len();
        type Expect = Box<dyn Fn(&serde_json::Value) -> String>;
        let cases: Vec<(Vec<&str>, Expect)> = vec![
            (
                vec![
                    "item",
                    "create",
                    "requirement",
                    path,
                    "--title",
                    "New",
                    "--field",
                    "status=draft",
                    "--body",
                    "New body.",
                ],
                Box::new({
                    let old = old.clone();
                    let eol = eol.to_owned();
                    move |json| {
                        let mid = json["item"]["mid"].as_str().unwrap();
                        let lead = if crlf {
                            format!("{eol}{eol}")
                        } else {
                            eol.clone()
                        };
                        let item = [
                            &format!("<a id=\"{mid}\"></a>") as &str,
                            "",
                            "## New",
                            "",
                            "| Type | Requirement |",
                            "| --- | --- |",
                            "| ID | REQ-NEW |",
                            "| Status | Draft |",
                            "",
                            "New body.",
                        ]
                        .join(&eol);
                        format!("{old}{lead}---{eol}{eol}{item}{eol}")
                    }
                }),
            ),
            (
                vec![
                    "item",
                    "update",
                    &alpha,
                    "--title",
                    "Alpha two",
                    "--field",
                    "status=accepted",
                ],
                Box::new({
                    let old = old.clone();
                    move |_| {
                        old.replacen("## Alpha", "## Alpha two", 1).replacen(
                            "| Status|Draft |",
                            "| Status|Accepted |",
                            1,
                        )
                    }
                }),
            ),
            (
                vec!["item", "delete", &gamma],
                Box::new({
                    let old = old.clone();
                    move |_| old[..beta_end].to_owned()
                }),
            ),
            (
                vec!["relation", "add", &gamma, "satisfies", &beta],
                Box::new({
                    let old = old.clone();
                    let row = format!("|Status|Draft|{eol}| Satisfies | [{beta}](#{beta_mid}) |");
                    move |_| old.replacen("|Status|Draft|", &row, 1)
                }),
            ),
            (
                vec!["relation", "remove", &alpha, "satisfies", &beta],
                Box::new({
                    let old = old.clone();
                    let satisfies = satisfies.clone();
                    move |_| old.replacen(&satisfies, "", 1)
                }),
            ),
        ];
        for (args, expected) in cases {
            let project = preserved_project();
            let before = project.tree();
            let run = project.run(&args).ok();
            assert_eq!(
                project.read(path),
                expected(&run.json()),
                "{args:?} on {path}"
            );
            let after = project.tree();
            assert_eq!(after[path].mode, before[path].mode, "{args:?} on {path}");
            // The problems of the broken document stay as they were (VER-FIX-CASES).
            assert_eq!(after["docs/broken.mara.md"].bytes, broken().into_bytes());
            for (other, file) in &before {
                if other != path {
                    assert_eq!(after.get(other), Some(file), "{args:?} changed {other}");
                }
            }
            assert_eq!(after.len(), before.len(), "{args:?} on {path}");
        }
    }
}

/// Each operation on the item `REQ-A`, with its source in `docs/a.mara.md`.
fn operations() -> Vec<Vec<&'static str>> {
    vec![
        vec![
            "item",
            "create",
            "requirement",
            "docs/a.mara.md",
            "--title",
            "New",
            "--field",
            "status=draft",
            "--body",
            "B.",
        ],
        vec!["item", "update", "REQ-A", "--field", "status=accepted"],
        vec!["item", "delete", "REQ-A"],
        vec!["relation", "add", "REQ-A", "satisfies", "NEED-N"],
        vec!["relation", "remove", "REQ-A", "refines", "REQ-B"],
    ]
}

/// A project for [`operations`]: `REQ-A` refines `REQ-B` in another file, and `NEED-N`.
fn operations_project() -> Project {
    let project = Project::new();
    let refines = format!("[REQ-B](b.mara.md#{})", mid(2));
    project.write(
        "docs/a.mara.md",
        &crate::document(
            "A",
            &[
                item(
                    &mid(1),
                    "Requirement",
                    "REQ-A",
                    "A",
                    &[("Refines", &refines)],
                    "A.",
                ),
                item(&mid(3), "Need", "NEED-N", "N", &[], "N."),
            ],
        ),
    );
    project.write(
        "docs/b.mara.md",
        &crate::document(
            "B",
            &[item(&mid(2), "Requirement", "REQ-B", "B", &[], "B.")],
        ),
    );
    project
}

#[test]
fn refuses_a_broken_project_an_unknown_or_ambiguous_item_and_an_undeclared_option() {
    for args in operations() {
        let project = operations_project();
        project.run(&args).ok();
    }
    for args in operations() {
        let project = operations_project();
        project.write(".mara/schema.toml", "format_version = 1\nstatuses = [\n");
        let tree = project.tree();
        project.run(&args).refused("invalid-project");
        project.unchanged(&tree, &format!("{args:?} on a broken schema"));
        let mut with = args.clone();
        with.extend(["--no-such-option", "x"]);
        project.run(&with).rejected();
        project.unchanged(&tree, &format!("{with:?}"));
    }
    for (code, extra) in [
        ("unknown-item", None),
        (
            "ambiguous-item",
            Some(item(&mid(9), "Requirement", "REQ-A", "Copy", &[], "Copy.")),
        ),
    ] {
        for args in operations().into_iter().skip(1) {
            let project = operations_project();
            match &extra {
                Some(copy) => project.write("docs/copy.mara.md", copy),
                None => {
                    // The operation names an item that no longer exists.
                    project.write(
                        "docs/a.mara.md",
                        &project.read("docs/a.mara.md").replace("REQ-A", "REQ-Z"),
                    );
                }
            }
            let tree = project.tree();
            project.run(&args).refused(code);
            project.unchanged(&tree, &format!("{args:?}"));
        }
    }
}

#[test]
fn ten_creates_at_once_all_land_in_the_document() {
    let project = Project::new();
    project.write("docs/reqs.mara.md", &crate::document("Requirements", &[]));
    let titles: Vec<String> = (0..10).map(|n| format!("Concurrent {n}")).collect();
    let children: Vec<_> = titles
        .iter()
        .map(|title| {
            let args = [
                "item",
                "create",
                "requirement",
                "docs/reqs.mara.md",
                "--title",
                title,
                "--field",
                "status=draft",
                "--body",
                "Text.",
            ];
            let child = crate::command(&project.root(), &args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("start mara");
            (args.map(str::to_owned), child)
        })
        .collect();
    let text = |project: &Project| project.read("docs/reqs.mara.md");
    let mut created = 0;
    for (args, child) in children {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = finish(&args, child.wait_with_output().unwrap());
        match run.status {
            0 => {
                created += 1;
                let mid = run.json()["item"]["mid"].as_str().unwrap().to_owned();
                assert!(text(&project).contains(&format!("<a id=\"{mid}\"></a>")));
            }
            _ => assert_eq!(run.status, 1, "{run:#?}"),
        }
    }
    let items = project.run(&["item", "list"]).ok().json()["items"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(items, created);
    assert!(created > 0);
    project.valid();
}

#[test]
fn a_write_waits_for_the_lock_and_then_applies_to_the_changed_file() {
    let project = Project::new();
    let path = "docs/reqs.mara.md";
    project.write(path, &crate::document("Requirements", &[]));
    let lock = WriteLock::take(&project.root()).unwrap();
    let before = project.tree();
    let args = [
        "item",
        "create",
        "requirement",
        path,
        "--title",
        "Waited",
        "--field",
        "status=draft",
        "--body",
        "Text.",
    ];
    let child = crate::command(&project.root(), &args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start mara");
    thread::sleep(Duration::from_millis(500));
    project.unchanged(&before, "a write while the lock is held");
    let changed = format!("{}\nChanged while Mara waited.\n", project.read(path));
    project.write(path, &changed);
    drop(lock);
    let run = finish(&args, child.wait_with_output().unwrap());
    let after = project.read(path);
    match run.status {
        0 => {
            assert!(after.starts_with(&changed), "{after}");
            assert!(after.contains("## Waited"), "{after}");
        }
        _ => {
            assert_eq!(run.json()["error"]["code"], "changed-file", "{run:#?}");
            assert_eq!(after, changed);
        }
    }
    assert!(no_temporary_file(&project.tree()));
}
