//! Native loading of explicit document inputs. Semantics live in mara-engine.
use crate::{
    DocumentSet, Error, IgnoreInput, Project, SelectionInputs, SourceEngine, SourceFile,
    SourceSnapshot,
};
use std::{fs, path::PathBuf};

/// Load the same document/selection inputs used by native reads, without code indexing.
/// Native rules and code remain separate capabilities, not browser snapshot imports.
pub fn load_source_engine(
    project: &Project,
    documents: &DocumentSet,
) -> Result<SourceEngine, Error> {
    if !documents.is_complete() {
        return Err(Error::InvalidProject {path:project.root().join(crate::PROJECT_FILE),message:"cannot construct a source snapshot from an incomplete document projection; supply the full source inventory and loading diagnostics".into()});
    }
    SourceEngine::new(source_snapshot(project, documents, true, true)?)
}

/// Load trusted host-relative filenames for native execution and mutation planning.
/// The host must supply a complete document inventory; this is not a browser import.
pub fn load_native_source_engine(
    project: &Project,
    documents: &DocumentSet,
) -> Result<SourceEngine, Error> {
    if !documents.is_complete() {
        return Err(Error::InvalidMutation {
            message: "cannot plan from an incomplete document inventory".into(),
        });
    }
    let snapshot = source_snapshot(project, documents, true, true)?;
    SourceEngine::from_native_sources(snapshot.files, snapshot.selection)
}

pub(crate) use load_native_source_engine as load_mutation_engine;

/// Snapshot project policy and query inputs; an absent document set is used only
/// for schema validation, which checks definitions without evaluating facts.
pub(crate) fn load_query_policy_engine(
    project: &Project,
    documents: Option<&DocumentSet>,
) -> Result<SourceEngine, Error> {
    let empty = DocumentSet {
        documents: vec![],
        complete: true,
    };
    load_source_engine(project, documents.unwrap_or(&empty))
}

pub(crate) fn source_fingerprint(project: &Project, documents: &DocumentSet) -> String {
    // Validation retains diagnostics for incomplete/unreadable inputs. Planning uses
    // the strict loader above and never treats an inspection failure as absence.
    let snapshot =
        source_snapshot(project, documents, false, true).expect("permissive fingerprint loading");
    // Snapshot path restrictions do not narrow legacy native filesystem names.
    // Ordinary inventories produce exactly the same normalized engine identity.
    let normalize = |path: String| {
        path.split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect::<Vec<_>>()
            .join("/")
    };
    let files = snapshot
        .files
        .into_iter()
        .map(|file| (normalize(file.path), file.content))
        .collect();
    let mut selection = snapshot.selection;
    selection.document_paths = selection
        .document_paths
        .into_iter()
        .map(normalize)
        .collect();
    selection.document_paths.sort();
    for ignore in &mut selection.ignores {
        ignore.directory = normalize(std::mem::take(&mut ignore.directory));
    }
    selection
        .ignores
        .sort_by(|a, b| (&a.root_prefix, &a.directory).cmp(&(&b.root_prefix, &b.directory)));
    mara_engine::fingerprint_source_inputs(&files, &selection)
}

