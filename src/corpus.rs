use crate::{CodeIndex, DiagnosticCode, Error, Project, Schema};
use globset::GlobSet;
use ignore::{DirEntry, Error as WalkError, Walk, WalkBuilder};
pub use mara_engine::corpus::*;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
pub fn load_documents_for_validation(
    project: &Project,
    schema: &Schema,
) -> Result<(DocumentSet, Vec<Diagnostic>), Error> {
    load_documents_for_validation_with_schema(project, Some(schema))
}

pub fn load_documents_syntax_for_validation(
    project: &Project,
) -> Result<(DocumentSet, Vec<Diagnostic>), Error> {
    load_documents_for_validation_with_schema(project, None)
}

fn load_documents_for_validation_with_schema(
    project: &Project,
    schema: Option<&Schema>,
) -> Result<(DocumentSet, Vec<Diagnostic>), Error> {
    let matcher = content_matcher(project)?;
    let (paths, mut diagnostics) = discover_for_validation(project.root(), &matcher);
    let mut complete = project.content_discovery_is_complete() && diagnostics.is_empty();
    let mut sources = Vec::with_capacity(paths.len());
    for relative_path in paths {
        let absolute_path = project.root().join(&relative_path);
        let source = match fs::read_to_string(&absolute_path) {
            Ok(source) => source,
            Err(error) => {
                complete = false;
                diagnostics.push(Diagnostic::unreadable(
                    relative_path,
                    format!("could not read Mara document: {error}"),
                ));
                continue;
            }
        };
        sources.push((relative_path, source));
    }
    let key = crate::runtime::sessions::key(
        "validation-documents",
        format!(
            "{sources:?}{schema:?}{diagnostics:?}{}",
            project.content_discovery_is_complete()
        )
        .as_bytes(),
    );
    let (mut documents, parsed_diagnostics) =
        crate::runtime::sessions::cached::<_, Error>(key, || {
            crate::runtime::sessions::count("documents", sources.len());
            Ok(parse_documents_for_validation(
                project,
                sources,
                schema,
                diagnostics,
            ))
        })?;
    documents.complete &= complete;
    Ok((documents, parsed_diagnostics))
}
// @mara implements REQ-CANONICAL-SOURCE
// @mara implements DES-DOCUMENT-FORMAT
pub fn load_documents(project: &Project, schema: &Schema) -> Result<DocumentSet, Error> {
    let matcher = content_matcher(project)?;
    let paths = discover(project.root(), &matcher)?;

    let mut documents = Vec::with_capacity(paths.len());
    for relative_path in paths {
        let absolute_path = project.root().join(&relative_path);
        let source = fs::read_to_string(&absolute_path).map_err(|source| Error::Io {
            action: "read Mara document",
            path: absolute_path,
            source,
        })?;
        let key = crate::runtime::sessions::key(
            "document",
            &serde_json::to_vec(&(
                relative_path.as_os_str().as_encoded_bytes(),
                &source,
                schema,
            ))
            .expect("document input bytes serialize"),
        );
        documents.push(crate::runtime::sessions::cached(key, || {
            crate::runtime::sessions::count("documents", 1);
            parse_document(relative_path, source, schema)
        })?);
    }
    Ok(DocumentSet {
        documents,
        complete: true,
    })
}

fn discover(root: &Path, matcher: &GlobSet) -> Result<Vec<PathBuf>, Error> {
    let mut paths = Vec::new();
    for entry in walker(root) {
        let entry = entry.map_err(|source| Error::Io {
            action: "discover Mara documents",
            path: root.to_path_buf(),
            source: io::Error::other(source),
        })?;
        if let Some(relative) = discovered_document(root, matcher, &entry) {
            paths.push(relative);
        }
    }
    paths.sort();
    Ok(paths)
}

fn discover_for_validation(root: &Path, matcher: &GlobSet) -> (Vec<PathBuf>, Vec<Diagnostic>) {
    let mut paths = Vec::new();
    let mut diagnostics = Vec::new();
    for result in walker(root) {
        match result {
            Ok(entry) => {
                if let Some(relative) = discovered_document(root, matcher, &entry) {
                    paths.push(relative);
                }
            }
            Err(error) => diagnostics.push(Diagnostic::unreadable(
                walk_error_path(root, &error),
                format!("could not discover Mara documents: {error}"),
            )),
        }
    }
    paths.sort();
    (paths, diagnostics)
}

fn walker(root: &Path) -> Walk {
    walker_builder(root).build()
}

fn walker_builder(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(false)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    builder
}

fn discovered_document(root: &Path, matcher: &GlobSet, entry: &DirEntry) -> Option<PathBuf> {
    if !entry
        .file_type()
        .is_some_and(|file_type| file_type.is_file())
    {
        return None;
    }
    let relative = entry
        .path()
        .strip_prefix(root)
        .expect("discovered content remains below the project root")
        .to_path_buf();
    (is_mara_document(&relative) && matcher.is_match(&relative)).then_some(relative)
}

