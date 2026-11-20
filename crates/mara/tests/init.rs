#![cfg(unix)]

//! `project init` through the real `mara` binary in disposable directories: VER-PROJECT-INIT in
//! `docs/verification.mara.md` and the `project init` cases of VER-REFUSED-EDITS in
//! `docs/authoring/verification.mara.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::SystemTime;

use serde_json::{Map, Value, json};

/// The files `project init` writes, in the order it writes them.
const FILES: [&str; 2] = [".mara/schema.toml", ".mara/config.toml"];

/// A disposable directory under `$TMPDIR`, removed on drop.
struct Dir {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Dir {
    fn new() -> Dir {
        let dir = tempfile::Builder::new()
            .prefix("mara-init-")
            .tempdir()
            .expect("create a temporary directory");
        let root = fs::canonicalize(dir.path()).unwrap();
        Dir { _dir: dir, root }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).unwrap()
    }

    /// Runs `mara <args>` in the directory.
    fn run(&self, args: &[&str]) -> Run {
        let child = command(&self.root, args).spawn().expect("start mara");
        Run::of(child, args)
    }
}

/// `mara <args>` in `directory` with an empty environment, so it reaches no user configuration.
fn command(directory: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mara"));
    command
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// One finished `mara` process.
#[derive(Debug)]
struct Run {
    args: Vec<String>,
    status: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn of(child: Child, args: &[&str]) -> Run {
        let output = child.wait_with_output().expect("wait for mara");
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            status: output.status.code().expect("mara exits"),
            stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
            stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
        }
    }

    fn status(self, status: i32) -> Run {
        assert_eq!(
            self.status, status,
            "exit status of mara {:?}: {self:#?}",
            self.args
        );
        self
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {self:#?}"))
    }
}

/// Every entry below `root`: a directory as `None`, a file or link with its bytes and mtime.
type Tree = BTreeMap<String, Option<(Vec<u8>, SystemTime)>>;

fn tree(root: &Path) -> Tree {
    fn walk(root: &Path, directory: &Path, out: &mut Tree) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if metadata.is_dir() {
                out.insert(relative, None);
                walk(root, &path, out);
            } else {
                let bytes = fs::read(&path).unwrap_or_default();
                out.insert(relative, Some((bytes, metadata.modified().unwrap())));
            }
        }
    }
    let mut out = Tree::new();
    walk(root, root, &mut out);
    out
}

/// Entries that differ between `before` and `after`, or are in only one of them.
fn changed(before: &Tree, after: &Tree) -> Vec<String> {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

#[test]
fn writes_the_engineering_template_into_the_working_or_a_named_directory() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema = fs::read_to_string(repository.join(".mara/schema.toml")).unwrap();
    for named in [None, Some("existing"), Some("new/project")] {
        let dir = Dir::new();
        if named == Some("existing") {
            fs::create_dir(dir.root.join("existing")).unwrap();
        }
        let before = tree(&dir.root);
        let mut args = vec!["project", "init", "--format", "json"];
        args.extend(named.iter().flat_map(|named| ["--project", *named]));
        let run = dir.run(&args).status(0);
        assert_eq!(
            run.json(),
            json!({"format_version": 1, "created": FILES}),
            "{named:?}"
        );
        // Exactly the files reported, with the directories that hold them.
        let target = named.map_or(String::new(), |named| format!("{named}/"));
        let mut expected: Vec<String> = match named {
            Some("new/project") => vec!["new".into(), "new/project".into()],
            _ => Vec::new(),
        };
        expected.push(format!("{target}.mara"));
        expected.extend(FILES.iter().map(|file| format!("{target}{file}")));
        expected.sort();
        assert_eq!(changed(&before, &tree(&dir.root)), expected, "{named:?}");
        // The schema is this repository's, the configuration the documented example.
        assert_eq!(dir.read(&format!("{target}.mara/schema.toml")), schema);
        assert_eq!(
            dir.read(&format!("{target}{}", FILES[1])),
            documented_example("DES-PROJECT-CONFIGURATION")
        );
        let mut validate = vec!["project", "validate", "--format", "json"];
        validate.extend(named.iter().flat_map(|named| ["--project", *named]));
        let run = dir.run(&validate).status(0);
        assert_eq!(run.json()["diagnostics"], json!([]), "{named:?}");
    }
    // The human form lists the same files.
    let dir = Dir::new();
    let run = dir.run(&["project", "init"]).status(0);
    assert_eq!(
        run.stdout,
        "created .mara/schema.toml\ncreated .mara/config.toml\n"
    );
    assert_eq!(run.stderr, "");
}

