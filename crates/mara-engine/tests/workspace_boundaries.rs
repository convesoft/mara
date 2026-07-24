use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use syn::{
    ItemExternCrate, ItemUse, UseTree,
    visit::{self, Visit},
};

const WORKSPACE_PACKAGES: [&str; 4] = ["mara-core", "mara-markdown", "mara-engine", "mara-cli"];

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: BTreeSet<String>,
}

#[derive(Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    dependencies: Vec<CargoDependency>,
}

#[derive(Deserialize)]
struct CargoDependency {
    name: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root")
}

fn cargo_metadata() -> CargoMetadata {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
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

fn internal_dependencies(package: &CargoPackage) -> BTreeSet<&str> {
    package
        .dependencies
        .iter()
        .map(|dependency| dependency.name.as_str())
        .filter(|dependency| WORKSPACE_PACKAGES.contains(dependency))
        .collect()
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
                    && matches!(module.as_str(), "env" | "fs" | "io" | "net" | "process")
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
                if segments.len() == 1 && segments[0] == "std" {
                    self.violations
                        .push("renaming the std root can hide infrastructure paths".into());
                }
                segments.pop();
            }
            UseTree::Glob(_) => self.check_path(segments),
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
    let packages = workspace_packages(&metadata);

    assert_eq!(
        internal_dependencies(packages["mara-core"]),
        BTreeSet::new()
    );
    assert_eq!(
        internal_dependencies(packages["mara-markdown"]),
        BTreeSet::from(["mara-core"])
    );
    assert_eq!(
        internal_dependencies(packages["mara-engine"]),
        BTreeSet::from(["mara-core", "mara-markdown"])
    );
    assert_eq!(
        internal_dependencies(packages["mara-cli"]),
        BTreeSet::from(["mara-engine"])
    );
}

#[test]
fn core_has_no_dependencies_or_infrastructure_coupling() {
    let metadata = cargo_metadata();
    let packages = workspace_packages(&metadata);
    assert!(packages["mara-core"].dependencies.is_empty());

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
fn resolved_dependency_names_defeat_manifest_aliases() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [{
                "id": "mara-markdown-id",
                "name": "mara-markdown",
                "dependencies": [{
                    "name": "mara-core",
                    "rename": "domain"
                }]
            }],
            "workspace_members": ["mara-markdown-id"]
        }"#,
    )
    .expect("decode metadata fixture");
    let packages = workspace_packages(&metadata);

    assert_eq!(
        internal_dependencies(packages["mara-markdown"]),
        BTreeSet::from(["mara-core"])
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
