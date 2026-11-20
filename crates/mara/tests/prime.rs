#![cfg(unix)]

//! `mara prime` through the real binary in disposable directories: the CLI side of
//! VER-AGENT-GUIDE in `docs/verification.mara.md`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

/// The heading that opens the full guide (DES-AGENT-GUIDE).
const HEADING: &str = "# Mara workflow\n";
/// Its sections, in order.
const SECTIONS: [&str; 4] = [
    "## Find and read",
    "## Author",
    "## Repair after hand edits",
    "## Validate",
];
/// The operations the full guide leaves to the notes, `prime` itself, the MCP server and the
/// daemon's control (DES-AGENT-GUIDE).
const NOT_IN_GUIDE: [&str; 7] = [
    "project init",
    "project convert",
    "prime",
    "mcp",
    "daemon status",
    "daemon start",
    "daemon stop",
];

/// A disposable directory under `$TMPDIR`, removed on drop.
struct Dir {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Dir {
    fn new() -> Dir {
        let dir = tempfile::Builder::new()
            .prefix("mara-prime-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        Dir { _dir: dir, root }
    }

    /// A directory initialized with `template`.
    fn project(template: &str) -> Dir {
        let dir = Dir::new();
        run(&dir.root, &["project", "init", "--template", template]);
        dir
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

/// Runs `mara <args>` in `directory` with an empty environment, requires exit status 0 and an
/// empty stderr, and returns stdout.
fn run(directory: &Path, args: &[&str]) -> String {
    let (status, stdout, stderr) = run_any(directory, args);
    assert_eq!(
        (status, stderr.as_str()),
        (0, ""),
        "mara {args:?} in {}; stdout:\n{stdout}",
        directory.display()
    );
    stdout
}

/// Runs `mara <args>` in `directory` with an empty environment: exit status, stdout, stderr.
fn run_any(directory: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .stdin(Stdio::null())
        .output()
        .expect("run mara");
    (
        output.status.code().expect("mara exits"),
        String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    )
}

/// The arguments of the command in the first code span of the guide's line that holds
/// `marker`, such as `["project", "validate"]` for `mara project validate`.
fn command_after(guide: &str, marker: &str) -> Vec<String> {
    let line = (guide.lines().find(|line| line.contains(marker)))
        .unwrap_or_else(|| panic!("no line with {marker:?}:\n{guide}"));
    let after = &line[line.find(marker).unwrap()..];
    let command = (after.split('`').skip(1).step_by(2))
        .find_map(|span| span.strip_prefix("mara "))
        .unwrap_or_else(|| panic!("no command after {marker:?}: {line}"));
    command.split(' ').map(str::to_owned).collect()
}

/// The operations and options that `text` names: each code span that starts with `mara `,
/// read up to its first argument or option, and each `--<name>`.
fn names(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut operations = BTreeSet::new();
    for (index, span) in text.split('`').enumerate() {
        if index % 2 == 0 {
            continue;
        }
        let Some(command) = span.strip_prefix("mara ") else {
            continue;
        };
        let words: Vec<&str> = (command.split(' '))
            .take_while(|word| !word.starts_with(['<', '[', '-']))
            .collect();
        if !words.is_empty() {
            operations.insert(words.join(" "));
        }
    }
    let mut options = BTreeSet::new();
    for (at, _) in text.match_indices("--") {
        let name: String = text[at + 2..]
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
            .collect();
        if name.starts_with(|c: char| c.is_ascii_lowercase()) && !text[..at].ends_with('-') {
            options.insert(name);
        }
    }
    (operations, options)
}

/// The operations and options that the CLI declares, from its usage.
fn declared() -> (BTreeSet<String>, BTreeSet<String>) {
    let usage = run(&Dir::new().root, &[]);
    let (_, options) = names(&usage);
    let operations = (usage.lines())
        .filter_map(|line| line.strip_prefix("  mara "))
        .map(|line| {
            let words: Vec<&str> = (line.split(' '))
                .take_while(|word| !word.starts_with(['<', '[']))
                .collect();
            words.join(" ")
        })
        .collect();
    (operations, options)
}

fn guide() -> String {
    run(&Dir::project("engineering").root, &["prime"])
}

#[test]
fn a_project_gets_the_full_guide_wherever_it_is_found() {
    let dir = Dir::project("engineering");
    let guide = run(&dir.root, &["prime"]);
    assert!(guide.starts_with(HEADING), "{guide}");
    let mut at = 0;
    for section in SECTIONS {
        let found = guide[at..].find(&format!("\n{section}\n"));
        at += found.unwrap_or_else(|| panic!("no section {section:?} in order:\n{guide}"));
    }
    dir.write("docs/sub/a.mara.md", "Narrative.\n");
    assert_eq!(run(&dir.root.join("docs/sub"), &["prime"]), guide);
    let outside = Dir::new();
    let named = dir.root.to_str().unwrap();
    assert_eq!(run(&outside.root, &["prime", "--project", named]), guide);
    assert_eq!(run(&outside.root, &["--project", named, "prime"]), guide);
}

#[test]
fn the_guide_names_only_what_the_cli_declares_and_every_operation_in_a_project() {
    let guide = guide();
    let (operations, options) = names(&guide);
    let (declared_operations, declared_options) = declared();
    let undeclared: Vec<_> = operations.difference(&declared_operations).collect();
    assert!(
        undeclared.is_empty(),
        "undeclared operations {undeclared:?}"
    );
    let undeclared: Vec<_> = options.difference(&declared_options).collect();
    assert!(undeclared.is_empty(), "undeclared options {undeclared:?}");
    let missing: Vec<_> = (declared_operations.iter())
        .filter(|operation| !NOT_IN_GUIDE.contains(&operation.as_str()))
        .filter(|operation| !operations.contains(*operation))
        .collect();
    assert!(
        missing.is_empty(),
        "operations the guide leaves out: {missing:?}"
    );
    for operation in NOT_IN_GUIDE {
        assert!(!operations.contains(operation), "{operation} in the guide");
    }
    // The scan reads an option with a dash in its name.
    assert!(declared_options.contains("clear-field") && options.contains("clear-field"));
}

#[test]
fn outside_every_project_only_initialization_is_named() {
    let outside = Dir::new();
    let empty = Dir::new();
    let empty_path = empty.root.to_str().unwrap();
    let note = run(&outside.root, &["prime"]);
    assert_eq!(
        run(&outside.root, &["prime", "--project", empty_path]),
        note
    );
    assert!(!note.contains(HEADING.trim_end()), "{note}");
    let (operations, options) = names(&note);
    assert_eq!(
        operations,
        BTreeSet::from(["project init".to_owned()]),
        "{note}"
    );
    let (declared_operations, declared_options) = declared();
    assert!(declared_operations.contains("project init"));
    assert!(options.is_subset(&declared_options), "{options:?}");
    // The note's command creates the project that the guide then covers.
    run(&outside.root, &["project", "init"]);
    assert_eq!(run(&outside.root, &["prime"]), guide());
}

#[test]
fn a_legacy_project_gets_only_its_conversion() {
    let dir = Dir::new();
    dir.write(".mara/project.toml", "");
    let note = run(&dir.root, &["prime"]);
    assert!(!note.contains(HEADING.trim_end()), "{note}");
    let (operations, _) = names(&note);
    assert_eq!(
        operations,
        BTreeSet::from(["project convert".to_owned()]),
        "{note}"
    );
}

#[test]
fn the_guide_depends_on_no_schema_and_no_valid_configuration() {
    let guide = guide();
    assert_eq!(run(&Dir::project("empty").root, &["prime"]), guide);
    let broken = Dir::project("engineering");
    broken.write(".mara/config.toml", "format_version = 2\nunknown = [\n");
    broken.write(".mara/schema.toml", "not toml at all = = =\n");
    assert_eq!(run(&broken.root, &["prime"]), guide);
}

#[test]
fn json_holds_the_same_text_in_one_object() {
    let dir = Dir::project("engineering");
    let stdout = run(&dir.root, &["prime", "--format", "json"]);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    let object: Value = serde_json::from_str(&stdout).expect("one JSON object");
    assert_eq!(object["format_version"], 1);
    assert_eq!(
        object["text"].as_str(),
        Some(run(&dir.root, &["prime"]).as_str())
    );
    assert_eq!(object.as_object().unwrap().len(), 2, "{object}");
}

/// The review's page: a title and an introduction, then the guide's hand-written example, the
/// way the guide says to write it, and the example starting a document. One `project fix` makes
/// both items that `item list` lists.
#[test]
fn an_item_written_by_hand_after_a_title_and_introduction_is_listed_after_one_fix() {
    let dir = Dir::project("engineering");
    let guide = run(&dir.root, &["prime"]);
    let start = guide
        .find("```markdown\n")
        .expect("the hand-written example")
        + 12;
    let example = &guide[start..start + guide[start..].find("```\n").unwrap()];
    assert!(example.starts_with("---\n"), "{example}");
    let need = [
        "item",
        "create",
        "need",
        "docs/needs.mara.md",
        "--title",
        "Sign in",
        "--id",
        "NEED-SIGN-IN",
        "--field",
        "status=draft",
        "--body",
        "Users sign in.",
    ];
    run(&dir.root, &need);
    // The document written by hand: no item before the example, only narrative.
    dir.write(
        "docs/auth.mara.md",
        &format!("# Authentication\n\nHow users sign in.\n\n{example}"),
    );
    // The one exception the guide allows: an item that starts the document has no `---`.
    let first = example
        .strip_prefix("---\n\n")
        .unwrap()
        .replace("REQ-PASSKEYS", "REQ-FIRST");
    dir.write("docs/first.mara.md", &first);
    run(&dir.root, &["project", "fix"]);
    let list = run(&dir.root, &["--format", "json", "item", "list"]);
    let list: Value = serde_json::from_str(&list).unwrap();
    let ids: Vec<&str> = (list["items"].as_array().unwrap().iter())
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["REQ-PASSKEYS", "REQ-FIRST", "NEED-SIGN-IN"], "{list}");
    let relations = run(&dir.root, &["relation", "list", "REQ-PASSKEYS"]);
    assert!(relations.contains("NEED-SIGN-IN"), "{relations}");
    run(&dir.root, &["project", "validate"]);
}

/// The review's configuration error: the command the guide names after `invalid-project`
/// reports it at its line.
#[test]
fn the_command_the_guide_names_for_invalid_project_reports_a_configuration_error() {
    let dir = Dir::project("engineering");
    let guide = run(&dir.root, &["prime"]);
    let config = fs::read_to_string(dir.root.join(".mara/config.toml")).unwrap();
    let broken = config.replace(
        "include = [\"docs/**/*.mara.md\"]",
        "include = \"docs/**/*.mara.md\"",
    );
    assert_ne!(broken, config);
    dir.write(".mara/config.toml", &broken);
    let line = 1 + broken
        .lines()
        .position(|l| l.starts_with("include"))
        .unwrap();
    let (status, _, stderr) = run_any(&dir.root, &["item", "list"]);
    assert!(
        status == 1 && stderr.contains("error[invalid-project]"),
        "{stderr}"
    );
    let command = command_after(&guide, "`invalid-project`");
    let args: Vec<&str> = command.iter().map(String::as_str).collect();
    let (status, stdout, _) = run_any(&dir.root, &args);
    assert_eq!(status, 1, "mara {args:?}: {stdout}");
    let at = format!(".mara/config.toml:{line}: error[");
    assert!(
        stdout.contains(&at),
        "mara {args:?} reports no {at}: {stdout}"
    );
}
