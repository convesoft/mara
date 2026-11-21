//! Project selection and loading on disposable projects in the temporary directory
//! (REQ-ONE-PROJECT-PER-OPERATION and DES-PROJECT-CONFIGURATION in `docs/project.mara.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mara::project::{Project, SelectError, load, select};
use mara_engine::schema::FieldKind;
use mara_engine::work::WorkCounts;

/// A directory under `std::env::temp_dir()`, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mara-project-test-{}-{nanos}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    fn mkdir(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CONFIG: &str = "format_version = 1\nschema = \".mara/schema.toml\"\n\n[documents]\nformat_version = 1\ninclude = [\"**/*.mara.md\"]\n";

const SCHEMA: &str = "format_version = 1\nstatuses = [\"open\", \"closed\"]\n\n[types.finding]\nid_prefix = \"FND-\"\ndescription = \"Something found.\"\n";

/// Writes a project with the configuration and schema above at `relative`.
fn project(dir: &TempDir, relative: &str) -> PathBuf {
    dir.write(&format!("{relative}/.mara/config.toml"), CONFIG);
    dir.write(&format!("{relative}/.mara/schema.toml"), SCHEMA);
    dir.write(&format!("{relative}/docs/a.mara.md"), "# A\n");
    dir.path(relative)
}

/// A temporary directory holding an outer project at `outer/` and an inner one at
/// `outer/inner/`. The outer include pattern matches the inner documents.
fn nested() -> (TempDir, PathBuf, PathBuf) {
    let dir = TempDir::new();
    let outer = project(&dir, "outer");
    let inner = project(&dir, "outer/inner");
    (dir, outer, inner)
}

fn select_error(start: &Path, project: Option<&Path>) -> SelectError {
    select(start, project).expect_err("no project should be selected")
}

/// Loads the project at `root` and returns it with the number of files loading read, after
/// checking that loading hashed each of them once, from the same read, and did no other work.
fn load_counting(root: PathBuf) -> (Project, u64) {
    let mut work = WorkCounts::default();
    let project = load(root, &mut work);
    let reads = work.file_reads;
    assert_eq!(
        work,
        WorkCounts {
            file_reads: reads,
            hashes: reads,
            ..WorkCounts::default()
        }
    );
    assert_eq!(
        (project.config_hash.is_some(), project.schema_hash.is_some()),
        (reads >= 1, reads == 2),
        "a hash for each file read"
    );
    (project, reads)
}

#[test]
fn selects_the_nearest_project_above_the_start() {
    let (_dir, outer, inner) = nested();
    assert_eq!(select(&outer, None).unwrap(), outer);
    assert_eq!(select(&outer.join("docs"), None).unwrap(), outer);
    assert_eq!(select(&inner.join("docs"), None).unwrap(), inner);
    assert_eq!(select(&inner.join(".mara"), None).unwrap(), inner);
}

#[test]
fn project_option_names_the_root_from_anywhere() {
    let (_dir, outer, inner) = nested();
    assert_eq!(select(&inner.join("docs"), Some(&outer)).unwrap(), outer);
    assert_eq!(
        select(&outer.join("docs"), Some(Path::new("../inner"))).unwrap(),
        inner
    );
    let error = select_error(&inner, Some(&inner.join("docs")));
    assert_eq!(error.code(), "not-a-project");
    let error = select_error(&inner, Some(Path::new("missing")));
    assert_eq!(error.code(), "io-error");
    assert!(error.to_string().contains("missing"), "{error}");
}

#[test]
fn refuses_a_legacy_project_and_names_the_conversion() {
    let (dir, outer, _inner) = nested();
    dir.write("outer/old/.mara/project.toml", "format_version = 3\n");
    let old = dir.mkdir("outer/old/docs");
    for error in [
        select_error(&old, None),
        select_error(&outer, Some(&outer.join("old"))),
    ] {
        assert_eq!(error.code(), "legacy-project");
        assert!(
            error.to_string().contains("mara project convert"),
            "{error}"
        );
    }
    assert_eq!(select(&outer.join("docs"), None).unwrap(), outer);
}

