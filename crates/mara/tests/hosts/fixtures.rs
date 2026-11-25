//! The fixture projects of the Node and browser modules, written into the test's directory
//! ([`Host::fixture`]) with the CLI where it writes them.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::node::Host;

/// A schema with two types and a relation between them, and the relation of code markers.
const SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted", "retired"]

[types.need]
id_prefix = "NEED-"
description = "A stakeholder capability."

[types.requirement]
id_prefix = "REQ-"
description = "One verifiable obligation."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"] }

[relations.implements]
inverse = "implemented_by"
description = "The code realizes the target."
targets = { code = ["requirement"] }
"#;

/// VER-BROWSER-WORKSPACE's project: `project init` of `empty`, [`SCHEMA`], items in two
/// documents, no program, and a `.git` that the workspace is not given.
pub fn workspace(host: &Host) {
    let root = host.fixture("workspace");
    fs::create_dir_all(&root).unwrap();
    mara(host, &root, &["project", "init", "--template", "empty"]);
    write(&root, ".mara/schema.toml", SCHEMA);
    let create = |kind: &str, path: &str, title: &str, body: &str| {
        let args = [
            "item",
            "create",
            kind,
            path,
            "--title",
            title,
            "--field",
            "status=draft",
            "--body",
            body,
        ];
        mara(host, &root, &args);
    };
    create(
        "need",
        "docs/needs.mara.md",
        "Fast answers",
        "Agents get answers quickly.",
    );
    create(
        "need",
        "docs/needs.mara.md",
        "Offline work",
        "People work without a network.",
    );
    create(
        "requirement",
        "docs/reqs.mara.md",
        "Cold read time",
        "A cold read takes at most 200 ms.",
    );
    create(
        "requirement",
        "docs/reqs.mara.md",
        "Warm read time",
        "A warm read takes at most 20 ms.",
    );
    write(&root, ".git/HEAD", "ref: refs/heads/main\n");
    host.hash("workspace");
}

/// The fixture grammars, as VER-GRAMMAR-MARKERS pins them (`tests/fixtures/grammars`): the
/// language, the version and the module's file.
const GRAMMARS: [(&str, &str, &str); 2] = [
    ("rust", "0.24.2", "tree-sitter-rust.wasm"),
    ("python", "0.25.0", "tree-sitter-python.wasm"),
];

/// The Rust file of VER-GRAMMAR-MARKERS (`tests/code/grammar.rs`): markers before declarations,
/// inside and after code, in each kind of comment, and marker-like lines in strings.
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

/// The Python file of VER-GRAMMAR-MARKERS: markers before a function and a class, and inside a
/// method.
const PYTHON: &str = "#!/usr/bin/env python3\n# @mara implements REQ-LOGIN\ndef login():\n    \
                      pass\n\n\n# @mara implements REQ-AUDIT\nclass Audit:\n    \
                      def record(self):\n        # @mara implements REQ-LOGIN-AUDIT\n        \
                      return 1\n";

