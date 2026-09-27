mod support;
use mara::DiagnosticCode;
use mara::{CodeFile, CodeIndex, CodeProblem};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn fixture(language: &str, extensions: &[&str]) -> TempDir {
    let fixture = support::fixture();
    mara::initialize_project(fixture.path(), mara::Template::Minimal).unwrap();
    support::code_index::configure(fixture.path(), language, extensions, true);
    fixture
}

fn load(root: &Path) -> (CodeIndex, Vec<CodeProblem>) {
    let project = mara::resolve_project(Some(root), root).unwrap();
    CodeIndex::load(&project)
}

fn parse_file(
    path: PathBuf,
    source: String,
    definitions: &[(&str, &str)],
) -> (CodeFile, Vec<CodeProblem>) {
    let extension = path.extension().unwrap().to_str().unwrap();
    let language = match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        _ => unreachable!(),
    };
    let fixture = fixture(language, &[extension]);
    fs::write(fixture.path().join(&path), &source).unwrap();
    let definitions = definitions
        .iter()
        .map(|(name, descriptor)| support::code_index::definition(&source, name, descriptor))
        .collect::<Vec<_>>();
    support::code_index::write_index(
        fixture.path(),
        language,
        &[(path.to_str().unwrap(), &definitions)],
    );
    let (index, problems) = load(fixture.path());
    assert_eq!(
        fs::read_to_string(fixture.path().join(&path)).unwrap(),
        source
    );
    (
        index
            .files()
            .find(|file| file.path == path)
            .unwrap()
            .clone(),
        problems,
    )
}