#[test]
fn a_config_file_outside_mara_makes_no_project() {
    let (dir, outer, _inner) = nested();
    dir.write("outer/stray/config.toml", CONFIG);
    dir.write("outer/odd/.mara", "not a directory\n");
    assert_eq!(select(&dir.path("outer/stray"), None).unwrap(), outer);
    assert_eq!(select(&dir.path("outer/odd"), None).unwrap(), outer);
}

#[test]
fn a_converted_project_keeps_its_old_configuration_and_is_a_project() {
    let dir = TempDir::new();
    let root = project(&dir, "converted");
    dir.write("converted/.mara/project.toml", "format_version = 3\n");
    assert_eq!(select(&root, None).unwrap(), root);
}

#[test]
fn fails_outside_every_project() {
    let dir = TempDir::new();
    let start = dir.mkdir("plain/sub");
    if let Some(above) = dir.0.ancestors().find(|ancestor| {
        ancestor.join(".mara/config.toml").exists() || ancestor.join(".mara/project.toml").exists()
    }) {
        panic!(
            "the temporary directory is inside the project at {}: set TMPDIR outside every Mara project",
            above.display()
        );
    }
    let error = select_error(&start, None);
    assert_eq!(error.code(), "no-project");
    assert!(error.to_string().contains("mara project init"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_searched_stops_the_search() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, _outer, _inner) = nested();
    let locked = dir.mkdir("outer/locked/.mara");
    let start = dir.mkdir("outer/locked/docs");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&locked).is_ok();
    let result = select(&start, None);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        // Permissions do not apply, as for root; the search reaches the outer project.
        return;
    }
    let error = result.expect_err("the outer project must not be selected");
    assert_eq!(error.code(), "io-error");
}

#[test]
fn loads_the_configuration_and_the_schema() {
    let dir = TempDir::new();
    let root = project(&dir, "p");
    let (project, reads) = load_counting(root.clone());
    assert_eq!(reads, 2);
    assert_eq!(project.root, root);
    assert_eq!(project.diagnostics, []);
    let include = project.config.include.unwrap();
    assert_eq!(
        (include[0].path.as_str(), include[0].line),
        ("**/*.mara.md", 6)
    );
    let schema = project.schema.unwrap();
    assert_eq!(schema.statuses, ["open", "closed"]);
    assert_eq!(schema.types["finding"].id_prefix.as_deref(), Some("FND-"));
}

/// Gives the project at `relative` a configuration that names a schema file of its own,
/// `.mara/{name}.toml`, whose only status is `name`. The schema at `.mara/schema.toml` stays.
fn own_schema(dir: &TempDir, relative: &str, name: &str) {
    dir.write(
        &format!("{relative}/.mara/config.toml"),
        &CONFIG.replace(".mara/schema.toml", &format!(".mara/{name}.toml")),
    );
    dir.write(
        &format!("{relative}/.mara/{name}.toml"),
        &SCHEMA.replace("[\"open\", \"closed\"]", &format!("[\"{name}\"]")),
    );
}

/// VER-NESTED-PROJECTS below the CLI: selecting and loading either of two nested projects
/// reads that project's configuration and the schema it names, and no other file.
#[test]
fn a_nested_project_reads_only_its_own_configuration_and_schema() {
    let (dir, outer, inner) = nested();
    own_schema(&dir, "outer", "outer");
    own_schema(&dir, "outer/inner", "inner");
    for (start, named, root, name) in [
        (inner.join("docs"), None, &inner, "inner"),
        (outer.join("docs"), None, &outer, "outer"),
        (inner.join("docs"), Some(&outer), &outer, "outer"),
        (outer.join("docs"), Some(&inner), &inner, "inner"),
    ] {
        let case = format!("{name} from {}", start.display());
        let selected = select(&start, named.map(PathBuf::as_path)).unwrap();
        let (project, reads) = load_counting(selected);
        assert_eq!(&project.root, root, "{case}");
        assert_eq!(project.diagnostics, [], "{case}");
        assert_eq!(
            project.config.schema.unwrap().path,
            format!(".mara/{name}.toml"),
            "{case}"
        );
        assert_eq!(project.schema.unwrap().statuses, [name], "{case}");
        assert_eq!(reads, 2, "{case}");
    }
}