/// The project of the fixtures of VER-GRAMMAR-MARKERS with both grammars pinned, and no
/// program: `empty`, [`SCHEMA`], the requirements that the markers name, the Rust and Python
/// files, and each language's pin of its fixture grammar, which the per-user cache of the CLI's
/// runs, under the test's directory, does not hold.
pub fn grammars(host: &Host) {
    let root = host.fixture("grammars");
    fs::create_dir_all(&root).unwrap();
    mara(host, &root, &["project", "init", "--template", "empty"]);
    write(&root, ".mara/schema.toml", SCHEMA);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grammars");
    let mut config = fs::read_to_string(root.join(".mara/config.toml")).unwrap();
    config.push_str("\n[code]\nformat_version = 1\n");
    for (language, version, wasm) in GRAMMARS {
        let wasm = fs::read(fixtures.join(language).join(wasm)).unwrap();
        let tags = fs::read(fixtures.join(language).join("tags.scm")).unwrap();
        let checksum = mara::grammar::checksum(&wasm, &tags);
        let (include, prefixes) = match language {
            "rust" => (r#"["src/**/*.rs"]"#, r#"["//", "///", "*"]"#),
            _ => (r#"["scripts/**/*.py"]"#, r##"["#"]"##),
        };
        config.push_str(&format!(
            "\n[code.languages.{language}]\ninclude = {include}\ncomment_prefixes = \
             {prefixes}\ngrammar = {{ version = \"{version}\", checksum = \"blake3:{checksum}\" }}\n"
        ));
    }
    write(&root, ".mara/config.toml", &config);
    for (title, body) in [
        ("Login", "Users log in."),
        ("Audit", "Log-ins are audited."),
        ("Login audit", "Audits log in."),
    ] {
        let id = format!("REQ-{}", title.to_uppercase().replace(' ', "-"));
        let args = [
            "item",
            "create",
            "requirement",
            "docs/reqs.mara.md",
            "--id",
            &id,
            "--title",
            title,
            "--field",
            "status=draft",
            "--body",
            body,
        ];
        mara(host, &root, &args);
    }
    write(&root, "src/login.rs", RUST);
    write(&root, "scripts/tool.py", PYTHON);
    host.hash("grammars");
}

/// The 0.3.0 input of the performance harness ([`crate::legacy_project`]), `.git` included.
pub fn legacy(host: &Host) {
    let root = host.fixture("legacy");
    for (path, contents) in crate::legacy_project::FILES {
        write(&root, path, contents);
    }
}

/// The `n`th MID of the programs' and facts' fixtures, as `tests/programs` and `tests/facts`
/// number them.
fn mid(n: u32) -> String {
    format!("01M7D8DZ2663WP3A03000000{n:02}")
}

/// An item of those fixtures: its anchor, heading, metadata table and body.
fn item(
    n: u32,
    kind: &str,
    id: &str,
    status: &str,
    title: &str,
    rows: &[(&str, String)],
) -> String {
    let mut text = format!(
        "<a id=\"{}\"></a>\n\n### {title}\n\n| Type | {kind} |\n| --- | --- |\n| ID | {id} |\n\
         | Status | {status} |\n",
        mid(n)
    );
    for (key, value) in rows {
        text.push_str(&format!("| {key} | {value} |\n"));
    }
    text.push_str("\nThe body.\n");
    text
}

/// The project of the rules Verification items' tests (`tests/programs`, VER-PROGRAM-CONSUMERS):
/// documents below `docs/`, programs below `.mara/rules/`, and in `docs/a.mara.md` `REQ-A`
/// (MID 1, accepted, owner `abc`, satisfying `NEED-ONE`), `REQ-B` (MID 2, draft, titled
/// `Héllo`) and `NEED-ONE` (MID 3, accepted). The browser cases write their programs and
/// entries into it.
pub fn programs(host: &Host) {
    let root = host.fixture("programs");
    write(&root, ".mara/schema.toml", PROGRAMS_SCHEMA);
    write(&root, ".mara/config.toml", &programs_config(""));
    let items = [
        item(
            1,
            "Requirement",
            "REQ-A",
            "Accepted",
            "Alpha",
            &[
                ("Owner", "abc".to_owned()),
                ("Satisfies", format!("[NEED-ONE](#{})", mid(3))),
            ],
        ),
        item(2, "Requirement", "REQ-B", "Draft", "Héllo", &[]),
        item(3, "Need", "NEED-ONE", "Accepted", "One", &[]),
    ];
    let text = format!("# A\n\n---\n\n{}", items.join("\n---\n\n"));
    write(&root, "docs/a.mara.md", &text);
    host.hash("programs");
}

/// The schema of [`programs`].
const PROGRAMS_SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.owner]
kind = "text"

[types.need]
id_prefix = "NEED-"
description = "A need."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"] }
"#;

/// The configuration of [`programs`] followed by `entries`.
fn programs_config(entries: &str) -> String {
    format!(
        "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\n\
         include = [\"docs/**/*.mara.md\"]\n\n[rules]\nprograms = [\".mara/rules/**/*.dl\"]\n\
         {entries}"
    )
}

/// The project of VER-FACT-ROWS (`tests/facts`): two requirements with fields and relation rows
/// in `docs/a.mara.md`, a title `ÉCOLE` with a combining accent and a title and a field value
/// with a tab, a quote, a backslash and non-ASCII text; a need and a requirement without fields
/// in `docs/b.mara.md`.
pub fn facts(host: &Host) {
    let root = host.fixture("facts");
    let config = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\n\
                  format_version = 1\ninclude = [\"**/*.mara.md\"]\n";
    write(&root, ".mara/config.toml", config);
    write(&root, ".mara/schema.toml", FACTS_SCHEMA);
    let link = |path: &str, n: u32| format!("[x]({path}#{})", mid(n));
    let mentioned = |text: String| {
        text.replace(
            "\nThe body.\n",
            &format!("\nMentions [one](b.mara.md#{}).\n", mid(3)),
        )
    };
    let a = [
        item(
            1,
            "Requirement",
            "REQ-A",
            "Accepted",
            "E\u{301}COLE",
            &[
                ("Priority", "Very High".to_owned()),
                ("Owner", "a\tb \"q\" back\\slash \u{fc}".to_owned()),
                ("Satisfies", link("b.mara.md", 3)),
            ],
        ),
        item(
            2,
            "Requirement",
            "REQ-B",
            "Draft",
            "Tab\there \"q\" back\\\\slash \u{fc}",
            &[("Owner", "Ann".to_owned()), ("Refines", link("", 1))],
        ),
    ];
    let b = [
        item(3, "Need", "NEED-ONE", "Accepted", "One", &[]),
        item(4, "Requirement", "REQ-C", "Draft", "Without result", &[]),
    ];
    write(&root, "docs/a.mara.md", &a.map(mentioned).join("\n---\n\n"));
    write(&root, "docs/b.mara.md", &b.map(mentioned).join("\n---\n\n"));
    host.hash("facts");
}

const FACTS_SCHEMA: &str = r#"format_version = 1
statuses = ["draft", "accepted"]

[types.requirement]
id_prefix = "REQ-"
description = "An obligation."

[types.requirement.fields.priority]
kind = "enum"
values = ["very_high", "low"]

[types.requirement.fields.owner]
kind = "text"
repeatable = true

[types.need]
id_prefix = "NEED-"
description = "A need."

[relations.satisfies]
inverse = "satisfied_by"
description = "The source fulfils the target."
targets = { requirement = ["need"] }

[relations.refines]
inverse = "refined_by"
description = "The source details the target."
targets = { requirement = ["requirement"] }
"#;

/// Copies the files below `from` to `to`.
pub fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// Runs `mara <args>` in direct mode in `directory`, with the test's directory as `HOME`, which
/// must succeed.
fn mara(host: &Host, directory: &Path, args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .env("HOME", host.path())
        .output()
        .expect("start mara");
    assert!(output.status.success(), "mara {args:?}: {output:?}");
}
