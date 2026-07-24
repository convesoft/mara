use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use proc_macro2::{TokenStream, TokenTree};
use serde::Deserialize;
use syn::{
    ItemExternCrate, ItemUse, UseTree,
    visit::{self, Visit},
};

const WORKSPACE_PACKAGES: [&str; 4] = ["mara-core", "mara-markdown", "mara-engine", "mara-cli"];
const ALLOWED_CORE_DEPENDENCIES: [&str; 1] = ["petgraph"];

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: BTreeSet<String>,
    resolve: CargoResolve,
}

#[derive(Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    #[serde(default)]
    targets: Vec<CargoTarget>,
}

#[derive(Deserialize)]
struct CargoTarget {
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct CargoResolve {
    nodes: Vec<CargoNode>,
}

#[derive(Deserialize)]
struct CargoNode {
    id: String,
    deps: Vec<CargoNodeDependency>,
}

#[derive(Deserialize)]
struct CargoNodeDependency {
    pkg: String,
    dep_kinds: Vec<CargoDependencyKind>,
}

#[derive(Deserialize)]
struct CargoDependencyKind {
    kind: Option<String>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root")
}

fn cargo_metadata() -> CargoMetadata {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode cargo metadata")
}

fn workspace_packages(metadata: &CargoMetadata) -> BTreeMap<&str, &CargoPackage> {
    metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
        .map(|package| (package.name.as_str(), package))
        .collect()
}

fn resolved_workspace_dependencies<'a>(
    metadata: &'a CargoMetadata,
    package_name: &str,
) -> BTreeSet<&'a str> {
    let packages = workspace_packages(metadata);
    let package = packages[package_name];
    let workspace_names_by_id: BTreeMap<_, _> = packages
        .values()
        .map(|package| (package.id.as_str(), package.name.as_str()))
        .collect();
    metadata
        .resolve
        .nodes
        .iter()
        .find(|node| node.id == package.id)
        .unwrap_or_else(|| panic!("missing resolve node for {package_name}"))
        .deps
        .iter()
        .filter_map(|dependency| workspace_names_by_id.get(dependency.pkg.as_str()).copied())
        .collect()
}

fn direct_non_dev_dependency_names<'a>(
    metadata: &'a CargoMetadata,
    package_name: &str,
) -> BTreeSet<&'a str> {
    let packages = workspace_packages(metadata);
    let package = packages[package_name];
    let package_names_by_id: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package.name.as_str()))
        .collect();
    metadata
        .resolve
        .nodes
        .iter()
        .find(|node| node.id == package.id)
        .unwrap_or_else(|| panic!("missing resolve node for {package_name}"))
        .deps
        .iter()
        .filter(|dependency| {
            dependency
                .dep_kinds
                .iter()
                .any(|kind| kind.kind.as_deref() != Some("dev"))
        })
        .map(|dependency| {
            package_names_by_id
                .get(dependency.pkg.as_str())
                .copied()
                .unwrap_or_else(|| panic!("missing package {}", dependency.pkg))
        })
        .collect()
}

fn disallowed_core_dependencies(metadata: &CargoMetadata) -> BTreeSet<&str> {
    direct_non_dev_dependency_names(metadata, "mara-core")
        .into_iter()
        .filter(|dependency| !ALLOWED_CORE_DEPENDENCIES.contains(dependency))
        .collect()
}

fn core_production_targets(metadata: &CargoMetadata) -> (usize, BTreeSet<&str>) {
    let packages = workspace_packages(metadata);
    let mut libraries = 0;
    let mut disallowed = BTreeSet::new();
    for kind in packages["mara-core"]
        .targets
        .iter()
        .flat_map(|target| &target.kind)
    {
        match kind.as_str() {
            "lib" => libraries += 1,
            "bench" | "example" | "test" => {}
            other => {
                disallowed.insert(other);
            }
        }
    }
    (libraries, disallowed)
}

fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
    {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

#[derive(Default)]
struct CoreBoundaryVisitor {
    violations: Vec<String>,
}

impl CoreBoundaryVisitor {
    fn check_path(&mut self, segments: &[String]) {
        let higher_layer_or_adapter = segments.first().is_some_and(|segment| {
            matches!(
                segment.as_str(),
                "mara_cli" | "mara_engine" | "mara_markdown" | "clap" | "rushdown"
            )
        });
        let infrastructure = matches!(
            segments,
            [standard, module, ..]
                if standard == "std"
                    && matches!(
                        module.as_str(),
                        "env" | "fs" | "io" | "net" | "os" | "process"
                    )
        );
        if higher_layer_or_adapter || infrastructure {
            self.violations.push(segments.join("::"));
        }
    }

    fn visit_use_tree(&mut self, tree: &UseTree, segments: &mut Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                segments.push(path.ident.to_string());
                self.visit_use_tree(&path.tree, segments);
                segments.pop();
            }
            UseTree::Name(name) => {
                segments.push(name.ident.to_string());
                self.check_path(segments);
                segments.pop();
            }
            UseTree::Rename(rename) => {
                segments.push(rename.ident.to_string());
                self.check_path(segments);
                let aliases_std_root = matches!(
                    segments.as_slice(),
                    [standard] if standard == "std"
                ) || matches!(
                    segments.as_slice(),
                    [standard, current] if standard == "std" && current == "self"
                );
                if aliases_std_root {
                    self.violations
                        .push("renaming the std root can hide infrastructure paths".into());
                }
                segments.pop();
            }
            UseTree::Glob(_) => {
                self.check_path(segments);
                if matches!(segments.as_slice(), [standard] if standard == "std") {
                    self.violations
                        .push("glob-importing the std root can hide infrastructure paths".into());
                }
            }
            UseTree::Group(group) => {
                for tree in &group.items {
                    self.visit_use_tree(tree, segments);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for CoreBoundaryVisitor {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments: Vec<_> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        self.check_path(&segments);
        visit::visit_path(self, path);
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        self.visit_use_tree(&item.tree, &mut Vec::new());
        visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        let crate_name = item.ident.to_string();
        self.check_path(std::slice::from_ref(&crate_name));
        if crate_name == "std" && item.rename.is_some() {
            self.violations
                .push("renaming the std crate can hide infrastructure paths".into());
        }
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_macro(&mut self, expression: &'ast syn::Macro) {
        if let Some(segment) = expression.path.segments.last() {
            let name = segment.ident.to_string();
            if matches!(
                name.as_str(),
                "dbg" | "eprint" | "eprintln" | "print" | "println"
            ) {
                self.violations.push(format!("terminal macro {name}!"));
            }
            if matches!(
                name.as_str(),
                "env" | "include" | "include_bytes" | "include_str" | "option_env"
            ) {
                self.violations
                    .push(format!("compile-time I/O macro {name}!"));
            }
        }
        if let Some(path) = forbidden_macro_token_path(&expression.tokens) {
            self.violations
                .push(format!("macro body contains forbidden path {path}"));
        }
        visit::visit_macro(self, expression);
    }
}

fn forbidden_macro_token_path(tokens: &TokenStream) -> Option<String> {
    fn collect(tokens: &TokenStream, atoms: &mut Vec<String>) {
        for token in tokens.clone() {
            match token {
                TokenTree::Group(group) => collect(&group.stream(), atoms),
                TokenTree::Ident(identifier) => atoms.push(identifier.to_string()),
                TokenTree::Punct(punctuation) => atoms.push(punctuation.as_char().to_string()),
                TokenTree::Literal(_) => atoms.push("<literal>".into()),
            }
        }
    }

    let mut atoms = Vec::new();
    collect(tokens, &mut atoms);
    for window in atoms.windows(4) {
        if window[0] == "std"
            && window[1] == ":"
            && window[2] == ":"
            && matches!(
                window[3].as_str(),
                "env" | "fs" | "io" | "net" | "os" | "process"
            )
        {
            return Some(format!("std::{}", window[3]));
        }
    }
    atoms
        .iter()
        .find(|atom| {
            matches!(
                atom.as_str(),
                "clap" | "mara_cli" | "mara_engine" | "mara_markdown" | "rushdown"
            )
        })
        .cloned()
}

#[test]
fn workspace_contains_exactly_the_four_accepted_packages() {
    let metadata = cargo_metadata();
    let packages = workspace_packages(&metadata);
    let actual: BTreeSet<_> = packages.keys().copied().collect();
    let expected = BTreeSet::from(WORKSPACE_PACKAGES);

    assert_eq!(actual, expected);
    assert_eq!(metadata.workspace_members.len(), WORKSPACE_PACKAGES.len());
}

#[test]
fn workspace_dependencies_follow_the_accepted_layer_direction() {
    let metadata = cargo_metadata();

    assert_eq!(
        resolved_workspace_dependencies(&metadata, "mara-core"),
        BTreeSet::new()
    );
    assert_eq!(
        resolved_workspace_dependencies(&metadata, "mara-markdown"),
        BTreeSet::from(["mara-core"])
    );
    assert_eq!(
        resolved_workspace_dependencies(&metadata, "mara-engine"),
        BTreeSet::from(["mara-core", "mara-markdown"])
    );
    assert_eq!(
        resolved_workspace_dependencies(&metadata, "mara-cli"),
        BTreeSet::from(["mara-engine"])
    );
}

#[test]
fn core_has_no_dependencies_or_infrastructure_coupling() {
    let metadata = cargo_metadata();
    assert_eq!(disallowed_core_dependencies(&metadata), BTreeSet::new());
    assert_eq!(core_production_targets(&metadata), (1, BTreeSet::new()));

    let core_source = workspace_root().join("crates/mara-core/src");
    let mut sources = Vec::new();
    rust_sources(&core_source, &mut sources);
    for source in sources {
        let contents = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
        let syntax = syn::parse_file(&contents)
            .unwrap_or_else(|error| panic!("parse {}: {error}", source.display()));
        let mut visitor = CoreBoundaryVisitor::default();
        visitor.visit_file(&syntax);
        assert!(
            visitor.violations.is_empty(),
            "{} contains forbidden core coupling: {}",
            source.display(),
            visitor.violations.join(", ")
        );
    }
}

#[test]
fn same_name_external_packages_do_not_count_as_workspace_edges() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [
                {
                    "id": "workspace-core-id",
                    "name": "mara-core"
                },
                {
                    "id": "mara-markdown-id",
                    "name": "mara-markdown"
                },
                {
                    "id": "external-core-id",
                    "name": "mara-core"
                }
            ],
            "workspace_members": ["workspace-core-id", "mara-markdown-id"],
            "resolve": {
                "nodes": [{
                    "id": "mara-markdown-id",
                    "deps": [{
                        "name": "domain",
                        "pkg": "external-core-id",
                        "dep_kinds": [{"kind": null}]
                    }]
                }]
            }
        }"#,
    )
    .expect("decode metadata fixture");

    assert_eq!(
        resolved_workspace_dependencies(&metadata, "mara-markdown"),
        BTreeSet::new()
    );
}

#[test]
fn syntax_inspection_rejects_grouped_infrastructure_imports() {
    let syntax =
        syn::parse_file("use std::{collections::BTreeSet, fs, io};").expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(visitor.violations, ["std::fs", "std::io"]);
}

#[test]
fn syntax_inspection_rejects_grouped_std_root_aliases() {
    let syntax = syn::parse_file(
        r#"use std::{self as platform}; fn read() { let _ = platform::fs::read("input"); }"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        ["renaming the std root can hide infrastructure paths"]
    );
}

#[test]
fn syntax_inspection_rejects_std_root_globs() {
    let syntax = syn::parse_file(r#"use std::*; fn read() { let _ = fs::read("input"); }"#)
        .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        ["glob-importing the std root can hide infrastructure paths"]
    );
}

#[test]
fn syntax_inspection_rejects_terminal_output_macros() {
    let syntax = syn::parse_file(
        r#"fn report(value: i32) {
            print!("{value}");
            println!("{value}");
            eprint!("{value}");
            eprintln!("{value}");
            let _ = dbg!(value);
        }"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        [
            "terminal macro print!",
            "terminal macro println!",
            "terminal macro eprint!",
            "terminal macro eprintln!",
            "terminal macro dbg!",
        ]
    );
}