#[test]
fn reports_problems_of_both_files_by_path_and_line() {
    let dir = TempDir::new();
    let root = dir.path("p");
    dir.write(
        "p/.mara/config.toml",
        "format_version = 1\nschema = \"vocabulary/schema.toml\"\nname = \"p\"\n\n[documents]\nformat_version = 1\ninclude = [\"docs/*.mara.md\"]\n",
    );
    dir.write(
        "p/vocabulary/schema.toml",
        "format_version = 1\nstatuses = [\"open\"]\n\n[types.code]\nid_prefix = \"C-\"\ndescription = \"Reserved.\"\n\n[types.finding]\nid_prefix = \"FND-\"\ndescription = \"Found.\"\n",
    );
    let project = load(root, &mut WorkCounts::default());
    let printed: Vec<String> = project
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        printed,
        [
            ".mara/config.toml:3: error[unknown-key]: unknown key `name` in the configuration; the keys are `format_version`, `schema`, `documents`, `code`, `rules`, `queries`, `views`, `invariants`, `gates`",
            "vocabulary/schema.toml:4: error[reserved-name]: type name `code` is reserved",
        ]
    );
    let schema = project.schema.unwrap();
    assert_eq!(schema.types.keys().collect::<Vec<_>>(), ["finding"]);
    assert_eq!(project.config.include.unwrap()[0].path, "docs/*.mara.md");
}

#[test]
fn reports_a_missing_schema_at_the_line_that_names_it() {
    let dir = TempDir::new();
    let root = project(&dir, "p");
    fs::remove_file(root.join(".mara/schema.toml")).unwrap();
    let (project, reads) = load_counting(root);
    assert_eq!(reads, 1, "a read that fails is no file read");
    assert_eq!(project.schema, None);
    assert_eq!(project.diagnostics.len(), 1);
    let diagnostic = &project.diagnostics[0];
    assert_eq!(
        (diagnostic.path.as_str(), diagnostic.line, &*diagnostic.code),
        (".mara/config.toml", 2, "io-error")
    );
    assert!(
        diagnostic.message.contains(".mara/schema.toml"),
        "{diagnostic}"
    );
}

#[test]
fn reports_an_unreadable_configuration() {
    let dir = TempDir::new();
    let root = dir
        .mkdir("p/.mara/config.toml")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    assert_eq!(select(&root, None).unwrap(), root);
    let (project, reads) = load_counting(root);
    assert_eq!(reads, 0, "a read that fails is no file read");
    assert_eq!(project.schema, None);
    assert_eq!(project.diagnostics.len(), 1);
    assert_eq!(
        project.diagnostics[0].to_string().split(": ").next(),
        Some(".mara/config.toml:1")
    );
    assert_eq!(project.diagnostics[0].code, "io-error");
}

/// Writes the outer configuration of `nested()` with `schema` as its schema path, on line 2.
fn outer_schema(dir: &TempDir, schema: &str) {
    dir.write(
        "outer/.mara/config.toml",
        &CONFIG.replace("\".mara/schema.toml\"", &format!("\"{schema}\"")),
    );
}

/// Loads the project at `root` and requires that loading reported exactly one diagnostic: `code`
/// at `line` of the configuration, with a message that contains `expected`. The refused file is
/// not read: no schema is, and at line 1 neither is the configuration.
fn assert_refused(root: PathBuf, line: usize, code: &str, expected: &str) -> Project {
    let (project, reads) = load_counting(root);
    assert_eq!(reads, u64::from(line != 1), "{code} at line {line}");
    assert_eq!(project.schema, None);
    let printed: Vec<String> = project
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(printed.len(), 1, "{printed:?}");
    let prefix = format!(".mara/config.toml:{line}: error[{code}]: ");
    assert!(printed[0].starts_with(&prefix), "{}", printed[0]);
    assert!(printed[0].contains(expected), "{}", printed[0]);
    project
}

fn assert_outside_project(root: PathBuf, expected: &str) {
    assert_refused(root, 2, "outside-project", expected);
}

#[test]
fn a_nested_project_never_supplies_the_schema() {
    let (dir, outer, _inner) = nested();
    outer_schema(&dir, "inner/.mara/schema.toml");
    assert_eq!(select(&outer, None).unwrap(), outer);
    assert_outside_project(outer.clone(), "in the project at inner");

    dir.write("outer/old/.mara/project.toml", "format_version = 3\n");
    dir.write("outer/old/.mara/schema.toml", SCHEMA);
    outer_schema(&dir, "old/.mara/schema.toml");
    assert_outside_project(outer.clone(), "in the project at old");

    dir.write("outer/old/vocabulary/schema.toml", SCHEMA);
    outer_schema(&dir, "old/vocabulary/schema.toml");
    assert_outside_project(outer, "in the project at old");
}

