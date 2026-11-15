//! `project fix` through the real `mara` binary, on disposable projects in the temporary
//! directory: VER-FIX-CASES and, for `project fix`, VER-PRESERVED-BYTES, VER-REFUSED-EDITS,
//! VER-INTERRUPTED-EDITS and VER-CONCURRENT-WRITERS (`docs/verification-writes.mara.md`).

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mara_engine::document::is_mid;
use serde_json::Value;

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/**/*.mara.md\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"draft\"]\n\n[types.requirement]\nid_prefix = \"REQ-\"\ndescription = \"A requirement.\"\n\n[relations.refines]\ninverse = \"refined_by\"\ndescription = \"Refines.\"\ntargets = { requirement = [\"requirement\"] }\n";

const A: &str = "01M7FWCTQ46H3XM2M9SKGXYSRK";
const B: &str = "01M7FWCTQDEP64X7ERJ8TV33P5";
const C: &str = "01M7FWCTQND76J0W53DRMFFE04";
const D: &str = "01M7FWCTQW7KS5BNRBB0T0P0CK";
const E: &str = "01M7FWCTR3AKDKGR7RGDD4CYY3";
const MISSING: &str = "01M7FWCTRCY4P0QDN4YEV83AWH";

/// An item of type `Requirement` with the ID `id`, `rows` after its `Status` row, each ending
/// with a line break, and `body`.
fn item(mid: &str, id: &str, rows: &str, body: &str) -> String {
    format!(
        "<a id=\"{mid}\"></a>\n\n### {id}\n\n| Type | Requirement |\n| --- | --- |\n| ID | {id} |\n\
         | Status | Draft |\n{rows}\n{body}\n"
    )
}

/// A document of a heading and `regions`, separated by thematic breaks.
fn page(title: &str, regions: &[String]) -> String {
    let mut text = format!("# {title}\n");
    for region in regions {
        text.push_str("\n---\n\n");
        text.push_str(region);
    }
    text
}

/// What a file of the tree is: a directory, or a file's bytes, mode and mtime.
type Entry = Option<(Vec<u8>, u32, SystemTime)>;

/// A project in its own temporary directory, removed on drop.
struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Project {
    fn new() -> Project {
        let dir = tempfile::Builder::new()
            .prefix("mara-fix-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        let project = Project { _dir: dir, root };
        project.write(".mara/config.toml", CONFIG);
        project.write(".mara/schema.toml", SCHEMA);
        project
    }

    /// A new project with the documents `files`.
    fn with(files: &[(&str, String)]) -> Project {
        let project = Project::new();
        for (path, text) in files {
            project.write(path, text);
        }
        project
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }

    /// Every entry below the root.
    fn tree(&self) -> BTreeMap<String, Entry> {
        fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Entry>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let relative = path.strip_prefix(root).unwrap().to_string_lossy();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    out.insert(relative.into_owned(), None);
                    walk(root, &path, out);
                } else {
                    let bytes = fs::read(&path).unwrap_or_default();
                    let mode = metadata.mode() & 0o7777;
                    let entry = Some((bytes, mode, metadata.modified().unwrap()));
                    out.insert(relative.into_owned(), entry);
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.root, &self.root, &mut out);
        out
    }

    /// `mara --format json <args>` in the project, with an empty environment.
    fn command(&self, args: &[&str]) -> Command {
        self.command_as("json", args)
    }

    /// `mara --format <format> <args>` in the project, with an empty environment.
    fn command_as(&self, format: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
        command
            .args(["--format", format])
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn run(&self, args: &[&str]) -> Run {
        Run::of(self.command(args).output().expect("run mara"))
    }

    fn fix(&self) -> Run {
        self.run(&["project", "fix"])
    }

    /// The `(code, path, line)` of every diagnostic of `project validate`, in its order.
    fn diagnostics(&self) -> Vec<(String, String, u64)> {
        let run = self.run(&["project", "validate", "--limit", "100"]);
        let json = run.json();
        assert_eq!(json["next_cursor"], Value::Null, "one page: {run:?}");
        json["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                let text = |key: &str| d[key].as_str().unwrap().to_owned();
                (text("code"), text("path"), d["line"].as_u64().unwrap())
            })
            .collect()
    }
}