#[test]
fn core_dependency_allowlist_rejects_infrastructure_but_allows_pure_support() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [
                {"id": "core-id", "name": "mara-core"},
                {"id": "petgraph-id", "name": "petgraph"},
                {"id": "git2-id", "name": "git2"},
                {"id": "proptest-id", "name": "proptest"}
            ],
            "workspace_members": ["core-id"],
            "resolve": {
                "nodes": [{
                    "id": "core-id",
                    "deps": [
                        {
                            "name": "petgraph",
                            "pkg": "petgraph-id",
                            "dep_kinds": [{"kind": null}]
                        },
                        {
                            "name": "git2",
                            "pkg": "git2-id",
                            "dep_kinds": [{"kind": null}]
                        },
                        {
                            "name": "proptest",
                            "pkg": "proptest-id",
                            "dep_kinds": [{"kind": "dev"}]
                        }
                    ]
                }]
            }
        }"#,
    )
    .expect("decode metadata fixture");

    assert_eq!(
        disallowed_core_dependencies(&metadata),
        BTreeSet::from(["git2"])
    );
}

#[test]
fn core_clippy_policy_covers_path_filesystem_capabilities() {
    let config: toml::Value = fs::read_to_string(workspace_root().join("clippy.toml"))
        .expect("read Clippy policy")
        .parse()
        .expect("parse Clippy policy");
    let configured: BTreeSet<_> = config["disallowed-methods"]
        .as_array()
        .expect("disallowed-methods array")
        .iter()
        .map(|entry| entry["path"].as_str().expect("disallowed method path"))
        .collect();
    let expected = BTreeSet::from([
        "std::path::Path::canonicalize",
        "std::path::Path::exists",
        "std::path::Path::is_dir",
        "std::path::Path::is_file",
        "std::path::Path::is_symlink",
        "std::path::Path::metadata",
        "std::path::Path::read_dir",
        "std::path::Path::read_link",
        "std::path::Path::symlink_metadata",
        "std::path::Path::try_exists",
    ]);
    assert_eq!(configured, expected);

    let core_root = fs::read_to_string(workspace_root().join("crates/mara-core/src/lib.rs"))
        .expect("read core crate root");
    assert!(core_root.contains("clippy::disallowed_methods"));

    let core_manifest: toml::Value =
        fs::read_to_string(workspace_root().join("crates/mara-core/Cargo.toml"))
            .expect("read core manifest")
            .parse()
            .expect("parse core manifest");
    assert_eq!(core_manifest["package"]["build"].as_bool(), Some(false));
    assert_eq!(core_manifest["package"]["autobins"].as_bool(), Some(false));
    assert_eq!(
        core_manifest["package"]["autoexamples"].as_bool(),
        Some(false)
    );
    assert_eq!(
        core_manifest["package"]["autobenches"].as_bool(),
        Some(false)
    );
}