#[cfg(unix)]
#[test]
fn a_mara_directory_that_is_a_symbolic_link_is_refused() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new();
    let store = project(&dir, "store").join(".mara");
    let linked = dir.mkdir("linked");
    symlink(&store, linked.join(".mara")).unwrap();
    for start in [linked.clone(), dir.mkdir("linked/docs")] {
        let error = select_error(&start, None);
        assert_eq!(error.code(), "symbolic-link", "{error}");
        assert!(error.to_string().contains("linked/.mara"), "{error}");
    }
    assert_eq!(select_error(&dir.0, Some(&linked)).code(), "symbolic-link");
    assert_refused(linked, 1, "symbolic-link", ".mara is a symbolic link");

    // In a nested project the search stops there, so the outer project is not used instead.
    let (dir, outer, inner) = nested();
    fs::remove_dir_all(inner.join(".mara")).unwrap();
    symlink(&store, inner.join(".mara")).unwrap();
    assert_eq!(
        select_error(&inner.join("docs"), None).code(),
        "symbolic-link"
    );
    assert_eq!(select(&outer.join("docs"), None).unwrap(), outer);
    outer_schema(&dir, "inner/.mara/schema.toml");
    assert_refused(outer.clone(), 2, "symbolic-link", "inner/.mara");
    dir.write("outer/inner/schema.toml", SCHEMA);
    outer_schema(&dir, "inner/schema.toml");
    assert_refused(outer, 2, "symbolic-link", "inner/.mara");
}

#[cfg(unix)]
#[test]
fn a_configuration_that_is_a_symbolic_link_is_refused() {
    use std::os::unix::fs::symlink;
    let (dir, outer, inner) = nested();
    dir.write(
        "outer/inner/.mara/config.toml",
        &format!("{CONFIG}unknown = 1\n"),
    );
    fs::remove_file(outer.join(".mara/config.toml")).unwrap();
    symlink(
        inner.join(".mara/config.toml"),
        outer.join(".mara/config.toml"),
    )
    .unwrap();
    assert_eq!(select(&outer, None).unwrap(), outer);
    let project = assert_refused(outer, 1, "symbolic-link", ".mara/config.toml");
    assert_eq!(project.config, Default::default());
}

#[cfg(unix)]
#[test]
fn a_schema_path_with_a_symbolic_link_is_refused() {
    use std::os::unix::fs::symlink;
    let (dir, outer, inner) = nested();
    let store = project(&dir, "store");

    // A directory component that is a link, into a nested project and into the project itself.
    symlink(inner.join(".mara"), outer.join("shared")).unwrap();
    outer_schema(&dir, "shared/schema.toml");
    assert_refused(
        outer.clone(),
        2,
        "symbolic-link",
        "shared is a symbolic link",
    );
    symlink(outer.join(".mara"), outer.join("own")).unwrap();
    outer_schema(&dir, "own/schema.toml");
    assert_refused(outer.clone(), 2, "symbolic-link", "own is a symbolic link");

    // A schema file that is a link: inside the project, out of it, and below a nested root back
    // into the outer project.
    symlink(outer.join(".mara/schema.toml"), outer.join("alias.toml")).unwrap();
    outer_schema(&dir, "alias.toml");
    assert_refused(outer.clone(), 2, "symbolic-link", "alias.toml");
    symlink(
        store.join(".mara/schema.toml"),
        outer.join("elsewhere.toml"),
    )
    .unwrap();
    outer_schema(&dir, "elsewhere.toml");
    assert_refused(outer.clone(), 2, "symbolic-link", "elsewhere.toml");
    symlink(outer.join(".mara/schema.toml"), inner.join("back.toml")).unwrap();
    outer_schema(&dir, "inner/back.toml");
    assert_outside_project(outer.clone(), "in the project at inner");

    // The schema at `.mara/schema.toml` when `.mara` itself is a link.
    fs::remove_dir_all(outer.join(".mara")).unwrap();
    symlink(store.join(".mara"), outer.join(".mara")).unwrap();
    assert_refused(outer, 1, "symbolic-link", ".mara is a symbolic link");
}

