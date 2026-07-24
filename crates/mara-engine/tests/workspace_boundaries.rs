use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
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
    #[serde(default)]
    src_path: String,
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
            "test" => {}
            other => {
                disallowed.insert(other);
            }
        }
    }
    (libraries, disallowed)
}

fn core_library_source(metadata: &CargoMetadata) -> &Path {
    let packages = workspace_packages(metadata);
    let target = packages["mara-core"]
        .targets
        .iter()
        .find(|target| target.kind.iter().any(|kind| kind == "lib"))
        .expect("mara-core library target");
    Path::new(&target.src_path)
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

fn is_infrastructure_std_module(module: &str) -> bool {
    matches!(module, "env" | "fs" | "io" | "net" | "os" | "process")
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
                    && is_infrastructure_std_module(module)
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

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let attribute_name = attribute.path().get_ident().map(ToString::to_string);
        if matches!(attribute_name.as_deref(), Some("allow" | "expect")) {
            let tokens = match &attribute.meta {
                syn::Meta::List(list) => list.tokens.to_string(),
                _ => String::new(),
            };
            if tokens.contains("clippy :: disallowed_methods")
                || tokens.contains("clippy :: dbg_macro")
                || tokens.contains("clippy :: print_stderr")
                || tokens.contains("clippy :: print_stdout")
            {
                self.violations
                    .push("core boundary lints must not be suppressed".into());
            }
        }
        if attribute.path().is_ident("path")
            && let syn::Meta::NameValue(name_value) = &attribute.meta
            && let syn::Expr::Lit(expression) = &name_value.value
            && let syn::Lit::Str(path) = &expression.lit
        {
            let path = Path::new(&path.value()).to_path_buf();
            if path.is_absolute()
                || path
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
                self.violations
                    .push("module path escapes the enforced core source tree".into());
            }
        }
        if attribute.path().is_ident("cfg_attr")
            && let syn::Meta::List(list) = &attribute.meta
            && token_atoms(&list.tokens)
                .iter()
                .any(|token| token == "path")
        {
            self.violations
                .push("conditional module paths escape source enforcement".into());
        }
        visit::visit_attribute(self, attribute);
    }
}

fn token_atoms(tokens: &TokenStream) -> Vec<String> {
    fn collect(stream: &TokenStream, atoms: &mut Vec<String>) {
        for token in stream.clone() {
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
    atoms
}

fn forbidden_macro_token_path(tokens: &TokenStream) -> Option<String> {
    let atoms = token_atoms(tokens);
    for window in atoms.windows(4) {
        if window[0] == "std"
            && window[1] == ":"
            && window[2] == ":"
            && (window[3] == "$" || is_infrastructure_std_module(&window[3]))
        {
            return Some(if window[3] == "$" {
                "std::<dynamic>".into()
            } else {
                format!("std::{}", window[3])
            });
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

struct UseCollector<'ast> {
    trees: Vec<&'ast UseTree>,
    extern_crates: Vec<&'ast ItemExternCrate>,
}

impl<'ast> Visit<'ast> for UseCollector<'ast> {
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        self.trees.push(&item.tree);
        visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        self.extern_crates.push(item);
        visit::visit_item_extern_crate(self, item);
    }
}

fn petgraph_aliases(file: &syn::File) -> (HashSet<String>, bool) {
    fn collect(
        tree: &UseTree,
        under_petgraph: bool,
        known: &HashSet<String>,
        aliases: &mut HashSet<String>,
        wildcard: &mut bool,
    ) {
        match tree {
            UseTree::Path(path) => {
                let under_petgraph = under_petgraph
                    || path.ident == "petgraph"
                    || known.contains(path.ident.to_string().as_str());
                collect(&path.tree, under_petgraph, known, aliases, wildcard);
            }
            UseTree::Name(name) if under_petgraph => {
                aliases.insert(name.ident.to_string());
            }
            UseTree::Rename(rename)
                if under_petgraph
                    || rename.ident == "petgraph"
                    || known.contains(rename.ident.to_string().as_str()) =>
            {
                aliases.insert(rename.rename.to_string());
            }
            UseTree::Glob(_) if under_petgraph => *wildcard = true,
            UseTree::Group(group) => {
                for item in &group.items {
                    collect(item, under_petgraph, known, aliases, wildcard);
                }
            }
            UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {}
        }
    }

    let mut collector = UseCollector {
        trees: Vec::new(),
        extern_crates: Vec::new(),
    };
    collector.visit_file(file);
    let mut aliases = HashSet::from(["petgraph".to_owned()]);
    for item in &collector.extern_crates {
        if item.ident == "petgraph" {
            aliases.insert(
                item.rename
                    .as_ref()
                    .map_or_else(|| item.ident.to_string(), |(_, rename)| rename.to_string()),
            );
        }
    }
    let mut wildcard = false;
    loop {
        let before = aliases.len();
        let known = aliases.clone();
        for tree in &collector.trees {
            collect(tree, false, &known, &mut aliases, &mut wildcard);
        }
        if aliases.len() == before {
            break;
        }
    }
    (aliases, wildcard)
}

struct PetgraphPathVisitor<'a> {
    aliases: &'a HashSet<String>,
    paths: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PetgraphPathVisitor<'_> {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path
            .segments
            .first()
            .is_some_and(|segment| self.aliases.contains(segment.ident.to_string().as_str()))
        {
            self.paths.insert(
                path.segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
        }
        visit::visit_path(self, path);
    }
}

struct PublicPetgraphVisitor<'a> {
    aliases: &'a HashSet<String>,
    violations: BTreeSet<String>,
}