fn source_snapshot(
    project: &Project,
    documents: &DocumentSet,
    strict: bool,
    include_rules: bool,
) -> Result<SourceSnapshot, Error> {
    let mut files = documents
        .documents()
        .iter()
        .map(|d| SourceFile {
            path: d.path().to_string_lossy().into_owned(),
            content: d.source().to_owned(),
        })
        .collect::<Vec<_>>();
    let mut paths = vec![
        PathBuf::from(crate::PROJECT_FILE),
        project
            .schema_path()
            .strip_prefix(project.root())
            .unwrap_or(project.schema_path())
            .to_owned(),
    ];
    for path in project.rule_files.iter().filter(|_| include_rules) {
        if !crate::is_project_relative(path) {
            continue;
        }
        match fs::canonicalize(project.root().join(path)) {
            Ok(canonical) if canonical.starts_with(project.root()) => {
                match fs::metadata(&canonical) {
                    Ok(metadata) if metadata.is_file() => paths.push(path.clone()),
                    Err(source) if strict => {
                        return Err(Error::Io {
                            action: "snapshot configured rule input",
                            path: canonical,
                            source,
                        });
                    }
                    _ => {}
                }
            }
            Err(source) if strict && source.kind() != std::io::ErrorKind::NotFound => {
                return Err(Error::Io {
                    action: "snapshot configured rule input",
                    path: project.root().join(path),
                    source,
                });
            }
            _ => {}
        }
    }
    if include_rules && let Some(policy) = project.query_policy() {
        paths.extend(policy.files.iter().cloned());
        paths.extend(policy.definition_files.iter().map(PathBuf::from));
        paths.extend(policy.shape_files.iter().cloned());
    }
    for path in paths {
        if files.iter().any(|f| std::path::Path::new(&f.path) == path) {
            continue;
        }
        if let Some(content) = read_input(&project.root().join(&path), strict)? {
            files.push(SourceFile {
                path: path.to_string_lossy().into_owned(),
                content,
            });
        }
    }
    let mut ignores = Vec::new();
    for ancestor in project.root().ancestors() {
        if let Some(content) = read_input(&ancestor.join(".gitignore"), strict)? {
            ignores.push(IgnoreInput {
                directory: String::new(),
                root_prefix: project
                    .root()
                    .strip_prefix(ancestor)
                    .expect("ancestor contains root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect(),
                content,
            });
        }
    }
    let mut walk = ignore::WalkBuilder::new(project.root());
    walk.hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(false)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    for entry in walk.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if strict => {
                return Err(Error::Io {
                    action: "snapshot source selection inputs",
                    path: project.root().to_owned(),
                    source: std::io::Error::other(error),
                });
            }
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|t| t.is_dir()) || entry.path() == project.root() {
            continue;
        }
        if let Some(content) = read_input(&entry.path().join(".gitignore"), strict)? {
            ignores.push(IgnoreInput {
                directory: entry
                    .path()
                    .strip_prefix(project.root())
                    .expect("walk below root")
                    .to_string_lossy()
                    .into_owned(),
                root_prefix: Vec::new(),
                content,
            });
        }
    }
    Ok(SourceSnapshot {
        format_version: 1,
        files,
        selection: SelectionInputs {
            document_paths: documents
                .documents()
                .iter()
                .map(|d| d.path().to_string_lossy().into_owned())
                .collect(),
            ignores,
        },
        base_identity: None,
    })
}

/// Snapshot selected documents plus explicit query files. No imports or network reads.
pub(crate) fn load_query_engine(
    project: &Project,
    documents: &DocumentSet,
    definition_files: &[String],
) -> Result<SourceEngine, Error> {
    if !documents.is_complete() {
        return Err(Error::InvalidProject {
            path: project.root().join(crate::PROJECT_FILE),
            message: "query source inventory is incomplete".into(),
        });
    }
    let mut snapshot = source_snapshot(project, documents, true, false)?;
    for file in definition_files {
        let file = mara_engine::normalize_source_path(file)?;
        if snapshot.files.iter().any(|f| f.path == file) {
            continue;
        }
        let requested = project.root().join(&file);
        let canonical = match fs::canonicalize(&requested) {
            Ok(path) => path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(Error::Io {
                    action: "load query definition",
                    path: requested,
                    source,
                });
            }
        };
        if !canonical.starts_with(project.root()) {
            return Err(Error::InvalidProject {
                path: requested,
                message: "query definition escapes selected project".into(),
            });
        }
        snapshot.files.push(SourceFile {
            path: file,
            content: fs::read_to_string(&canonical).map_err(|source| Error::Io {
                action: "load query definition",
                path: canonical,
                source,
            })?,
        });
    }
    SourceEngine::from_native_sources(snapshot.files, snapshot.selection)
}

fn read_input(path: &std::path::Path, strict: bool) -> Result<Option<String>, Error> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound || !strict => Ok(None),
        Err(source) => Err(Error::Io {
            action: "snapshot explicit source input",
            path: path.to_owned(),
            source,
        }),
    }
}