/// The examples of DES-PROJECT-CONFIGURATION and DES-SCHEMA-MODEL, taken from the page as
/// written, load as one project without diagnostics.
#[test]
fn the_documented_examples_load_as_a_project() {
    let dir = TempDir::new();
    dir.write(
        "p/.mara/config.toml",
        documented_example("DES-PROJECT-CONFIGURATION"),
    );
    dir.write(
        "p/.mara/schema.toml",
        documented_example("DES-SCHEMA-MODEL"),
    );
    let root = select(&dir.path("p"), None).unwrap();
    let project = load(root, &mut WorkCounts::default());
    assert_eq!(project.diagnostics, []);
    let include = project.config.include.unwrap();
    assert_eq!(
        (include[0].path.as_str(), include[0].line, include.len()),
        ("docs/**/*.mara.md", 6, 1)
    );

    let schema = project.schema.unwrap();
    assert_eq!(schema.statuses, ["draft", "accepted", "retired"]);
    assert_eq!(
        schema.types.keys().collect::<Vec<_>>(),
        ["need", "requirement", "risk", "term"]
    );
    assert_eq!(
        schema.types["risk"].fields["likelihood"].kind,
        FieldKind::Enum(
            ["rare", "unlikely", "possible", "likely", "almost-certain"]
                .map(str::to_owned)
                .to_vec()
        )
    );
    let aliases = &schema.types["term"].fields["aliases"];
    assert!(aliases.kind == FieldKind::Text && aliases.repeatable && !aliases.required);
    assert_eq!(
        schema.relations.keys().collect::<Vec<_>>(),
        ["implements", "satisfies", "supersedes"]
    );
    assert_eq!(
        schema.relations["implements"]
            .targets
            .keys()
            .collect::<Vec<_>>(),
        ["code"]
    );
    let supersedes = &schema.relations["supersedes"];
    assert_eq!(supersedes.inverse.as_deref(), Some("superseded_by"));
    assert_eq!(
        supersedes.targets.keys().collect::<Vec<_>>(),
        ["need", "requirement"]
    );
    assert!(supersedes.same_type && supersedes.acyclic);
}

/// The code language example of DES-CODE-MARKERS, added to the configuration example, loads
/// without diagnostics.
#[test]
fn the_documented_code_example_loads() {
    const CODE: &str = include_str!("../../../docs/code/index.mara.md");
    let dir = TempDir::new();
    let example = format!(
        "{}\n{}",
        documented_example("DES-PROJECT-CONFIGURATION"),
        example_on(CODE, "DES-CODE-MARKERS")
    );
    dir.write("p/.mara/config.toml", &example);
    dir.write(
        "p/.mara/schema.toml",
        documented_example("DES-SCHEMA-MODEL"),
    );
    let root = select(&dir.path("p"), None).unwrap();
    let project = load(root, &mut WorkCounts::default());
    assert_eq!(project.diagnostics, []);
    let rust = &project.config.code[0];
    assert_eq!(
        (rust.name.as_str(), rust.include[0].path.as_str()),
        ("rust", "src/**/*.rs")
    );
    assert_eq!(rust.prefixes, ["//", "///", "*"]);
}

/// The first TOML code block of the item with `id` on `docs/project.mara.md`, read at compile
/// time, before the next item's anchor.
fn documented_example(id: &str) -> &'static str {
    const PAGE: &str = include_str!("../../../docs/project.mara.md");
    example_on(PAGE, id)
}

/// The first TOML code block of the item with `id` on `page`, before the next item's anchor.
fn example_on(page: &'static str, id: &str) -> &'static str {
    let start = page
        .find(&format!("| ID | {id} |"))
        .unwrap_or_else(|| panic!("the page has no item {id}"));
    let item = &page[start..];
    let item = &item[..item.find("<a id=").unwrap_or(item.len())];
    let (_, block) = item
        .split_once("```toml\n")
        .unwrap_or_else(|| panic!("{id} has no TOML example"));
    let (example, _) = block
        .split_once("```\n")
        .unwrap_or_else(|| panic!("the TOML example of {id} does not end"));
    example
}