impl PublicPetgraphVisitor<'_> {
    fn inspect_signature(&mut self, signature: &syn::Signature) {
        let mut visitor = PetgraphPathVisitor {
            aliases: self.aliases,
            paths: BTreeSet::new(),
        };
        visitor.visit_signature(signature);
        self.violations.extend(visitor.paths);
    }

    fn inspect_type(&mut self, item_type: &syn::Type) {
        let mut visitor = PetgraphPathVisitor {
            aliases: self.aliases,
            paths: BTreeSet::new(),
        };
        visitor.visit_type(item_type);
        self.violations.extend(visitor.paths);
    }

    fn inspect_generics(&mut self, generics: &syn::Generics) {
        let mut visitor = PetgraphPathVisitor {
            aliases: self.aliases,
            paths: BTreeSet::new(),
        };
        visitor.visit_generics(generics);
        self.violations.extend(visitor.paths);
    }

    fn inspect_path(&mut self, path: &syn::Path) {
        let mut visitor = PetgraphPathVisitor {
            aliases: self.aliases,
            paths: BTreeSet::new(),
        };
        visitor.visit_path(path);
        self.violations.extend(visitor.paths);
    }

    fn inspect_type_param_bound(&mut self, bound: &syn::TypeParamBound) {
        let mut visitor = PetgraphPathVisitor {
            aliases: self.aliases,
            paths: BTreeSet::new(),
        };
        visitor.visit_type_param_bound(bound);
        self.violations.extend(visitor.paths);
    }

    fn inspect_macro_tokens(&mut self, expression: &syn::Macro) {
        let atoms = token_atoms(&expression.tokens);
        if let Some(alias) = atoms.iter().find(|atom| self.aliases.contains(*atom)) {
            self.violations.insert(format!(
                "macro tokens reference private graph backend {alias}"
            ));
        }
    }

    fn inspect_public_use_tree(
        &mut self,
        tree: &UseTree,
        under_petgraph: bool,
        segments: &mut Vec<String>,
    ) {
        match tree {
            UseTree::Path(path) => {
                let at_root = segments.is_empty();
                let segment = path.ident.to_string();
                let under_petgraph =
                    under_petgraph || (at_root && self.aliases.contains(segment.as_str()));
                segments.push(segment);
                self.inspect_public_use_tree(&path.tree, under_petgraph, segments);
                segments.pop();
            }
            UseTree::Name(name) => {
                let at_root = segments.is_empty();
                let segment = name.ident.to_string();
                let exposes_petgraph =
                    under_petgraph || (at_root && self.aliases.contains(segment.as_str()));
                segments.push(segment);
                if exposes_petgraph {
                    self.violations
                        .insert(format!("public use {}", segments.join(" :: ")));
                }
                segments.pop();
            }
            UseTree::Rename(rename) => {
                let at_root = segments.is_empty();
                let segment = rename.ident.to_string();
                let exposes_petgraph =
                    under_petgraph || (at_root && self.aliases.contains(segment.as_str()));
                segments.push(segment);
                if exposes_petgraph {
                    self.violations.insert(format!(
                        "public use {} as {}",
                        segments.join(" :: "),
                        rename.rename
                    ));
                }
                segments.pop();
            }
            UseTree::Glob(_) if under_petgraph => {
                self.violations
                    .insert(format!("public use {} :: *", segments.join(" :: ")));
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.inspect_public_use_tree(item, under_petgraph, segments);
                }
            }
            UseTree::Glob(_) => {}
        }
    }
}

