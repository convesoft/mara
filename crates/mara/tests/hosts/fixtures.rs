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