#[test]
fn core_target_check_rejects_build_scripts() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [{
                "id": "core-id",
                "name": "mara-core",
                "targets": [
                    {"kind": ["lib"]},
                    {"kind": ["custom-build"]}
                ]
            }],
            "workspace_members": ["core-id"],
            "resolve": {"nodes": [{"id": "core-id", "deps": []}]}
        }"#,
    )
    .expect("decode metadata fixture");

    assert_eq!(
        core_production_targets(&metadata),
        (1, BTreeSet::from(["custom-build"]))
    );
}

#[test]
fn syntax_inspection_rejects_os_specific_io_paths() {
    let syntax = syn::parse_file(
        r#"fn connect(path: &std::path::Path) {
            let _ = std::os::unix::fs::symlink(path, path);
            let _ = std::os::unix::net::UnixStream::connect(path);
        }"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        [
            "std::os::unix::fs::symlink",
            "std::os::unix::net::UnixStream::connect"
        ]
    );
}

#[test]
fn syntax_inspection_rejects_macro_hidden_and_compile_time_io() {
    let syntax = syn::parse_file(
        r#"macro_rules! read_input {
            () => { std::fs::read("input") };
        }
        const TEXT: &str = include_str!("input");"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        [
            "macro body contains forbidden path std::fs",
            "compile-time I/O macro include_str!",
        ]
    );
}