pub(crate) fn walk_error_path(root: &Path, error: &WalkError) -> PathBuf {
    let path = match error {
        WalkError::Partial(errors) => errors.iter().find_map(walk_error_source_path),
        _ => walk_error_source_path(error),
    };
    path.map(|path| path.strip_prefix(root).unwrap_or(path).to_path_buf())
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn walk_error_source_path(error: &WalkError) -> Option<&Path> {
    match error {
        WalkError::Partial(errors) => errors.iter().find_map(walk_error_source_path),
        WalkError::WithLineNumber { err, .. } | WalkError::WithDepth { err, .. } => {
            walk_error_source_path(err)
        }
        WalkError::WithPath { path, .. } => Some(path),
        WalkError::Loop { child, .. } => Some(child),
        WalkError::Io(_)
        | WalkError::Glob { .. }
        | WalkError::UnrecognizedFileType(_)
        | WalkError::InvalidDefinition => None,
    }
}

pub fn load_corpus(project: &Project, schema: &Schema) -> Result<Corpus, Error> {
    let documents = load_documents(project, schema)?;
    let (code, problems) = crate::code::load(project);
    if let Some(problem) = problems.into_iter().next() {
        return Err(Error::InvalidProject {
            path: project.root().join(problem.source.path()),
            message: problem.message,
        });
    }
    retain_corpus(project, documents, code)
}

// @mara implements DES-CORPUS-CONFORMANCE
pub fn load_corpus_for_validation(
    project: &Project,
    schema: &Schema,
) -> Result<(Corpus, Vec<Diagnostic>), Error> {
    let (mut documents, mut diagnostics) = load_documents_for_validation(project, schema)?;
    let (code, problems) = crate::code::load(project);
    for problem in problems {
        if problem.code == DiagnosticCode::SourceInvalid {
            documents.complete = false;
        }
        let mut item_ids = Vec::new();
        if let Some(target) = &problem.target {
            item_ids.push(target.clone());
            for item in documents.items() {
                if item.id() == target || item.mid() == Some(target.as_str()) {
                    item_ids.push(item.id().to_owned());
                    if let Some(mid) = item.mid() {
                        item_ids.push(mid.to_owned());
                    }
                }
            }
        }
        diagnostic(
            problem.code,
            &mut diagnostics,
            &problem.source,
            problem.message,
        );
        diagnostics
            .last_mut()
            .expect("diagnostic was added")
            .set_item_ids(item_ids);
    }
    Ok((retain_corpus(project, documents, code)?, diagnostics))
}

pub fn load_corpus_syntax_for_validation(
    project: &Project,
) -> Result<(Corpus, Vec<Diagnostic>), Error> {
    let (documents, diagnostics) = load_documents_syntax_for_validation(project)?;
    Ok((
        retain_corpus(project, documents, CodeIndex::empty(project))?,
        diagnostics,
    ))
}

fn retain_corpus(
    project: &Project,
    documents: DocumentSet,
    code: CodeIndex,
) -> Result<Corpus, Error> {
    let source_fingerprint = crate::source_bridge::source_fingerprint(project, &documents);
    let key = crate::runtime::sessions::key(
        "corpus",
        format!("{documents:?}{code:?}{source_fingerprint}").as_bytes(),
    );
    let corpus = crate::runtime::sessions::cached::<_, Error>(key, || {
        Ok(Corpus {
            documents,
            code,
            source_fingerprint,
            projections: Default::default(),
        })
    })?;
    for path in corpus.file_only_code_paths() {
        crate::runtime::sessions::record(project.root().join(path));
    }
    Ok(corpus)
}

pub(crate) fn document_is_discoverable(project: &Project, relative: &Path) -> Result<bool, Error> {
    let content = content_matcher(project)?;
    if !is_mara_document(relative) || !content.is_match(relative) {
        return Ok(false);
    }
    let mut ancestor = project.root().to_path_buf();
    for component in relative.parent().into_iter().flat_map(Path::components) {
        ancestor.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&ancestor).map_err(|source| Error::Io {
            action: "inspect document discovery path",
            path: ancestor.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Ok(false);
        }
    }
    let mut matchers = walker_builder(project.root()).build_matchers();
    let mut ignores = matchers
        .pop()
        .expect("a walker builder produces one matcher for its root");
    let (matched, error) = ignores.matched_with_errors(relative, false);
    if let Some(source) = error {
        return Err(Error::Io {
            action: "evaluate document discovery",
            path: project.root().join(relative),
            source: io::Error::other(source),
        });
    }
    Ok(!matched.is_ignore())
}