fn is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}

impl<'ast> Visit<'ast> for PublicPetgraphVisitor<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if is_public(&item.vis) {
            self.inspect_signature(&item.sig);
        }
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if is_public(&item.vis) {
            self.inspect_signature(&item.sig);
        }
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            for field in &item.fields {
                if is_public(&field.vis) {
                    self.inspect_type(&field.ty);
                }
            }
        }
        visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            for variant in &item.variants {
                for field in &variant.fields {
                    self.inspect_type(&field.ty);
                }
            }
        }
        visit::visit_item_enum(self, item);
    }

    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            for field in &item.fields.named {
                if is_public(&field.vis) {
                    self.inspect_type(&field.ty);
                }
            }
        }
        visit::visit_item_union(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            self.inspect_type(&item.ty);
        }
        visit::visit_item_type(self, item);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            self.inspect_type(&item.ty);
        }
        visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        if is_public(&item.vis) {
            self.inspect_type(&item.ty);
        }
        visit::visit_item_static(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        if is_public(&item.vis) {
            self.inspect_generics(&item.generics);
            for bound in &item.supertraits {
                self.inspect_type_param_bound(bound);
            }
            for trait_item in &item.items {
                match trait_item {
                    syn::TraitItem::Fn(function) => self.inspect_signature(&function.sig),
                    syn::TraitItem::Type(item_type) => {
                        self.inspect_generics(&item_type.generics);
                        for bound in &item_type.bounds {
                            self.inspect_type_param_bound(bound);
                        }
                        if let Some((_, default)) = &item_type.default {
                            self.inspect_type(default);
                        }
                    }
                    syn::TraitItem::Const(item_const) => {
                        self.inspect_generics(&item_const.generics);
                        self.inspect_type(&item_const.ty);
                    }
                    syn::TraitItem::Macro(item_macro) => {
                        self.inspect_macro_tokens(&item_macro.mac);
                    }
                    _ => {}
                }
            }
        }
        visit::visit_item_trait(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if let Some((trait_path, _)) = &item.trait_ {
            self.inspect_generics(&item.generics);
            self.inspect_path(trait_path);
            self.inspect_type(&item.self_ty);
            for impl_item in &item.items {
                match impl_item {
                    syn::ImplItem::Fn(function) => self.inspect_signature(&function.sig),
                    syn::ImplItem::Type(item_type) => {
                        self.inspect_generics(&item_type.generics);
                        self.inspect_type(&item_type.ty);
                    }
                    syn::ImplItem::Const(item_const) => {
                        self.inspect_generics(&item_const.generics);
                        self.inspect_type(&item_const.ty);
                    }
                    syn::ImplItem::Macro(item_macro) => {
                        self.inspect_macro_tokens(&item_macro.mac);
                    }
                    _ => {}
                }
            }
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        if is_public(&item.vis) && item.ident == "petgraph" {
            let exposed_name = item
                .rename
                .as_ref()
                .map_or_else(|| item.ident.to_string(), |(_, rename)| rename.to_string());
            self.violations
                .insert(format!("public extern crate {exposed_name}"));
        }
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_macro(&mut self, expression: &'ast syn::Macro) {
        self.inspect_macro_tokens(expression);
        visit::visit_macro(self, expression);
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        if is_public(&item.vis) {
            self.inspect_public_use_tree(&item.tree, false, &mut Vec::new());
        }
        visit::visit_item_use(self, item);
    }
}

fn public_petgraph_violations(file: &syn::File) -> BTreeSet<String> {
    let (aliases, wildcard) = petgraph_aliases(file);
    let mut visitor = PublicPetgraphVisitor {
        aliases: &aliases,
        violations: BTreeSet::new(),
    };
    if wildcard {
        visitor
            .violations
            .insert("petgraph glob imports obscure exported types".into());
    }
    visitor.visit_file(file);
    visitor.violations
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

    let expected_library = workspace_root().join("crates/mara-core/src/lib.rs");
    let actual_library = core_library_source(&metadata)
        .canonicalize()
        .expect("resolve mara-core library source");
    assert_eq!(actual_library, expected_library);
    let core_source = expected_library.parent().expect("core source directory");
    let mut sources = Vec::new();
    rust_sources(core_source, &mut sources);
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
        let public_violations = public_petgraph_violations(&syntax);
        assert!(
            public_violations.is_empty(),
            "{} exposes private petgraph implementation types: {}",
            source.display(),
            public_violations.into_iter().collect::<Vec<_>>().join(", ")
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
    let core_syntax = syn::parse_file(&core_root).expect("parse core crate root");
    assert!(core_syntax.attrs.iter().any(|attribute| {
        attribute.path().is_ident("forbid")
            && matches!(
                &attribute.meta,
                syn::Meta::List(list)
                    if list.tokens.to_string().contains("clippy :: disallowed_methods")
            )
    }));

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
fn core_target_check_rejects_unscanned_production_targets() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [{
                "id": "core-id",
                "name": "mara-core",
                "targets": [
                    {"kind": ["lib"], "src_path": "/workspace/core/src/lib.rs"},
                    {"kind": ["custom-build"], "src_path": "/workspace/core/build.rs"},
                    {"kind": ["example"], "src_path": "/workspace/core/examples/demo.rs"},
                    {"kind": ["bench"], "src_path": "/workspace/core/benches/load.rs"}
                ]
            }],
            "workspace_members": ["core-id"],
            "resolve": {"nodes": [{"id": "core-id", "deps": []}]}
        }"#,
    )
    .expect("decode metadata fixture");

    assert_eq!(
        core_production_targets(&metadata),
        (1, BTreeSet::from(["bench", "custom-build", "example"]))
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

#[test]
fn syntax_inspection_distinguishes_pure_and_infrastructure_macro_paths() {
    let syntax = syn::parse_file(
        r#"macro_rules! build_map {
            () => { std::collections::BTreeMap::new() };
        }
        macro_rules! read_fixed {
            () => { std::fs::read("input") };
        }
        macro_rules! read_dynamic {
            ($module:ident) => { std::$module::read("input") };
        }"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        [
            "macro body contains forbidden path std::fs",
            "macro body contains forbidden path std::<dynamic>",
        ]
    );
}

#[test]
fn syntax_inspection_rejects_dynamic_macro_paths_and_conditional_modules() {
    let syntax = syn::parse_file(
        r#"macro_rules! read_input {
            ($module:ident) => { std::$module::read("input") };
        }
        #[cfg_attr(all(), path = "../../outside.rs")]
        mod outside;"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert!(
        visitor
            .violations
            .contains(&"macro body contains forbidden path std::<dynamic>".into())
    );
    assert!(
        visitor
            .violations
            .contains(&"conditional module paths escape source enforcement".into())
    );
}