// @mara implements VER-CODE-DISCOVERY
// @mara checks DES-CODE-TRACEABILITY
#[test]
fn adapters_attach_markers_to_methods_and_nested_functions() {
    let cases = [
        (
            "sample.rs",
            "// @mara implements REQ-A\nmod outer { struct Worker; impl Worker { fn run() { // @mara verifies REQ-A\n fn check() {} } } }",
            "rust::outer/Worker#run().",
            "rust::outer/Worker#run().check().",
            vec![
                ("outer", "outer/"),
                ("run", "outer/Worker#run()."),
                ("check", "outer/Worker#run().check()."),
            ],
        ),
        (
            "sample.py",
            "class Outer:\n    # @mara implements REQ-A\n    def run(self):\n        # @mara verifies REQ-A\n        def check(): pass\n",
            "python::Outer#run().",
            "python::Outer#run().check().",
            vec![("run", "Outer#run()."), ("check", "Outer#run().check().")],
        ),
        (
            "sample.js",
            "class Outer { // @mara implements REQ-A\n run() { // @mara verifies REQ-A\n function check() {} } }",
            "javascript::Outer#run().",
            "javascript::Outer#run().check().",
            vec![("run", "Outer#run()."), ("check", "Outer#run().check().")],
        ),
        (
            "sample.ts",
            "class Outer { // @mara implements REQ-A\n run(): void { // @mara verifies REQ-A\n function check(): void {} } }",
            "typescript::Outer#run().",
            "typescript::Outer#run().check().",
            vec![("run", "Outer#run()."), ("check", "Outer#run().check().")],
        ),
    ];
    for (path, source, method, nested, definitions) in cases {
        let (file, problems) = parse_file(path.into(), source.into(), &definitions);
        assert!(problems.is_empty(), "{path}: {problems:?}");
        assert!(
            file.symbols.iter().any(|s| s.selector == method),
            "{path}: {:?}",
            file.symbols.iter().map(|s| &s.selector).collect::<Vec<_>>()
        );
        assert!(
            file.symbols.iter().any(|s| s.selector == nested),
            "{path}: {:?}",
            file.symbols.iter().map(|s| &s.selector).collect::<Vec<_>>()
        );
        assert_eq!(file.markers.len(), 2, "{path}: {:?}", file.markers);
    }
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn shared_marker_parser_reads_block_comments_from_tree_sitter() {
    let (file, problems) = parse_file(
        "sample.js".into(),
        "/* @mara code_implements REQ-A */\nfunction run() {}\n".into(),
        &[("run", "run().")],
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        file.markers[0].endpoint,
        "code:sample.js::javascript::run()."
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn shared_marker_parser_requires_the_complete_introducer() {
    let (file, problems) = parse_file(
        "sample.js".into(),
        "// @marathon is an ordinary comment\n// @mara code_implements REQ-A\nfunction run() {}\n"
            .into(),
        &[("run", "run().")],
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(file.markers.len(), 1);
    assert_eq!(
        file.markers[0].endpoint,
        "code:sample.js::javascript::run()."
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn body_marker_does_not_attach_to_the_next_top_level_declaration() {
    let (file, problems) = parse_file(
        "sample.py".into(),
        "def first():\n    pass\n    # @mara code_implements REQ-A\ndef second(): pass\n".into(),
        &[("first", "first()."), ("second", "second().")],
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(file.markers.len(), 1);
    assert_eq!(file.markers[0].endpoint, "code:sample.py::python::first().");
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn unowned_nested_marker_is_not_assigned_to_the_file() {
    let (file, problems) = parse_file(
        "sample.js".into(),
        "const run = () => { /* @mara code_implements REQ-A */ };\n".into(),
        &[],
    );
    assert!(file.markers.is_empty());
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code, DiagnosticCode::CodeUnsupported);
    assert!(problems[0].message.contains("no supported code owner"));

    let (file, problems) = parse_file(
        "sample.js".into(),
        "// @mara code_implements REQ-A\nconst run = () => {};\n".into(),
        &[],
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(file.markers[0].endpoint, "code:sample.js");
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn adapters_attach_markers_through_declaration_modifiers() {
    let cases = [
        (
            "sample.rs",
            "trait Api {\n    /// @mara code_implements REQ-A\n    fn run(&self);\n}\n",
            "code:sample.rs::rust::Api#run().",
        ),
        (
            "sample.rs",
            "// @mara code_implements REQ-A\n#[test]\nfn run() {}\n",
            "code:sample.rs::rust::run().",
        ),
        (
            "sample.rs",
            "#[test]\n// @mara code_implements REQ-A\nfn run() {}\n",
            "code:sample.rs::rust::run().",
        ),
        (
            "sample.py",
            "# @mara code_implements REQ-A\n@decorator\ndef run(): pass\n",
            "code:sample.py::python::run().",
        ),
        (
            "sample.py",
            "@decorator\n# @mara code_implements REQ-A\ndef run(): pass\n",
            "code:sample.py::python::run().",
        ),
        (
            "sample.js",
            "// @mara code_implements REQ-A\nexport function run() {}\n",
            "code:sample.js::javascript::run().",
        ),
        (
            "sample.js",
            "// @mara code_implements REQ-A\n@sealed\nclass Service {}\n",
            "code:sample.js::javascript::Service#",
        ),
        (
            "sample.ts",
            "// @mara code_implements REQ-A\nexport function run(): void {}\n",
            "code:sample.ts::typescript::run().",
        ),
        (
            "sample.ts",
            "// @mara code_implements REQ-A\n@sealed\nclass Service {}\n",
            "code:sample.ts::typescript::Service#",
        ),
        (
            "sample.ts",
            "@sealed\n// @mara code_implements REQ-A\nclass Service {}\n",
            "code:sample.ts::typescript::Service#",
        ),
    ];
    for (path, source, endpoint) in cases {
        let (_, tail) = endpoint.split_once("::").unwrap();
        let (_, descriptor) = tail.split_once("::").unwrap();
        let name = if descriptor == "Service#" {
            "Service"
        } else {
            "run"
        };
        let (file, problems) = parse_file(path.into(), source.into(), &[(name, descriptor)]);
        assert!(problems.is_empty(), "{path}: {problems:?}");
        assert!(
            file.symbols
                .iter()
                .any(|s| format!("code:{path}::{}", s.selector) == endpoint)
        );
        assert_eq!(file.markers.len(), 1, "{path}: {:?}", file.markers);
        assert_eq!(file.markers[0].endpoint, endpoint, "{path}");
    }
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn symbol_content_includes_attached_modifiers() {
    let cases = [
        (
            "sample.rs",
            "#[test]\nfn run() {}\n",
            "run",
            "#[test]\nfn run() {}",
        ),
        (
            "sample.py",
            "@decorator\ndef run(): pass\n",
            "run",
            "@decorator\ndef run(): pass",
        ),
        (
            "sample.js",
            "export function run() {}\n",
            "run",
            "export function run() {}",
        ),
        (
            "sample.ts",
            "@sealed\nclass Service {}\n",
            "Service",
            "@sealed\nclass Service {}",
        ),
    ];
    for (path, source, selector, expected) in cases {
        let descriptor = if selector == "Service" {
            "Service#"
        } else {
            "run()."
        };
        let (file, problems) = parse_file(path.into(), source.into(), &[(selector, descriptor)]);
        assert!(problems.is_empty(), "{path}: {problems:?}");
        let symbol = file
            .symbols
            .iter()
            .find(|symbol| symbol.selector.ends_with(&format!("::{descriptor}")))
            .unwrap();
        let content =
            &file.source[symbol.content.span().start_byte()..symbol.content.span().end_byte()];
        assert_eq!(content, expected, "{path}");
    }
}
// @mara checks DES-CODE-TRACEABILITY
#[test]
fn discovery_is_ordered_ignores_git_paths_and_is_independent_of_document_filters() {
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    let config = root.join(".mara/project.toml");
    fs::write(
        &config,
        fs::read_to_string(&config)
            .unwrap()
            .replace("**/*.mara.md", "docs/*.mara.md"),
    )
    .unwrap();
    fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
    for name in ["z.rs", "a.rs", ".hidden.rs", "ignored.rs"] {
        fs::write(root.join(name), "fn run() {}\n").unwrap();
    }
    fs::write(
        root.join("literal.rs"),
        "const TEXT: &str = \"@mara implements REQ-A\";\n// @marathon ordinary\n",
    )
    .unwrap();
    fs::write(root.join("other.txt"), "ordinary unconfigured source").unwrap();
    let (index, problems) = load(root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        index
            .files()
            .map(|file| file.path.to_str().unwrap())
            .collect::<Vec<_>>(),
        [".hidden.rs", "a.rs", "literal.rs", "z.rs"]
    );
    assert!(index.files().all(|file| file.markers.is_empty()));
    assert_eq!(index.root(), root);
    let assets = index.assets().collect::<Vec<_>>();
    for path in [
        ".mara/project.toml",
        ".mara/code/rust.wasm",
        ".mara/code/rust.scm",
        "a.rs",
    ] {
        assert!(assets.contains(&&PathBuf::from(path)));
    }
    assert!(!assets.contains(&&PathBuf::from("ignored.rs")));
    let (again, problems) = load(root);
    assert!(problems.is_empty());
    assert_eq!(again, index);
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn invalid_packs_prevent_partial_scans() {
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    fs::write(root.join("source.rs"), "fn run() {}\n").unwrap();
    let query = root.join(".mara/code/rust.scm");
    let valid = fs::read(&query).unwrap();
    for invalid in [
        "(",
        "(function_item name: (_) @name) @symbol",
        "(function_item) @symbol\n(function_item name: (_) @name)\n(line_comment) @comment\n",
    ] {
        fs::write(&query, invalid).unwrap();
        let (index, problems) = load(root);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].code, DiagnosticCode::CodeUnsupported);
        assert_eq!(problems[0].source.path(), Path::new(".mara/project.toml"));
        assert_eq!(index.files().count(), 0);
    }
    fs::write(&query, valid).unwrap();
    fs::remove_file(root.join(".mara/code/rust.wasm")).unwrap();
    let (index, problems) = load(root);
    assert_eq!(index.files().count(), 0);
    assert_eq!(problems[0].code, DiagnosticCode::CodeUnsupported);
    assert!(problems[0].message.contains("rust.wasm"));
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn duplicate_extension_assignments_reject_all_adapters_before_scanning() {
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    fs::write(root.join("source.rs"), "fn run() {}\n").unwrap();
    let config = root.join(".mara/project.toml");
    let text = fs::read_to_string(&config).unwrap();
    let declaration = text.split_once("[[code.languages]]").unwrap().1;
    fs::write(&config, format!("{text}\n[[code.languages]]{declaration}")).unwrap();
    let (index, problems) = load(root);
    assert_eq!(index.files().count(), 0);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code, DiagnosticCode::CodeUnsupported);
    assert!(
        problems[0]
            .message
            .contains("unique alphanumeric source extensions")
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn unreadable_sources_and_invalid_markers_preserve_independent_files() {
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    fs::write(root.join("bad.rs"), [0xff]).unwrap();
    let source = "// @mara implements\nfn run() {}\n";
    fs::write(root.join("good.rs"), source).unwrap();
    let (index, problems) = load(root);
    assert_eq!(index.files().count(), 1);
    assert_eq!(index.files().next().unwrap().source, source);
    assert!(index.files().next().unwrap().markers.is_empty());
    assert_eq!(problems.len(), 2);
    assert!(
        problems
            .iter()
            .any(|problem| problem.code == DiagnosticCode::SourceInvalid
                && problem.source.path() == Path::new("bad.rs"))
    );
    let marker = problems
        .iter()
        .find(|problem| problem.code == DiagnosticCode::CodeUnsupported)
        .unwrap();
    assert_eq!(marker.source.path(), Path::new("good.rs"));
    assert_eq!(marker.source.span().start_line(), 1);
    assert_eq!(fs::read(root.join("bad.rs")).unwrap(), [0xff]);
    assert_eq!(fs::read_to_string(root.join("good.rs")).unwrap(), source);
}

// @mara checks DES-CODE-TRACEABILITY
#[cfg(unix)]
#[test]
fn code_and_adapter_symlinks_stay_within_the_project() {
    use std::os::unix::fs::symlink;
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    let outside = tempfile::tempdir().unwrap();
    fs::write(root.join("source.rs"), "fn run() {}\n").unwrap();
    fs::write(outside.path().join("outside.rs"), "fn outside() {}\n").unwrap();
    symlink("source.rs", root.join("linked.rs")).unwrap();
    symlink(outside.path().join("outside.rs"), root.join("outside.rs")).unwrap();
    let (index, problems) = load(root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        index
            .files()
            .map(|file| file.path.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["linked.rs", "source.rs"]
    );
    assert!(index.files().all(|file| file.symbols.is_empty()));
    let query = root.join(".mara/code/rust.scm");
    let outside_query = outside.path().join("rust.scm");
    fs::copy(&query, &outside_query).unwrap();
    fs::remove_file(&query).unwrap();
    symlink(&outside_query, &query).unwrap();
    let (index, problems) = load(root);
    assert_eq!(index.files().count(), 0);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code, DiagnosticCode::CodeUnsupported);
    assert!(problems[0].message.contains("within the project"));
}

// @mara checks DES-CODE-TRACEABILITY
#[cfg(unix)]
#[test]
fn walk_failures_prevent_indexing_partial_inputs() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture("rust", &["rs"]);
    let root = fixture.path();
    fs::write(root.join("source.rs"), "fn run() {}\n").unwrap();
    let unreadable = root.join("unreadable");
    fs::create_dir(&unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let (index, problems) = load(root);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(index.files().count(), 0);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].code, DiagnosticCode::SourceInvalid);
    assert_eq!(problems[0].source.path(), Path::new(".mara/project.toml"));
    assert!(
        problems[0]
            .message
            .contains("could not snapshot indexer inputs")
    );
}

// @mara checks DES-CODE-TRACEABILITY
#[test]
fn grouped_comments_preserve_markers_content_and_lexical_boundaries() {
    for (path, comment, declaration, language) in [
        ("sample.rs", "//", "#[test]\nfn run() {}", "rust"),
        ("sample.py", "#", "@decorator\ndef run(): pass", "python"),
        ("sample.js", "//", "export function run() {}", "javascript"),
        (
            "sample.ts",
            "//",
            "export function run(): void {}",
            "typescript",
        ),
    ] {
        let first = format!("{comment} @mara checks REQ-A");
        let second = format!("{comment} @mara checks REQ-B");
        let source = format!("{first}\n{comment} explanation\n\n{second}\n{declaration}\n");
        let (file, problems) = parse_file(path.into(), source.clone(), &[("run", "run().")]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(file.markers.len(), 2);
        for (marker, authored) in file.markers.iter().zip([first, second]) {
            assert_eq!(marker.endpoint, format!("code:{path}::{language}::run()."));
            let span = marker.source.span();
            assert_eq!(&source[span.start_byte()..span.end_byte()], authored);
        }
        let span = file.symbols[0].content.span();
        assert_eq!(&source[span.start_byte()..span.end_byte()], declaration);
    }
    for (source, endpoint) in [
        (
            "// @mara checks REQ-A\n// ordinary\nconst value = 1;\nfunction run() {}",
            "code:sample.js",
        ),
        (
            "function first() {\n// @mara checks REQ-A\n// ordinary\nwork();\nfunction run() {}\n}",
            "code:sample.js::javascript::first().",
        ),
        (
            "// @mara checks REQ-A\n// ordinary\nfunction first() {}\nfunction run() {}",
            "code:sample.js::javascript::first().",
        ),
    ] {
        let mut definitions = vec![("run", "run().")];
        if source.contains("first") {
            definitions.push(("first", "first()."));
        }
        let (file, problems) = parse_file("sample.js".into(), source.into(), &definitions);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(file.markers[0].endpoint, endpoint);
    }
}