/// One finished `mara` process.
#[derive(Debug)]
struct Run {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

impl Run {
    fn of(output: std::process::Output) -> Run {
        Run {
            status: output.status,
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {self:?}"))
    }

    fn code(&self) -> Option<i32> {
        self.status.code()
    }

    /// The `(code, path, line)` of each repair of a fix result.
    fn repairs(&self) -> Vec<(String, String, u64)> {
        assert_eq!(self.code(), Some(0), "{}{}", self.stdout, self.stderr);
        let json = self.json();
        let repairs = json["repairs"].as_array().unwrap();
        assert_eq!(json["repairs_total"].as_u64(), Some(repairs.len() as u64));
        repairs
            .iter()
            .map(|r| {
                let text = |key: &str| r[key].as_str().unwrap().to_owned();
                (text("code"), text("path"), r["line"].as_u64().unwrap())
            })
            .collect()
    }

    /// The code of a failed request.
    fn error(&self) -> String {
        self.json()["error"]["code"].as_str().unwrap().to_owned()
    }
}

fn at(code: &str, path: &str, line: u64) -> (String, String, u64) {
    (code.to_owned(), path.to_owned(), line)
}

/// The MID of the one anchor in `after` that `before` does not hold.
fn new_mid(before: &str, after: &str) -> String {
    let mids: Vec<&str> = after
        .match_indices("<a id=\"")
        .map(|(at, _)| &after[at + 7..at + 33])
        .filter(|mid| !before.contains(mid))
        .collect();
    let [mid] = mids[..] else {
        panic!("one new anchor: {mids:?} in {after:?}");
    };
    assert!(is_mid(mid), "{mid}");
    mid.to_owned()
}

#[test]
fn adds_the_anchor_an_item_lacks() {
    let lacking = "### REQ-NEW\n\n| Type | Requirement |\n| --- | --- |\n| ID | REQ-NEW |\n\
                   | Status | Draft |\n\nBody.\n";
    let before = page("Page", &[lacking.to_owned()]);
    let project = Project::with(&[("docs/a.mara.md", before.clone())]);
    assert_eq!(
        project.diagnostics(),
        [at("incomplete-item", "docs/a.mara.md", 5)]
    );
    let run = project.fix();
    assert_eq!(run.repairs(), [at("incomplete-item", "docs/a.mara.md", 5)]);
    let after = project.read("docs/a.mara.md");
    let mid = new_mid(&before, &after);
    assert_eq!(
        after,
        before.replace(
            "### REQ-NEW",
            &format!("<a id=\"{mid}\"></a>\n\n### REQ-NEW")
        )
    );
    assert_eq!(project.diagnostics(), []);
    let get = project.run(&["get", "REQ-NEW"]);
    assert_eq!(get.json()["node"]["mid"], mid.as_str(), "{get:?}");
}

/// The hand-authoring flow: items written without anchors, their relations named by ID. One
/// fix repairs everything fix can, also what only the new anchors make repairable.
#[test]
fn repairs_items_that_get_their_anchor_in_one_run() {
    let rows = "| Type | Requirement |\n| --- | --- |\n";
    let a = format!(
        "# A\n\n---\n\n### REQ-A\n\n{rows}| ID | REQ-A |\n| Status | Draft |\n\
         | Refines | REQ-B |\n| Refines | [REQ-OLD](old.mara.md#{C}) |\n| Refines | REQ-D |\n\n\
         See [C](old.mara.md#{C}).\n"
    );
    let d = format!(
        "# D\n\n---\n\nREQ-D title\n===========\n\n{rows}| ID | REQ-D |\n| Status | Draft |\n\
         | Refines | REQ-A |\n\nBody.\n"
    );
    let e = page("E", &[item(E, "REQ-E", "| Refines | REQ-D |\n", "Body.")]);
    let project = Project::with(&[
        ("docs/a.mara.md", a.clone()),
        (
            "docs/b/b.mara.md",
            page("B", &[item(B, "REQ-B", "", "Body.")]),
        ),
        (
            "docs/c.mara.md",
            page("C", &[item(C, "REQ-C", "", "Body.")]),
        ),
        ("docs/d/d.mara.md", d.clone()),
        ("docs/e.mara.md", e.clone()),
    ]);
    assert_eq!(
        project.diagnostics(),
        [
            at("incomplete-item", "docs/a.mara.md", 5),
            at("stale-link-path", "docs/a.mara.md", 15),
            at("incomplete-item", "docs/d/d.mara.md", 5),
            at("setext-heading", "docs/d/d.mara.md", 6),
            at("bare-id-value", "docs/e.mara.md", 13),
        ]
    );
    let repairs = project.fix().repairs();
    assert_eq!(repairs.len(), 10, "{repairs:?}");
    assert_eq!(project.diagnostics(), []);
    let a_after = project.read("docs/a.mara.md");
    let d_after = project.read("docs/d/d.mara.md");
    let (mid_a, mid_d) = (new_mid(&a, &a_after), new_mid(&d, &d_after));
    let a_fixed = a
        .replace("### REQ-A", &format!("<a id=\"{mid_a}\"></a>\n\n### REQ-A"))
        .replace("| REQ-B |", &format!("| [REQ-B](b/b.mara.md#{B}) |"))
        .replace(
            &format!("[REQ-OLD](old.mara.md#{C})"),
            &format!("[REQ-C](c.mara.md#{C})"),
        )
        .replace("| REQ-D |", &format!("| [REQ-D](d/d.mara.md#{mid_d}) |"))
        .replace("(old.mara.md#", "(c.mara.md#");
    assert_eq!(a_after, a_fixed);
    let d_fixed = d
        .replace(
            "REQ-D title\n===========",
            &format!("<a id=\"{mid_d}\"></a>\n\n# REQ-D title"),
        )
        .replace("| REQ-A |", &format!("| [REQ-A](../a.mara.md#{mid_a}) |"));
    assert_eq!(d_after, d_fixed);
    assert_eq!(
        project.read("docs/e.mara.md"),
        e.replace("| REQ-D |", &format!("| [REQ-D](d/d.mara.md#{mid_d}) |"))
    );
    assert_eq!(
        project.fix().repairs(),
        [],
        "nothing is left for a second run"
    );
}

#[test]
fn writes_the_link_for_a_bare_id_of_one_item() {
    let source = page(
        "A",
        &[item(A, "REQ-A", "| Refines   |  REQ-B |\n", "Body.")],
    );
    let target = page("B", &[item(B, "REQ-B", "", "Body.")]);
    let project = Project::with(&[
        ("docs/a.mara.md", source.clone()),
        ("docs/sub/b.mara.md", target),
    ]);
    assert_eq!(
        project.diagnostics(),
        [at("bare-id-value", "docs/a.mara.md", 13)]
    );
    assert_eq!(
        project.fix().repairs(),
        [at("bare-id-value", "docs/a.mara.md", 13)]
    );
    assert_eq!(
        project.read("docs/a.mara.md"),
        source.replace("|  REQ-B |", &format!("|  [REQ-B](sub/b.mara.md#{B}) |"))
    );
    assert_eq!(project.diagnostics(), []);
}

#[test]
fn rewrites_stale_link_paths_and_relation_text() {
    let source = page(
        "A",
        &[
            item(
                A,
                "REQ-A",
                &format!("| Refines | [REQ-OLD](../b.mara.md#{B}) |\n"),
                &format!("See [the *second* item](../b.mara.md#{B} \"Title\")."),
            ),
            format!("Narrative about [REQ-OLD][r].\n\n[r]: <../b.mara.md#{B}>\n"),
        ],
    );
    let target = page("B", &[item(B, "REQ-B", "", "Body.")]);
    let project = Project::with(&[
        ("docs/one/a.mara.md", source.clone()),
        ("docs/two/b.mara.md", target),
    ]);
    let path = "docs/one/a.mara.md";
    let expected = [
        at("stale-link-path", path, 13),
        at("stale-link-text", path, 13),
        at("stale-link-path", path, 15),
        at("stale-link-path", path, 19),
    ];
    assert_eq!(project.diagnostics(), expected);
    assert_eq!(project.fix().repairs(), expected);
    // The relation link is written as the writer writes it; mention text stays.
    let fixed = source
        .replace(
            &format!("[REQ-OLD](../b.mara.md#{B})"),
            &format!("[REQ-B](../two/b.mara.md#{B})"),
        )
        .replace("(../b.mara.md#", "(../two/b.mara.md#")
        .replace("<../b.mara.md#", "<../two/b.mara.md#");
    assert_eq!(project.read(path), fixed);
    assert!(fixed.contains("[the *second* item](../two/b.mara.md#"));
    assert!(fixed.contains("Narrative about [REQ-OLD][r]."));
    assert_eq!(project.diagnostics(), []);
}

#[test]
fn repairs_malformed_item_formatting() {
    let a = item(A, "REQ-A", "", "Body of A.");
    let b = item(B, "REQ-B", "", "Body of B.").replacen("\n\n###", "\n###", 1);
    let c = item(C, "REQ-C", "", "Body of C.").replacen("### REQ-C", "REQ-C title\n=====", 1);
    let before = format!("# Page\n\nIntro.\n\n{a}\n---\n\n{b}\n---\n\n{c}");
    let project = Project::with(&[("docs/a.mara.md", before.clone())]);
    let path = "docs/a.mara.md";
    let expected = [
        at("missing-separator", path, 5),
        at("merged-anchor", path, 18),
        at("setext-heading", path, 33),
    ];
    assert_eq!(project.diagnostics(), expected);
    assert_eq!(project.fix().repairs(), expected);
    let fixed = before
        .replace("Intro.\n\n<a", "Intro.\n\n---\n\n<a")
        .replace(&format!("{B}\"></a>\n###"), &format!("{B}\"></a>\n\n###"))
        .replace("REQ-C title\n=====", "# REQ-C title");
    assert_eq!(project.read(path), fixed);
    assert_eq!(project.diagnostics(), []);
}

#[test]
fn supplies_no_heading_type_or_id_and_keeps_both_copies() {
    let files = [
        (
            "docs/a.mara.md",
            page(
                "A",
                &[
                    format!("<a id=\"{A}\"></a>\n\nAn anchor without a heading.\n"),
                    item(B, "REQ-B", "", "Body.").replace("| ID | REQ-B |\n", ""),
                    item(C, "REQ-C", "| Refines | REQ-NONE |\n", "Body."),
                    item(
                        D,
                        "REQ-D",
                        &format!("| Refines | [REQ-X](#{MISSING}) |\n"),
                        "Body.",
                    ),
                    format!(
                        "<a id=\"{}\"></a>\n\n### REQ-LOWER\n\n| Type | Requirement |\n\
                         | --- | --- |\n| ID | REQ-LOWER |\n| Status | Draft |\n\nBody.\n",
                        MISSING.to_lowercase()
                    ),
                ],
            ),
        ),
        (
            "docs/copy-1.mara.md",
            page("One", &[item(E, "REQ-E", "", "Body.")]),
        ),
        (
            "docs/copy-2.mara.md",
            page("Two", &[item(E, "REQ-E", "", "Body.")]),
        ),
    ];
    let project = Project::with(&files);
    let diagnostics = project.diagnostics();
    let codes: Vec<&str> = diagnostics.iter().map(|(code, ..)| code.as_str()).collect();
    for code in [
        "incomplete-item",
        "missing-row",
        "bare-id-value",
        "broken-link",
        "duplicate-mid",
    ] {
        assert!(codes.contains(&code), "{code}: {diagnostics:?}");
    }
    let tree = project.tree();
    let run = project.fix();
    assert_eq!(run.repairs(), []);
    assert!(run.stdout.contains("\"repairs_total\":0"), "{run:?}");
    assert_eq!(project.tree(), tree);
    assert_eq!(project.diagnostics(), diagnostics);
}

#[test]
fn rewrites_what_an_interrupted_move_or_rename_leaves_in_several_files() {
    // REQ-OLD moved from `one/` to `two/` and renamed REQ-NEW; the move stopped before the
    // inbound link paths and the rename after its `ID` row (DES-WRITE-PROCEDURE).
    let project = Project::with(&[
        (
            "docs/two/t.mara.md",
            page("T", &[item(D, "REQ-NEW", "", "Body.")]),
        ),
        (
            "docs/one/a.mara.md",
            page(
                "A",
                &[item(
                    A,
                    "REQ-A",
                    &format!("| Refines | [REQ-OLD](t.mara.md#{D}) |\n"),
                    &format!("See [REQ-NEW](t.mara.md#{D})."),
                )],
            ),
        ),
        (
            "docs/two/b.mara.md",
            page(
                "B",
                &[item(
                    B,
                    "REQ-B",
                    &format!("| Refines | [REQ-OLD](../one/t.mara.md#{D}) |\n"),
                    "Body.",
                )],
            ),
        ),
        (
            "docs/three/c.mara.md",
            page(
                "C",
                &[
                    item(
                        C,
                        "REQ-C",
                        &format!("| Refines | [REQ-NEW](../one/t.mara.md#{D}) |\n"),
                        "Body.",
                    ),
                    format!("Narrative: [the moved one](../one/t.mara.md#{D}).\n"),
                ],
            ),
        ),
    ]);
    let stale = |diagnostics: &[(String, String, u64)]| {
        diagnostics
            .iter()
            .filter(|(code, ..)| code.starts_with("stale-link"))
            .count()
    };
    assert_eq!(stale(&project.diagnostics()), 7);
    let repairs = project.fix().repairs();
    assert_eq!(repairs.len(), 7);
    assert_eq!(project.diagnostics(), []);
}

#[test]
fn changes_nothing_in_a_valid_project() {
    let project = Project::with(&[
        (
            "docs/a.mara.md",
            page(
                "A",
                &[item(
                    A,
                    "REQ-A",
                    &format!("| Refines | [REQ-B](b/b.mara.md#{B}) |\n"),
                    "Body.",
                )],
            ),
        ),
        (
            "docs/b/b.mara.md",
            page("B", &[item(B, "REQ-B", "", "Body.")]),
        ),
    ]);
    assert_eq!(project.diagnostics(), []);
    let tree = project.tree();
    assert_eq!(project.fix().repairs(), []);
    let human = Run::of(
        project
            .command_as("human", &["project", "fix"])
            .output()
            .unwrap(),
    );
    assert_eq!(human.code(), Some(0), "{human:?}");
    assert!(human.stdout.starts_with("nothing to fix"), "{human:?}");
    assert_eq!(project.tree(), tree);
}

/// VER-PRESERVED-BYTES: CRLF in one file and LF in another, a byte order mark, front matter,
/// irregular padding in the edited tables, emphasis, trailing spaces, no final newline, and
/// modes 0600 and 0644.
#[test]
fn keeps_every_byte_and_mode_it_does_not_repair() {
    let crlf = format!(
        "\u{feff}---\ntitle: Notes\n---\n# *Notes*  \n\n_Intro_ with trailing spaces.   \n\n---\n\n\
         <a id=\"{A}\"></a>\n\n### REQ-A\n\n|Type|Requirement|\n|:-|-:|\n|  ID   | REQ-A|\n\
         | Status |   Draft   |\n|  Refines   |  REQ-B    |\n\nSee [the **target**](b.mara.md#{B}).  \n"
    )
    .replace('\n', "\r\n");
    let lf = format!(
        "# Other\n\n---\n\n<a id=\"{B}\"></a>\n\n### REQ-B\n\n| Type | Requirement |\n| --- | --- |\n\
         | ID | REQ-B |\n| Status | Draft |\n|Refines|[REQ-A](crlf.mara.md#{A})|\n\nBody *b*.\n\n---\n\n\
         ### REQ-C\n\n| Type   | Requirement |\n|---|---|\n| ID | REQ-C |\n| Status | Draft |\n\nNo final newline."
    );
    let untouched = page("Untouched", &[item(C, "REQ-D", "", "Body.")]);
    let project = Project::with(&[
        ("docs/crlf.mara.md", crlf.clone()),
        ("docs/lf/lf.mara.md", lf.clone()),
        ("docs/untouched.mara.md", untouched),
    ]);
    let set_mode = |path: &str, mode| {
        fs::set_permissions(project.path(path), fs::Permissions::from_mode(mode)).unwrap();
    };
    set_mode("docs/crlf.mara.md", 0o600);
    set_mode("docs/lf/lf.mara.md", 0o644);
    let before = project.tree();
    let repairs = project.fix().repairs();
    assert_eq!(
        repairs,
        [
            at("bare-id-value", "docs/crlf.mara.md", 18),
            at("stale-link-path", "docs/crlf.mara.md", 20),
            at("stale-link-path", "docs/lf/lf.mara.md", 13),
            at("incomplete-item", "docs/lf/lf.mara.md", 19),
        ]
    );
    let crlf_fixed = crlf
        .replace(
            "|  REQ-B    |",
            &format!("|  [REQ-B](lf/lf.mara.md#{B})    |"),
        )
        .replace("(b.mara.md#", "(lf/lf.mara.md#");
    assert_eq!(project.read("docs/crlf.mara.md"), crlf_fixed);
    let lf_after = project.read("docs/lf/lf.mara.md");
    let mid = new_mid(&lf, &lf_after);
    let lf_fixed = lf
        .replace("(crlf.mara.md#", "(../crlf.mara.md#")
        .replace("### REQ-C", &format!("<a id=\"{mid}\"></a>\n\n### REQ-C"));
    assert_eq!(lf_after, lf_fixed);
    let after = project.tree();
    let mode = |tree: &BTreeMap<String, Entry>, path: &str| tree[path].as_ref().unwrap().1;
    assert_eq!(mode(&after, "docs/crlf.mara.md"), 0o600);
    assert_eq!(mode(&after, "docs/lf/lf.mara.md"), 0o644);
    let changed: Vec<&str> = after
        .keys()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(String::as_str)
        .collect();
    assert_eq!(changed, ["docs/crlf.mara.md", "docs/lf/lf.mara.md"]);
    assert_eq!(after.len(), before.len());
    assert_eq!(project.diagnostics(), []);
}

#[test]
fn refuses_a_project_whose_configuration_or_schema_has_errors() {
    for (path, text) in [
        (
            ".mara/config.toml",
            CONFIG.replacen("format_version = 1", "format_version = 2", 1),
        ),
        (
            ".mara/schema.toml",
            SCHEMA.replacen("format_version = 1", "format_version = 2", 1),
        ),
    ] {
        let project = Project::with(&[(
            "docs/a.mara.md",
            page("A", &[item(A, "REQ-A", "| Refines | REQ-A |\n", "Body.")]),
        )]);
        project.write(path, &text);
        let tree = project.tree();
        let run = project.fix();
        assert_eq!(
            (run.code(), run.error()),
            (Some(1), "invalid-project".to_owned()),
            "{run:?}"
        );
        assert_eq!(project.tree(), tree, "{path}");
    }
}

#[test]
fn rejects_undeclared_arguments() {
    let project = Project::with(&[("docs/a.mara.md", page("A", &[]))]);
    for args in [
        &["project", "fix", "--path", "docs"][..],
        &["project", "fix", "docs"],
    ] {
        let run = project.run(args);
        assert_eq!(
            (run.code(), run.error()),
            (Some(2), "invalid-argument".to_owned()),
            "{run:?}"
        );
    }
}

/// A `project fix` that stops at its pause points (DES-WRITE-PROCEDURE) until the test lets it
/// go on.
struct Paused {
    child: Option<Child>,
    dir: tempfile::TempDir,
}

impl Paused {
    fn start(project: &Project) -> Paused {
        Paused::run(project, &["project", "fix"])
    }