#[test]
fn syntax_inspection_rejects_lint_suppression_and_escaping_modules() {
    let syntax = syn::parse_file(
        r#"#[allow(clippy::disallowed_methods)]
        fn inspect(path: &std::path::Path) { let _ = path.exists(); }
        #[path = "../../outside.rs"]
        mod outside;"#,
    )
    .expect("parse fixture");
    let mut visitor = CoreBoundaryVisitor::default();
    visitor.visit_file(&syntax);

    assert_eq!(
        visitor.violations,
        [
            "core boundary lints must not be suppressed",
            "module path escapes the enforced core source tree",
        ]
    );
}

#[test]
fn public_api_inspection_rejects_petgraph_types_and_reexports() {
    let syntax = syn::parse_file(
        r#"use petgraph::Graph as BackendGraph;
        pub fn graph() -> BackendGraph<(), ()> { todo!() }
        pub struct Snapshot { pub graph: petgraph::Graph<(), ()> }
        pub use petgraph::graph::NodeIndex;
        pub trait GraphView: petgraph::visit::GraphBase {
            type Storage: Into<BackendGraph<(), ()>>;
            fn graph(&self) -> BackendGraph<(), ()>;
        }
        pub struct MaraGraph;
        impl GraphView for MaraGraph {
            type Storage = petgraph::Graph<(), ()>;
            fn graph(&self) -> BackendGraph<(), ()> { todo!() }
        }
        expose!(petgraph::Graph);
        pub extern crate petgraph as graph_backend;"#,
    )
    .expect("parse fixture");
    let violations = public_petgraph_violations(&syntax);

    assert!(violations.contains("BackendGraph"));
    assert!(violations.contains("petgraph::Graph"));
    assert!(violations.contains("petgraph::visit::GraphBase"));
    assert!(violations.contains("macro tokens reference private graph backend petgraph"));
    assert!(violations.contains("public extern crate graph_backend"));
    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("public use petgraph :: graph :: NodeIndex"))
    );
}