#[test]
fn the_engineering_schema_declares_the_engineering_vocabulary_and_nothing_else() {
    let dir = Dir::new();
    dir.run(&["project", "init"]).status(0);
    let expected = vocabulary();
    let listed = dir
        .run(&["schema", "list", "--limit", "100", "--format", "json"])
        .status(0)
        .json();
    assert_eq!(listed["next_cursor"], Value::Null);
    let names = |declarations: &[Value]| -> BTreeSet<(String, String)> {
        declarations
            .iter()
            .map(|declaration| {
                let text = |key: &str| declaration[key].as_str().unwrap().to_owned();
                (text("kind"), text("name"))
            })
            .collect()
    };
    let listed = listed["declarations"].as_array().unwrap();
    assert_eq!(names(listed), names(&expected));
    assert_eq!(listed.len(), expected.len());
    for declaration in expected {
        let name = declaration["name"].as_str().unwrap();
        let got = dir
            .run(&["schema", "get", name, "--format", "json"])
            .status(0)
            .json();
        let got = got["declarations"].as_array().unwrap();
        assert_eq!(got.len(), 1, "{name}: {got:?}");
        assert_eq!(sorted_targets(got[0].clone()), declaration, "{name}");
    }
}

#[test]
fn takes_each_status_of_the_engineering_template_and_no_other() {
    let dir = Dir::new();
    dir.run(&["project", "init"]).status(0);
    let item = |n: usize, status: &str| {
        format!(
            "<a id=\"01M7D8DZ2663WP3A030000000{n}\"></a>\n\n### Term {n}\n\n| Type | Term |\n\
             | --- | --- |\n| ID | TERM-{n} |\n| Status | {status} |\n\nA definition.\n"
        )
    };
    let regions: Vec<String> = ["Draft", "Accepted", "Retired"]
        .iter()
        .enumerate()
        .map(|(n, status)| item(n + 1, status))
        .collect();
    dir.write("docs/terms.mara.md", &regions.join("\n---\n\n"));
    let run = dir
        .run(&["project", "validate", "--format", "json"])
        .status(0);
    assert_eq!(run.json()["diagnostics"], json!([]));
    dir.write("docs/other.mara.md", &item(4, "Proposed"));
    let run = dir
        .run(&["project", "validate", "--format", "json"])
        .status(1);
    let diagnostics = run.json()["diagnostics"].clone();
    assert_eq!(diagnostics.as_array().unwrap().len(), 1, "{diagnostics}");
    assert_eq!(diagnostics[0]["code"], "invalid-value");
    assert_eq!(diagnostics[0]["path"], "docs/other.mara.md");
    assert_eq!(diagnostics[0]["line"], 8);
}

#[test]
fn writes_the_empty_template() {
    let dir = Dir::new();
    let run = dir
        .run(&["project", "init", "--template", "empty", "--format", "json"])
        .status(0);
    assert_eq!(run.json(), json!({"format_version": 1, "created": FILES}));
    let entries: Vec<String> = tree(&dir.root).into_keys().collect();
    assert_eq!(entries, [".mara", FILES[1], FILES[0]]);
    let listed = dir
        .run(&["schema", "list", "--format", "json"])
        .status(0)
        .json();
    assert_eq!(listed["declarations"], json!([]));
    let run = dir
        .run(&["project", "validate", "--format", "json"])
        .status(0);
    assert_eq!(run.json()["diagnostics"], json!([]));
    assert_eq!(
        dir.read(FILES[1]),
        documented_example("DES-PROJECT-CONFIGURATION")
    );
}

