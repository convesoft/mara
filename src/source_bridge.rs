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
    SourceEngine::new(source_snapshot(project, documents))
}

pub(crate) fn source_fingerprint(project: &Project, documents: &DocumentSet) -> String {
    let snapshot = source_snapshot(project, documents);
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

fn source_snapshot(project: &Project, documents: &DocumentSet) -> SourceSnapshot {
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
    paths.extend(
        project
            .rule_files
            .iter()
            .filter(|p| {
                crate::is_project_relative(p)
                    && fs::canonicalize(project.root().join(p)).is_ok_and(|canonical| {
                        canonical.starts_with(project.root()) && canonical.is_file()
                    })
            })
            .cloned(),
    );
    for path in paths {
        if files.iter().any(|f| std::path::Path::new(&f.path) == path) {
            continue;
        }
        if let Ok(content) = fs::read_to_string(project.root().join(&path)) {
            files.push(SourceFile {
                path: path.to_string_lossy().into_owned(),
                content,
            });
        }
    }
    let mut ignores = Vec::new();
    for ancestor in project.root().ancestors() {
        if let Ok(content) = fs::read_to_string(ancestor.join(".gitignore")) {
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
    for entry in walk.build().flatten() {
        if !entry.file_type().is_some_and(|t| t.is_dir()) || entry.path() == project.root() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(entry.path().join(".gitignore")) {
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
    SourceSnapshot {
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
    }
}