#[test]
fn public_api_inspection_rejects_root_grouped_petgraph_reexports() {
    let syntax = syn::parse_file(
        r#"pub use {
            petgraph::Graph as PublicGraph,
            petgraph::Direction,
        };"#,
    )
    .expect("parse fixture");
    let violations = public_petgraph_violations(&syntax);

    assert!(violations.contains("public use petgraph :: Graph as PublicGraph"));
    assert!(violations.contains("public use petgraph :: Direction"));
}

#[test]
fn public_api_inspection_tracks_chained_petgraph_aliases() {
    let syntax = syn::parse_file(
        r#"use petgraph::Graph as BackendGraph;
        use BackendGraph as InternalGraph;
        use InternalGraph as DomainGraph;
        pub fn graph() -> DomainGraph<(), ()> { todo!() }"#,
    )
    .expect("parse fixture");
    let violations = public_petgraph_violations(&syntax);

    assert!(violations.contains("DomainGraph"));
}

#[test]
fn core_library_source_check_rejects_custom_roots() {
    let metadata: CargoMetadata = serde_json::from_str(
        r#"{
            "packages": [{
                "id": "core-id",
                "name": "mara-core",
                "targets": [{"kind": ["lib"], "src_path": "/outside/lib.rs"}]
            }],
            "workspace_members": ["core-id"],
            "resolve": {"nodes": [{"id": "core-id", "deps": []}]}
        }"#,
    )
    .expect("decode metadata fixture");

    assert_ne!(
        core_library_source(&metadata),
        Path::new("/workspace/core/src/lib.rs")
    );
}