/// A case of the refusals, and what it puts into the directory before `project init` runs.
type Setup = (&'static str, fn(&Dir));

#[test]
fn refuses_to_overwrite_and_changes_nothing() {
    let setups: [Setup; 9] = [
        ("configuration", |dir| dir.write(FILES[1], "mine\n")),
        ("schema", |dir| dir.write(FILES[0], "mine\n")),
        ("both", |dir| {
            dir.write(FILES[0], "mine\n");
            dir.write(FILES[1], "mine\n");
        }),
        (".mara is a file", |dir| dir.write(".mara", "mine\n")),
        ("a dangling link", |dir| {
            fs::create_dir(dir.root.join(".mara")).unwrap();
            std::os::unix::fs::symlink("missing", dir.root.join(FILES[1])).unwrap();
        }),
        ("a directory", |dir| {
            fs::create_dir_all(dir.root.join(FILES[0])).unwrap()
        }),
        ("a named directory", |dir| {
            dir.write("named/.mara/config.toml", "mine\n")
        }),
        ("0.3.0", |dir| {
            dir.write(".mara/project.toml", "format_version = 3\n")
        }),
        (".mara is a link", |dir| {
            fs::create_dir(dir.root.join("elsewhere")).unwrap();
            std::os::unix::fs::symlink("elsewhere", dir.root.join(".mara")).unwrap();
        }),
    ];
    for (case, setup) in setups {
        let dir = Dir::new();
        setup(&dir);
        let before = tree(&dir.root);
        let mut args = vec!["project", "init", "--format", "json"];
        if case == "a named directory" {
            args.extend(["--project", "named"]);
        }
        let run = dir.run(&args).status(1);
        assert_eq!(
            changed(&before, &tree(&dir.root)),
            [] as [&str; 0],
            "{case}"
        );
        let error = run.json()["error"].clone();
        let (code, named): (&str, &[&str]) = match case {
            "configuration" | "a dangling link" | "a named directory" => {
                ("existing-file", &FILES[1..])
            }
            "schema" | "a directory" => ("existing-file", &FILES[..1]),
            "both" => ("existing-file", &FILES),
            ".mara is a file" => ("existing-file", &[".mara"]),
            "0.3.0" => ("legacy-project", &[]),
            _ => ("symbolic-link", &[FILES[1]]),
        };
        assert_eq!(error["code"], code, "{case}: {error}");
        let message = error["message"].as_str().unwrap();
        let paths: Vec<&str> = error["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, named, "{case}: {error}");
        for name in named.iter().filter(|_| code == "existing-file") {
            assert!(message.contains(&format!("`{name}`")), "{case}: {message}");
        }
        if code == "legacy-project" {
            assert!(message.contains("mara project convert"), "{message}");
        }
    }
    // A named directory that is a file; an undeclared option and an unknown template.
    for (args, status, code) in [
        (&["project", "init", "--project", "file"][..], 1, "io-error"),
        (
            &["project", "init", "--templates", "empty"][..],
            2,
            "invalid-argument",
        ),
        (
            &["project", "init", "--template", "other"][..],
            2,
            "invalid-argument",
        ),
    ] {
        let dir = Dir::new();
        dir.write("file", "mine\n");
        let before = tree(&dir.root);
        let mut args = args.to_vec();
        args.extend(["--format", "json"]);
        let run = dir.run(&args).status(status);
        assert_eq!(run.json()["error"]["code"], code, "{args:?}");
        assert_eq!(
            changed(&before, &tree(&dir.root)),
            [] as [&str; 0],
            "{args:?}"
        );
    }
}

/// The pause points of the debug binary (DES-WRITE-PROCEDURE): a file of the template created
/// by another program while `project init` waits before step 2 or before that file's second
/// check, and a failure after `.mara` is created.
#[cfg(debug_assertions)]
#[test]
fn refuses_a_file_created_while_it_waits_and_leaves_nothing_behind() {
    use std::os::unix::fs::PermissionsExt;

    let other = |dir: &Dir, path: &str| dir.write(path, "the other program's contents");

    // Uninterrupted: before step 2, then each file in order, the configuration last.
    let dir = Dir::new();
    let (run, seen) = paused(&dir, |_, _| {});
    run.status(0);
    assert_eq!(seen, ["", FILES[0], FILES[1]]);

    for with_mara in [false, true] {
        // Step 2: the first or the later file created before writing began.
        for file in FILES {
            let dir = Dir::new();
            if with_mara {
                fs::create_dir(dir.root.join(".mara")).unwrap();
            }
            let before = tree(&dir.root);
            let (run, seen) = paused(&dir, |_, index| {
                if index == 0 {
                    other(&dir, file);
                }
            });
            let error = run.status(1).json()["error"].clone();
            assert_eq!(error["code"], "changed-file", "{error}");
            assert_eq!(error["diagnostics"][0]["path"], file, "{error}");
            assert_eq!(seen, [""]);
            let mut expected = vec![file.to_owned()];
            if !with_mara {
                expected.insert(0, ".mara".into());
            }
            assert_eq!(changed(&before, &tree(&dir.root)), expected, "{file}");
        }
        // Step 3: the first file created at its pause, after its temporary file is written.
        let dir = Dir::new();
        if with_mara {
            fs::create_dir(dir.root.join(".mara")).unwrap();
        }
        let before = tree(&dir.root);
        let (run, seen) = paused(&dir, |paused, _| {
            if paused == FILES[0] {
                other(&dir, FILES[0]);
            }
        });
        let error = run.status(1).json()["error"].clone();
        assert_eq!(error["code"], "changed-file", "{error}");
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains("nothing was written"),
            "{error}"
        );
        assert_eq!(seen, ["", FILES[0]]);
        assert_eq!(dir.read(FILES[0]), "the other program's contents");
        let mut expected = vec![FILES[0].to_owned()];
        if !with_mara {
            expected.insert(0, ".mara".into());
        }
        assert_eq!(changed(&before, &tree(&dir.root)), expected);

        // A failure before the first rename: `.mara` cannot take the temporary file. A `.mara`
        // that `project init` created is removed again.
        // SAFETY: `geteuid` has no preconditions.
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "a directory without write permission needs a user other than root"
        );
        let dir = Dir::new();
        if with_mara {
            fs::create_dir(dir.root.join(".mara")).unwrap();
        }
        let before = tree(&dir.root);
        let mara = dir.root.join(".mara");
        let (run, _) = paused(&dir, |_, index| {
            if index == 0 {
                fs::set_permissions(&mara, fs::Permissions::from_mode(0o555)).unwrap();
            }
        });
        if mara.exists() {
            fs::set_permissions(&mara, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let error = run.status(1).json()["error"].clone();
        assert_eq!(error["code"], "io-error", "{error}");
        assert_eq!(changed(&before, &tree(&dir.root)), [] as [&str; 0]);
    }
}

/// Runs `project init --format json` in `dir` with the pause points on. At each pause, calls
/// `at` with the contents of `paused` and the number of earlier pauses, then removes `paused`.
#[cfg(debug_assertions)]
fn paused(dir: &Dir, mut at: impl FnMut(&str, usize)) -> (Run, Vec<String>) {
    use std::time::{Duration, Instant};

    let pauses = tempfile::Builder::new()
        .prefix("mara-init-pause-")
        .tempdir()
        .unwrap();
    let args = ["project", "init", "--format", "json"];
    let mut child = command(&dir.root, &args)
        .env("MARA_TEST_PAUSE", pauses.path())
        .spawn()
        .expect("start mara");
    let paused = pauses.path().join("paused");
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "mara neither pauses nor ends");
        match fs::read_to_string(&paused) {
            Ok(contents) => {
                at(&contents, seen.len());
                seen.push(contents);
                fs::remove_file(&paused).unwrap();
            }
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    (Run::of(child, &args), seen)
}

/// The declarations of DES-ENGINEERING-VOCABULARY, read from its page, as `schema get` returns
/// them.
fn vocabulary() -> Vec<Value> {
    const PAGE: &str = include_str!("../../../docs/template/vocabulary.mara.md");
    let name = |label: &str| label.to_lowercase().replace(' ', "_");
    let names = |labels: &str| -> Vec<String> { labels.split(", ").map(name).collect() };
    let statuses: Vec<String> = PAGE
        .lines()
        .find_map(|line| line.strip_prefix("Every type has `Status`:"))
        .expect("the page names the statuses")
        .split('`')
        .skip(1)
        .step_by(2)
        .map(name)
        .collect();
    let mut declarations = Vec::new();
    for row in table(PAGE, "| Type | ID prefix | Use for | Extra fields |") {
        let [kind, prefix, use_for, extra] = &row[..] else {
            panic!("a type row has four cells: {row:?}");
        };
        let mut fields: Vec<Value> = extra
            .split('`')
            .skip(1)
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|pair| {
                let spec = pair[1].trim_start_matches(':').trim().trim_end_matches('.');
                let (repeatable, spec) = match spec.strip_prefix("repeatable") {
                    Some(rest) => (true, rest.trim_start_matches(';').trim()),
                    None => (false, spec),
                };
                let (kind, values) = match spec {
                    "text" => ("text", Value::Null),
                    values => ("enum", json!(names(values))),
                };
                json!({"name": name(pair[0]), "kind": kind, "values": values,
                       "required": false, "repeatable": repeatable})
            })
            .collect();
        fields.sort_by_key(|field| field["name"].as_str().unwrap().to_owned());
        declarations.push(json!({
            "kind": "type",
            "name": name(kind),
            "id_prefix": prefix.trim_matches('`'),
            "description": format!("{use_for}."),
            "statuses": statuses,
            "fields": fields,
        }));
    }
    let header = "| Key on the source item | Shown on the target as | Source → target types | Constraint | Means |";
    for row in table(PAGE, header) {
        let [key, shown, ends, constraint, means] = &row[..] else {
            panic!("a relation row has five cells: {row:?}");
        };
        let mut targets = Map::new();
        let mut same_type = false;
        for group in ends.split("; ") {
            let (sources, types) = group.split_once(" → ").expect("sources → targets");
            for source in names(sources) {
                let types = match types {
                    "the same type" => {
                        same_type = true;
                        vec![source.clone()]
                    }
                    types => names(types),
                };
                targets.insert(source, json!(types));
            }
        }
        let acyclic = match constraint.as_str() {
            "No cycles" => true,
            "" | "Child → parent" => false,
            other => panic!("an unknown constraint {other:?}"),
        };
        declarations.push(sorted_targets(json!({
            "kind": "relation",
            "name": name(key),
            "inverse": name(shown),
            "description": format!("{means}."),
            "targets": targets,
            "same_type": same_type,
            "acyclic": acyclic,
        })));
    }
    // `Implements`, which only code markers carry.
    let line = PAGE
        .lines()
        .find(|line| line.starts_with("`Implements` (code → "))
        .expect("the page declares `Implements`");
    let between = |start: &str, end: char| {
        let rest = &line[line.find(start).expect(start) + start.len()..];
        rest[..rest.find(end).unwrap()].to_owned()
    };
    declarations.push(sorted_targets(json!({
        "kind": "relation",
        "name": "implements",
        "inverse": name(&between("shown on the target as `", '`')),
        "description": between("Its schema description is \"", '"'),
        "targets": {"code": names(&between("(code → ", ';'))},
        "same_type": false,
        "acyclic": false,
    })));
    declarations
}

/// The cells of each row of the table under `header` on `page`.
fn table(page: &str, header: &str) -> Vec<Vec<String>> {
    let mut lines = page.lines().skip_while(|line| *line != header).skip(2);
    let mut rows = Vec::new();
    for line in lines.by_ref() {
        let Some(inner) = line
            .strip_prefix('|')
            .and_then(|line| line.strip_suffix('|'))
        else {
            break;
        };
        rows.push(
            inner
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect(),
        );
    }
    assert!(!rows.is_empty(), "no table under {header:?}");
    rows
}

/// A declaration with each relation's target types sorted, which the comparison ignores.
fn sorted_targets(mut declaration: Value) -> Value {
    if let Some(targets) = declaration
        .get_mut("targets")
        .and_then(Value::as_object_mut)
    {
        for types in targets.values_mut() {
            types
                .as_array_mut()
                .unwrap()
                .sort_by_key(|name| name.to_string());
        }
    }
    declaration
}

/// The first TOML code block of the item with `id` on `docs/project.mara.md`.
fn documented_example(id: &str) -> &'static str {
    const PAGE: &str = include_str!("../../../docs/project.mara.md");
    let start = PAGE
        .find(&format!("| ID | {id} |"))
        .unwrap_or_else(|| panic!("docs/project.mara.md has no item {id}"));
    let block = PAGE[start..]
        .split_once("```toml\n")
        .unwrap_or_else(|| panic!("{id} has no TOML example"))
        .1;
    block
        .split_once("```")
        .unwrap_or_else(|| panic!("the TOML example of {id} does not end"))
        .0
}
