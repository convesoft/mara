use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const WORKSPACE_MEMBERS: [&str; 4] = [
    "crates/mara-core",
    "crates/mara-markdown",
    "crates/mara-engine",
    "crates/mara-cli",
];

const WORKSPACE_PACKAGES: [&str; 4] = ["mara-core", "mara-markdown", "mara-engine", "mara-cli"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root")
}

fn read_manifest(path: &Path) -> toml::Value {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn collect_dependencies(value: &toml::Value, dependencies: &mut BTreeSet<String>) {
    let Some(table) = value.as_table() else {
        return;
    };

    for (key, child) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            dependencies.extend(
                child
                    .as_table()
                    .unwrap_or_else(|| panic!("{key} must be a table"))
                    .keys()
                    .cloned(),
            );
        } else {
            collect_dependencies(child, dependencies);
        }
    }
}

fn package_dependencies(package: &str) -> BTreeSet<String> {
    let manifest = read_manifest(
        &workspace_root()
            .join("crates")
            .join(package)
            .join("Cargo.toml"),
    );
    let mut dependencies = BTreeSet::new();
    collect_dependencies(&manifest, &mut dependencies);
    dependencies
}

fn internal_dependencies(package: &str) -> BTreeSet<String> {
    package_dependencies(package)
        .into_iter()
        .filter(|dependency| WORKSPACE_PACKAGES.contains(&dependency.as_str()))
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

#[test]
fn workspace_contains_exactly_the_four_accepted_packages() {
    let manifest = read_manifest(&workspace_root().join("Cargo.toml"));
    let members: Vec<_> = manifest["workspace"]["members"]
        .as_array()
        .expect("workspace.members must be an array")
        .iter()
        .map(|member| member.as_str().expect("workspace member must be a string"))
        .collect();

    assert_eq!(members, WORKSPACE_MEMBERS);
}

#[test]
fn workspace_dependencies_follow_the_accepted_layer_direction() {
    assert_eq!(internal_dependencies("mara-core"), BTreeSet::new());
    assert_eq!(
        internal_dependencies("mara-markdown"),
        BTreeSet::from(["mara-core".to_owned()])
    );
    assert_eq!(
        internal_dependencies("mara-engine"),
        BTreeSet::from(["mara-core".to_owned(), "mara-markdown".to_owned()])
    );
    assert_eq!(
        internal_dependencies("mara-cli"),
        BTreeSet::from(["mara-engine".to_owned()])
    );
}

#[test]
fn core_has_no_dependencies_or_infrastructure_coupling() {
    assert_eq!(package_dependencies("mara-core"), BTreeSet::new());

    let core_source = workspace_root().join("crates/mara-core/src");
    let mut sources = Vec::new();
    rust_sources(&core_source, &mut sources);
    let forbidden = [
        "std::env",
        "std::fs",
        "std::io",
        "std::net",
        "std::process",
        "mara_cli",
        "mara_engine",
        "mara_markdown",
        "clap::",
        "rushdown",
    ];

    for source in sources {
        let contents = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
        for rejected in forbidden {
            assert!(
                !contents.contains(rejected),
                "{} must not expose or use {rejected}",
                source.display()
            );
        }
    }
}