    /// The write `args`, stopping at its pause points.
    fn run(project: &Project, args: &[&str]) -> Paused {
        let dir = tempfile::Builder::new()
            .prefix("mara-pause-")
            .tempdir()
            .expect("create a temporary directory");
        let child = project
            .command(args)
            .env("MARA_TEST_PAUSE", dir.path())
            .spawn()
            .expect("start mara");
        Paused {
            child: Some(child),
            dir,
        }
    }

    /// Waits until the writer stops, and returns what `paused` holds: nothing before step 2, the
    /// file's path before its second check. `None` when the process ended instead.
    fn stop(&mut self) -> Option<String> {
        let paused = self.dir.path().join("paused");
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Ok(text) = fs::read_to_string(&paused) {
                return Some(text);
            }
            let child = self.child.as_mut().unwrap();
            if child.try_wait().unwrap().is_some() {
                return None;
            }
            assert!(Instant::now() < deadline, "mara did not pause");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn resume(&self) {
        fs::remove_file(self.dir.path().join("paused")).unwrap();
    }

    fn finish(mut self) -> Run {
        Run::of(self.child.take().unwrap().wait_with_output().unwrap())
    }
}

/// The files that a fix of a copy of `files` replaces, in the order it replaces them, with their
/// new contents.
fn uninterrupted(files: &[(&str, String)]) -> Vec<(String, String)> {
    let project = Project::with(files);
    let mut paused = Paused::start(&project);
    let mut order = Vec::new();
    while let Some(path) = paused.stop() {
        if !path.is_empty() {
            order.push(path);
        }
        paused.resume();
    }
    let run = paused.finish();
    assert_eq!(run.code(), Some(0), "{run:?}");
    order
        .into_iter()
        .map(|path| {
            let text = project.read(&path);
            (path, text)
        })
        .collect()
}

/// Three documents in three directories, each with stale links into the others.
fn three_directories() -> Vec<(&'static str, String)> {
    vec![
        (
            "docs/one/a.mara.md",
            page(
                "A",
                &[item(
                    A,
                    "REQ-A",
                    &format!("| Refines | [REQ-B](b.mara.md#{B}) |\n"),
                    &format!("See [C](c.mara.md#{C})."),
                )],
            ),
        ),
        (
            "docs/two/b.mara.md",
            page(
                "B",
                &[item(
                    B,
                    "REQ-B",
                    &format!("| Refines | [REQ-C](c.mara.md#{C}) |\n"),
                    &format!("See [A](a.mara.md#{A})."),
                )],
            ),
        ),
        (
            "docs/three/c.mara.md",
            page(
                "C",
                &[item(C, "REQ-C", "", &format!("See [B](b.mara.md#{B})."))],
            ),
        ),
    ]
}

/// Every entry of the tree except the files named, which are compared on their own.
fn others(tree: &BTreeMap<String, Entry>, except: &[&str]) -> BTreeMap<String, Entry> {
    tree.iter()
        .filter(|(path, _)| !except.contains(&path.as_str()))
        .map(|(path, entry)| (path.clone(), entry.clone()))
        .collect()
}

#[test]
fn refuses_when_a_file_changes_at_the_pause_before_step_2() {
    let single = vec![(
        "docs/a.mara.md",
        page("A", &[item(A, "REQ-A", "| Refines | REQ-A |\n", "Body.")]),
    )];
    let multi = three_directories();
    let order = uninterrupted(&multi);
    assert_eq!(
        order
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        [
            "docs/one/a.mara.md",
            "docs/three/c.mara.md",
            "docs/two/b.mara.md"
        ]
    );
    for (files, changed) in [
        (&single, "docs/a.mara.md"),
        (&multi, "docs/one/a.mara.md"),
        (&multi, "docs/two/b.mara.md"),
    ] {
        let project = Project::with(files);
        let before = project.tree();
        let mut paused = Paused::start(&project);
        assert_eq!(paused.stop().as_deref(), Some(""));
        let text = project.read(changed) + "\nChanged by another program.\n";
        project.write(changed, &text);
        paused.resume();
        let run = paused.finish();
        assert_eq!(
            (run.code(), run.error()),
            (Some(1), "changed-file".to_owned()),
            "{run:?}"
        );
        assert!(run.stdout.contains(changed), "{run:?}");
        let after = project.tree();
        assert_eq!(project.read(changed), text);
        assert_eq!(
            others(&after, &[changed]),
            others(&before, &[changed]),
            "{changed}"
        );
    }
}

#[test]
fn refuses_when_the_first_file_changes_at_its_pause_before_its_rename() {
    let multi = three_directories();
    let first = "docs/one/a.mara.md";
    let project = Project::with(&multi);
    let before = project.tree();
    let mut paused = Paused::start(&project);
    assert_eq!(paused.stop().as_deref(), Some(""));
    paused.resume();
    assert_eq!(paused.stop().as_deref(), Some(first));
    let text = project.read(first) + "\nChanged by another program.\n";
    project.write(first, &text);
    paused.resume();
    let run = paused.finish();
    assert_eq!(
        (run.code(), run.error()),
        (Some(1), "changed-file".to_owned()),
        "{run:?}"
    );
    assert_eq!(project.read(first), text);
    // Nothing replaced and no temporary file left.
    assert_eq!(others(&project.tree(), &[first]), others(&before, &[first]));
}

/// The ways VER-INTERRUPTED-EDITS stops an edit at a file.
#[derive(Clone, Copy, Debug)]
enum Stop {
    Killed,
    WriteFails,
    Changed,
    RenameFails,
}

#[test]
fn an_interrupted_fix_leaves_each_file_old_or_new_and_reports_what_is_left() {
    let owner = tempfile::tempfile().unwrap().metadata().unwrap().uid();
    assert_ne!(
        owner, 0,
        "the case \"Rename fails\" needs a user without CAP_DAC_OVERRIDE: run the tests as \
         another user than root"
    );
    let base = three_directories();
    for target in 0..base.len() {
        // The file stopped at is padded so that its new contents are the largest.
        let mut files = base.clone();
        files[target]
            .1
            .push_str(&format!("\n---\n\n{}\n", "Padding text. ".repeat(400)));
        let target_path = files[target].0;
        let order = uninterrupted(&files);
        let position = order
            .iter()
            .position(|(path, _)| path == target_path)
            .unwrap();
        let largest_other = order
            .iter()
            .filter(|(path, _)| path != target_path)
            .map(|(_, text)| text.len())
            .max()
            .unwrap();
        let limit = (largest_other + order[position].1.len()) / 2;
        let diagnostics = Project::with(&files).diagnostics();
        for stop in [
            Stop::Killed,
            Stop::WriteFails,
            Stop::Changed,
            Stop::RenameFails,
        ] {
            if matches!(stop, Stop::Changed) && position == 0 {
                // VER-REFUSED-EDITS covers the first file.
                continue;
            }
            let name = format!("{stop:?} at {target_path}");
            let project = Project::with(&files);
            let mut changed = None;
            let run = match stop {
                Stop::Killed | Stop::WriteFails => {
                    let mut command = project.command(&["project", "fix"]);
                    let ignore = matches!(stop, Stop::WriteFails);
                    let limit = limit as libc::rlim_t;
                    // SAFETY: between fork and exec the closure only calls `setrlimit` and
                    // `signal`, which are async-signal-safe.
                    unsafe {
                        command.pre_exec(move || {
                            let rlimit = libc::rlimit {
                                rlim_cur: limit,
                                rlim_max: limit,
                            };
                            if libc::setrlimit(libc::RLIMIT_FSIZE, &rlimit) != 0 {
                                return Err(std::io::Error::last_os_error());
                            }
                            if ignore {
                                libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
                            }
                            Ok(())
                        });
                    }
                    Run::of(command.output().unwrap())
                }
                Stop::Changed | Stop::RenameFails => {
                    let mut paused = Paused::start(&project);
                    while let Some(path) = paused.stop() {
                        if path == target_path {
                            if matches!(stop, Stop::Changed) {
                                let text = project.read(target_path) + "\nChanged.\n";
                                project.write(target_path, &text);
                                changed = Some(text);
                            } else {
                                let directory = project.path(target_path);
                                let directory = directory.parent().unwrap();
                                fs::set_permissions(directory, fs::Permissions::from_mode(0o555))
                                    .unwrap();
                            }
                        }
                        paused.resume();
                    }
                    let run = paused.finish();
                    let directory = project.path(target_path);
                    fs::set_permissions(
                        directory.parent().unwrap(),
                        fs::Permissions::from_mode(0o755),
                    )
                    .unwrap();
                    run
                }
            };
            match stop {
                Stop::Killed => {
                    assert_eq!(run.status.signal(), Some(libc::SIGXFSZ), "{name}: {run:?}");
                }
                _ => {
                    assert_eq!(run.code(), Some(1), "{name}: {run:?}");
                    assert!(
                        run.json()["error"]["diagnostics"][0]["path"] == target_path,
                        "{name}: {run:?}"
                    );
                }
            }
            // The files before the one stopped at hold their new contents, the others their
            // former ones, or the other program's.
            let former: BTreeMap<&str, &String> = files.iter().map(|(p, t)| (*p, t)).collect();
            for (index, (path, new)) in order.iter().enumerate() {
                let expected = match index.cmp(&position) {
                    std::cmp::Ordering::Less => new,
                    std::cmp::Ordering::Equal => changed.as_ref().unwrap_or(former[path.as_str()]),
                    std::cmp::Ordering::Greater => former[path.as_str()],
                };
                assert_eq!(&project.read(path), expected, "{name}: {path}");
            }
            if matches!(stop, Stop::WriteFails) {
                let leftovers = project
                    .tree()
                    .into_keys()
                    .filter(|path| path.contains(".mara-tmp-"))
                    .count();
                assert_eq!(leftovers, 0, "{name}");
            }
            // Validation reports the stale links left, and nothing new.
            let replaced: Vec<&str> = order[..position].iter().map(|(p, _)| p.as_str()).collect();
            let left: Vec<_> = diagnostics
                .iter()
                .filter(|(_, path, _)| !replaced.contains(&path.as_str()))
                .cloned()
                .collect();
            assert_eq!(project.diagnostics(), left, "{name}");
        }
    }
}

/// VER-CONCURRENT-WRITERS: a fix started while another writer holds the lock waits, then
/// applies to what that writer left.
#[test]
fn waits_for_the_write_lock_and_fixes_what_it_finds_then() {
    let project = Project::with(&[
        (
            "docs/a.mara.md",
            page("A", &[item(A, "REQ-A", "| Refines | REQ-B |\n", "Body.")]),
        ),
        (
            "docs/b.mara.md",
            page("B", &[item(B, "REQ-B", "", "Body.")]),
        ),
    ]);
    let before = project.tree();
    let lock = mara::write::WriteLock::take(&project.root).unwrap();
    let mut child = project.command(&["project", "fix"]).spawn().unwrap();
    thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "mara waits for the lock"
    );
    assert_eq!(project.tree(), before);
    let changed = project.read("docs/a.mara.md") + "\nAdded while the lock was held.\n";
    project.write("docs/a.mara.md", &changed);
    drop(lock);
    let run = Run::of(child.wait_with_output().unwrap());
    assert_eq!(run.repairs(), [at("bare-id-value", "docs/a.mara.md", 13)]);
    assert_eq!(
        project.read("docs/a.mara.md"),
        changed.replace("| REQ-B |", &format!("| [REQ-B](b.mara.md#{B}) |"))
    );
}

/// A real `item rename` stopped after its `ID` row leaves stale relation link text
/// (VER-INTERRUPTED-EDITS), which one fix rewrites.
#[test]
fn fixes_what_a_rename_stopped_after_its_id_row_leaves() {
    let refines = |path: &str| format!("| Refines | [REQ-OLD]({path}#{D}) |\n");
    let project = Project::with(&[
        (
            "docs/t.mara.md",
            page("T", &[item(D, "REQ-OLD", "", "Body.")]),
        ),
        (
            "docs/x/a.mara.md",
            page("A", &[item(A, "REQ-A", &refines("../t.mara.md"), "Body.")]),
        ),
        (
            "docs/y/b.mara.md",
            page("B", &[item(B, "REQ-B", &refines("../t.mara.md"), "Body.")]),
        ),
    ]);
    let mut paused = Paused::run(&project, &["item", "rename", "REQ-OLD", "REQ-NEW"]);
    let mut stops = Vec::new();
    while let Some(path) = paused.stop() {
        // Another program changes the first file after the `ID` row, so the rename stops there.
        if path == "docs/x/a.mara.md" {
            let text = project.read(&path) + "\nChanged.\n";
            project.write(&path, &text);
        }
        stops.push(path);
        paused.resume();
    }
    let run = paused.finish();
    assert_eq!(
        (run.code(), run.error()),
        (Some(1), "changed-file".to_owned()),
        "{run:?}"
    );
    assert_eq!(stops, ["", "docs/t.mara.md", "docs/x/a.mara.md"]);
    assert_eq!(
        project.diagnostics(),
        [
            at("stale-link-text", "docs/x/a.mara.md", 13),
            at("stale-link-text", "docs/y/b.mara.md", 13),
        ]
    );
    assert_eq!(project.fix().repairs().len(), 2);
    assert_eq!(project.diagnostics(), []);
    assert!(
        project
            .read("docs/y/b.mara.md")
            .contains("[REQ-NEW](../t.mara.md#")
    );
}
