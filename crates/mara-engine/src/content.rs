//! Deterministic discovery and loading of configured Mara source documents.

use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use ignore::WalkBuilder;
use mara_core::{
    ContentDiagnosticCode, Diagnostic, ProjectDiagnosticCode, SourceDocument, SourceSpan,
    SourceText,
};
use wax::{Glob, Program};

use crate::{
    diagnostic::sort_diagnostics,
    project::{FileIdentity, LoadedProject, file_identity, open_read_no_follow, path_identity},
};

/// A complete content-discovery result. Per-file failures do not discard
/// independently loaded documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDiscovery {
    documents: Vec<SourceDocument>,
    diagnostics: Vec<Diagnostic>,
}

impl ContentDiscovery {
    pub fn documents(&self) -> &[SourceDocument] {
        &self.documents
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub const fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn into_parts(self) -> (Vec<SourceDocument>, Vec<Diagnostic>) {
        (self.documents, self.diagnostics)
    }
}

#[derive(Debug)]
struct Candidate {
    logical_path: PathBuf,
    source_path: String,
}

#[derive(Debug)]
struct OpenedCandidate {
    file: fs::File,
    resolved_path: PathBuf,
    identity: Option<FileIdentity>,
}

#[derive(Debug)]
struct RejectedDirectorySymlink {
    source_path: Option<String>,
    reason: &'static str,
}

/// Discovers and decodes the content selected by an already validated project.
/// Documents and diagnostics are both returned in normalized project-relative
/// path order.
pub fn discover_content(project: &LoadedProject) -> ContentDiscovery {
    let includes = compile_globs(&project.content.include);
    let excludes = compile_globs(&project.content.exclude);
    let rejected_directories = Arc::new(Mutex::new(Vec::new()));
    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();

    for result in content_walker(project, Arc::clone(&rejected_directories)).build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                for error in walk_error_parts(&error) {
                    let source_path = walk_error_path(error)
                        .and_then(|path| normalized_relative_path(&project.root, path));
                    let is_loop = is_walk_loop(error);
                    if !is_loop
                        && walk_error_path(error).is_some_and(|path| {
                            fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
                        })
                        && source_path
                            .as_deref()
                            .is_some_and(|path| !is_selected(&includes, &excludes, path))
                    {
                        continue;
                    }
                    let source_path =
                        source_path.unwrap_or_else(|| ".mara/project.toml".to_owned());
                    let reason = if is_loop {
                        "directory_cycle"
                    } else {
                        "walk_error"
                    };
                    diagnostics.push(
                        diagnostic_at_start(
                            ContentDiagnosticCode::Io,
                            &source_path,
                            "could not inspect a configured content path",
                        )
                        .with_detail("operation", "discover")
                        .with_detail("reason", reason),
                    );
                }
                continue;
            }
        };
        if let Some(error) = entry.error() {
            let affected_path = normalized_relative_path(&project.root, entry.path());
            if entry.file_type().is_some_and(|kind| kind.is_file())
                && affected_path
                    .as_deref()
                    .is_some_and(|path| !is_selected(&includes, &excludes, path))
            {
                continue;
            }
            for error in walk_error_parts(error) {
                let source_path = walk_error_path(error)
                    .and_then(|path| normalized_relative_path(&project.root, path))
                    .or_else(|| affected_path.clone())
                    .unwrap_or_else(|| ".mara/project.toml".to_owned());
                diagnostics.push(
                    diagnostic_at_start(
                        ContentDiagnosticCode::Io,
                        &source_path,
                        "could not evaluate ignore rules for content input",
                    )
                    .with_detail("operation", "gitignore")
                    .with_detail("reason", "ignore_rule_error"),
                );
            }
            continue;
        }
        if entry.depth() == 0 || entry.file_type().is_some_and(|kind| kind.is_dir()) {
            continue;
        }
        let source_path = match candidate_source_path(&project.root, entry.path()) {
            Ok(source_path) => source_path,
            Err(diagnostic) => {
                if lossy_relative_path(&project.root, entry.path())
                    .as_deref()
                    .is_some_and(|path| !is_selected(&includes, &excludes, path))
                {
                    continue;
                }
                diagnostics.push(*diagnostic);
                continue;
            }
        };
        if !is_selected(&includes, &excludes, &source_path) {
            continue;
        }
        if SourceSpan::try_new(&source_path, "", 0, 0, 1, 1, 1, 1).is_err() {
            diagnostics.push(
                diagnostic_at_start(
                    ContentDiagnosticCode::Io,
                    &source_path,
                    "selected content path cannot be represented as Mara source provenance",
                )
                .with_detail("operation", "identify")
                .with_detail("reason", "invalid_source_path"),
            );
            continue;
        }
        candidates.push(Candidate {
            logical_path: entry.into_path(),
            source_path,
        });
    }

    let rejected_directories = Arc::try_unwrap(rejected_directories)
        .expect("the content walker released its directory filter")
        .into_inner()
        .expect("the directory rejection lock is not poisoned");
    for rejected in rejected_directories {
        diagnostics.push(rejected_directory_diagnostic(rejected));
    }

    candidates.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    let mut documents = Vec::new();
    let mut selected_identities = HashMap::<FileIdentity, String>::new();
    let mut selected_paths = HashMap::<PathBuf, String>::new();
    let index_identity = opened_file_identity(&project.index_path);
    for candidate in candidates {
        if fs::canonicalize(&candidate.logical_path).is_ok_and(|path| path == project.index_path) {
            diagnostics.push(index_alias_diagnostic(project, &candidate));
            continue;
        }
        match open_candidate(project, &candidate) {
            Ok(opened) => {
                let aliases_index = opened.resolved_path == project.index_path
                    || matches!((opened.identity, index_identity), (Some(left), Some(right)) if left == right);
                if aliases_index {
                    diagnostics.push(index_alias_diagnostic(project, &candidate));
                    continue;
                }
                let duplicate = opened
                    .identity
                    .and_then(|identity| selected_identities.get(&identity).cloned())
                    .or_else(|| selected_paths.get(&opened.resolved_path).cloned());
                if let Some(first_path) = duplicate {
                    diagnostics.push(
                        diagnostic_at_start(
                            ProjectDiagnosticCode::DuplicateFile,
                            &candidate.source_path,
                            "more than one content path resolves to one file",
                        )
                        .with_detail("first_path", first_path)
                        .with_detail("path", candidate.source_path.clone()),
                    );
                    continue;
                }
                if let Some(identity) = opened.identity {
                    selected_identities.insert(identity, candidate.source_path.clone());
                }
                selected_paths.insert(opened.resolved_path, candidate.source_path.clone());
                match decode_candidate(&candidate, opened.file) {
                    Ok(document) => documents.push(document),
                    Err(diagnostic) => diagnostics.push(*diagnostic),
                }
            }
            Err(diagnostic) => diagnostics.push(*diagnostic),
        }
    }

    documents.sort_by(|left, right| left.path().cmp(right.path()));
    finalize_diagnostics(&mut diagnostics);

    ContentDiscovery {
        documents,
        diagnostics,
    }
}

fn opened_file_identity(path: &Path) -> Option<FileIdentity> {
    fs::metadata(path)
        .and_then(|metadata| path_identity(path, &metadata))
        .ok()
        .flatten()
}

fn content_walker(
    project: &LoadedProject,
    rejected: Arc<Mutex<Vec<RejectedDirectorySymlink>>>,
) -> WalkBuilder {
    let mut builder = WalkBuilder::new(&project.root);
    let respect_gitignore = project.content.respect_gitignore;
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(respect_gitignore)
        .git_exclude(respect_gitignore)
        .git_global(false)
        .parents(respect_gitignore)
        .require_git(true)
        .follow_links(project.content.follow_directory_symlinks);

    let root = project.root.clone();
    let follow_directory_symlinks = project.content.follow_directory_symlinks;
    builder.filter_entry(move |entry| {
        if entry.depth() == 0 || !entry.path_is_symlink() {
            return true;
        }
        let is_directory = fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir());
        if !is_directory {
            return true;
        }
        if !follow_directory_symlinks {
            return false;
        }
        match fs::canonicalize(entry.path()) {
            Ok(target) if target.starts_with(&root) => true,
            Ok(_) => {
                record_rejected_directory(
                    &rejected,
                    &root,
                    entry.path(),
                    "resolved target is outside the project root",
                );
                false
            }
            Err(_) => {
                record_rejected_directory(
                    &rejected,
                    &root,
                    entry.path(),
                    "target could not be resolved",
                );
                false
            }
        }
    });
    builder
}

fn record_rejected_directory(
    rejected: &Mutex<Vec<RejectedDirectorySymlink>>,
    root: &Path,
    path: &Path,
    reason: &'static str,
) {
    rejected
        .lock()
        .expect("the directory rejection lock is not poisoned")
        .push(RejectedDirectorySymlink {
            source_path: normalized_relative_path(root, path),
            reason,
        });
}

fn rejected_directory_diagnostic(rejected: RejectedDirectorySymlink) -> Diagnostic {
    match rejected.source_path {
        Some(source_path) => diagnostic_at_start(
            ProjectDiagnosticCode::SymlinkRejected,
            &source_path,
            "content directory symlink was rejected",
        )
        .with_detail("reason", rejected.reason),
        None => Diagnostic::new(
            ProjectDiagnosticCode::SymlinkRejected,
            "content directory symlink with a non-UTF-8 path was rejected",
            None,
        )
        .with_detail("path_reason", "non_utf8_path")
        .with_detail("reason", rejected.reason),
    }
}

fn compile_globs(patterns: &[String]) -> Vec<Glob<'static>> {
    patterns
        .iter()
        .map(|pattern| {
            compile_content_glob(pattern).expect("the project loader validated every content glob")
        })
        .collect()
}

pub(crate) fn compile_content_glob(pattern: &str) -> Result<Glob<'static>, wax::BuildError> {
    let pattern = pattern
        .split('/')
        .fold(Vec::new(), |mut segments, segment| {
            if segment != "**" || segments.last() != Some(&"**") {
                segments.push(segment);
            }
            segments
        })
        .join("/");
    let pattern = wax_expression(&pattern);
    let expression = if let Some(pattern) = pattern.strip_prefix("**/") {
        format!("**/(?-i){pattern}")
    } else if pattern == "**" {
        pattern
    } else {
        format!("(?-i){pattern}")
    };
    Glob::new(&expression).map(Glob::into_owned)
}

fn wax_expression(pattern: &str) -> String {
    let characters: Vec<char> = pattern.chars().collect();
    let mut expression = String::with_capacity(pattern.len());
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '[' {
            let close = index
                + 1
                + characters[index + 1..]
                    .iter()
                    .position(|character| *character == ']')
                    .expect("the project loader validated every character class");
            expression.push('[');
            let mut content_index = index + 1;
            if characters[content_index] == '!' {
                expression.push('!');
                content_index += 1;
            }
            while content_index < close {
                let character = characters[content_index];
                if character == '[' {
                    expression.push('\\');
                } else if character == '-'
                    && content_index >= index + 3
                    && characters[content_index - 2] == '-'
                {
                    expression.push(characters[content_index - 1]);
                }
                expression.push(character);
                content_index += 1;
            }
            expression.push(']');
            index = close + 1;
            continue;
        }
        let character = characters[index];
        if matches!(character, '$' | '(' | ')' | '<' | '>' | ',' | ':') {
            expression.push('\\');
        }
        expression.push(character);
        index += 1;
    }
    expression
}

fn matches_any(patterns: &[Glob<'_>], path: &str) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(path))
}

fn is_selected(includes: &[Glob<'_>], excludes: &[Glob<'_>], path: &str) -> bool {
    matches_any(includes, path) && !matches_any(excludes, path)
}

fn finalize_diagnostics(diagnostics: &mut [Diagnostic]) {
    sort_diagnostics(diagnostics);
}

fn candidate_source_path(root: &Path, path: &Path) -> Result<String, Box<Diagnostic>> {
    normalized_relative_path(root, path).ok_or_else(|| {
        Box::new(
            Diagnostic::new(
                ContentDiagnosticCode::Io,
                "content path is not valid UTF-8",
                None,
            )
            .with_detail("operation", "identify")
            .with_detail("reason", "non_utf8_path"),
        )
    })
}

fn index_alias_diagnostic(project: &LoadedProject, candidate: &Candidate) -> Diagnostic {
    let index_path = normalized_relative_path(&project.root, &project.index_path)
        .unwrap_or_else(|| "<configured-index>".to_owned());
    diagnostic_at_start(
        ProjectDiagnosticCode::DuplicateFile,
        &candidate.source_path,
        "selected content path aliases the configured index destination",
    )
    .with_detail("first_path", index_path)
    .with_detail("path", candidate.source_path.clone())
}

fn open_candidate(
    project: &LoadedProject,
    candidate: &Candidate,
) -> Result<OpenedCandidate, Box<Diagnostic>> {
    verify_parent_directory_policy(project, candidate)?;
    let logical_metadata = fs::symlink_metadata(&candidate.logical_path).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not inspect selected content path",
            )
            .with_detail("operation", "inspect"),
        )
    })?;
    let is_file_symlink = logical_metadata.file_type().is_symlink();
    if is_file_symlink && !project.content.allow_internal_file_symlinks {
        return Err(Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content file symlink is disabled by project configuration",
            )
            .with_detail("reason", "file_symlink_disabled"),
        ));
    }

    let resolved_path = fs::canonicalize(&candidate.logical_path).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not resolve content input",
            )
            .with_detail("operation", "resolve"),
        )
    })?;
    if !resolved_path.starts_with(&project.root) {
        return Err(Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content input resolves outside the project root",
            )
            .with_detail("reason", "outside_project_root"),
        ));
    }
    let metadata = fs::metadata(&resolved_path).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not inspect content input",
            )
            .with_detail("operation", "inspect"),
        )
    })?;
    if !metadata.is_file() {
        return Err(Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "selected content input is not a regular file",
            )
            .with_detail("operation", "inspect"),
        ));
    }

    let open_path = if is_file_symlink {
        resolved_path.as_path()
    } else {
        candidate.logical_path.as_path()
    };
    verify_parent_directory_policy(project, candidate)?;
    let file = open_read_no_follow(open_path)
        .map_err(|_| open_failure_diagnostic(project, candidate, "open"))?;
    let identity = file_identity(&file).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not identify opened content input",
            )
            .with_detail("operation", "identify"),
        )
    })?;
    verify_candidate_path(project, candidate, &resolved_path, identity)?;

    Ok(OpenedCandidate {
        file,
        resolved_path,
        identity,
    })
}

fn decode_candidate(
    candidate: &Candidate,
    mut file: fs::File,
) -> Result<SourceDocument, Box<Diagnostic>> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not read content input",
            )
            .with_detail("operation", "read"),
        )
    })?;
    let source = match std::str::from_utf8(&bytes) {
        Ok(source) => source,
        Err(error) => return Err(invalid_utf8_diagnostic(candidate, &bytes, error)),
    };
    SourceDocument::try_new(
        candidate.source_path.clone(),
        SourceText::new(source.to_owned()),
    )
    .map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "content source could not be represented as a Mara source document",
            )
            .with_detail("operation", "decode")
            .with_detail("reason", "invalid_source_document"),
        )
    })
}

fn invalid_utf8_diagnostic(
    candidate: &Candidate,
    bytes: &[u8],
    error: std::str::Utf8Error,
) -> Box<Diagnostic> {
    let offset = error.valid_up_to();
    let valid_prefix = std::str::from_utf8(&bytes[..offset])
        .expect("the prefix before a UTF-8 decoding error is valid");
    let Ok(index) = mara_core::SourceIndex::try_new(candidate.source_path.clone(), valid_prefix)
    else {
        return Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "selected content path cannot be indexed as Mara source provenance",
            )
            .with_detail("operation", "decode")
            .with_detail("reason", "invalid_source_path"),
        );
    };
    let (line, column) = index
        .coordinates_at(offset as u64)
        .expect("a UTF-8 error offset is a source boundary in its valid prefix");
    let primary = index
        .try_span(offset as u64, offset as u64, line, column, line, column)
        .expect("a UTF-8 error offset is a valid empty span in its prefix");
    Box::new(
        Diagnostic::new(
            ContentDiagnosticCode::InvalidUtf8,
            "content source is not valid UTF-8",
            Some(primary),
        )
        .with_detail("path", candidate.source_path.clone()),
    )
}

fn verify_candidate_path(
    project: &LoadedProject,
    candidate: &Candidate,
    expected_path: &Path,
    opened_identity: Option<FileIdentity>,
) -> Result<(), Box<Diagnostic>> {
    verify_parent_directory_policy(project, candidate)?;
    let logical_metadata = fs::symlink_metadata(&candidate.logical_path).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not recheck selected content path",
            )
            .with_detail("operation", "recheck"),
        )
    })?;
    let is_file_symlink = logical_metadata.file_type().is_symlink();
    if is_file_symlink && !project.content.allow_internal_file_symlinks {
        return Err(Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content path became a file symlink during verification",
            )
            .with_detail("reason", "file_symlink_disabled"),
        ));
    }
    let rechecked_path = fs::canonicalize(&candidate.logical_path).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not recheck opened content input",
            )
            .with_detail("operation", "recheck"),
        )
    })?;
    if !rechecked_path.starts_with(&project.root) || rechecked_path != expected_path {
        return Err(Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content input changed during containment verification",
            )
            .with_detail("reason", "path_changed_during_open"),
        ));
    }
    let recheck_open_path = if is_file_symlink {
        rechecked_path.as_path()
    } else {
        candidate.logical_path.as_path()
    };
    let rechecked_file = open_read_no_follow(recheck_open_path)
        .map_err(|_| open_failure_diagnostic(project, candidate, "reopen"))?;
    let rechecked_identity = file_identity(&rechecked_file).map_err(|_| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not recheck content input identity",
            )
            .with_detail("operation", "reidentify"),
        )
    })?;
    if matches!((opened_identity, rechecked_identity), (Some(left), Some(right)) if left != right) {
        return Err(Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content input changed during identity verification",
            )
            .with_detail("reason", "identity_changed_during_open"),
        ));
    }
    Ok(())
}

fn verify_parent_directory_policy(
    project: &LoadedProject,
    candidate: &Candidate,
) -> Result<(), Box<Diagnostic>> {
    if project.content.follow_directory_symlinks {
        return Ok(());
    }
    let relative = candidate
        .logical_path
        .strip_prefix(&project.root)
        .map_err(|_| {
            Box::new(
                diagnostic_at_start(
                    ProjectDiagnosticCode::PathOutsideRoot,
                    &candidate.source_path,
                    "content path is outside the project root",
                )
                .with_detail("reason", "outside_project_root"),
            )
        })?;
    let mut current = project.root.clone();
    for component in relative.parent().into_iter().flat_map(Path::components) {
        current.push(component);
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(Box::new(
                diagnostic_at_start(
                    ProjectDiagnosticCode::SymlinkRejected,
                    &candidate.source_path,
                    "content parent directory symlink is disabled by project configuration",
                )
                .with_detail("reason", "directory_symlink_disabled"),
            ));
        }
    }
    Ok(())
}

fn open_failure_diagnostic(
    project: &LoadedProject,
    candidate: &Candidate,
    operation: &'static str,
) -> Box<Diagnostic> {
    if !project.content.allow_internal_file_symlinks
        && fs::symlink_metadata(&candidate.logical_path)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        Box::new(
            diagnostic_at_start(
                ProjectDiagnosticCode::SymlinkRejected,
                &candidate.source_path,
                "content file symlink is disabled by project configuration",
            )
            .with_detail("reason", "file_symlink_disabled"),
        )
    } else {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not open content input",
            )
            .with_detail("operation", operation),
        )
    }
}

fn normalized_relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut output = String::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return None;
        };
        let segment = segment.to_str()?;
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str(segment);
    }
    (!output.is_empty()).then_some(output)
}

fn lossy_relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut output = String::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return None;
        };
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str(&segment.to_string_lossy());
    }
    (!output.is_empty()).then_some(output)
}

fn walk_error_path(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::Partial(errors) => errors.iter().find_map(walk_error_path),
        ignore::Error::WithLineNumber { err, .. } | ignore::Error::WithDepth { err, .. } => {
            walk_error_path(err)
        }
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::Loop { child, .. } => Some(child),
        ignore::Error::Io(_)
        | ignore::Error::Glob { .. }
        | ignore::Error::UnrecognizedFileType(_)
        | ignore::Error::InvalidDefinition => None,
    }
}

fn walk_error_parts(error: &ignore::Error) -> Vec<&ignore::Error> {
    match error {
        ignore::Error::Partial(errors) => errors.iter().flat_map(walk_error_parts).collect(),
        _ => vec![error],
    }
}

fn is_walk_loop(error: &ignore::Error) -> bool {
    match error {
        ignore::Error::Partial(errors) => errors.iter().any(is_walk_loop),
        ignore::Error::WithLineNumber { err, .. }
        | ignore::Error::WithPath { err, .. }
        | ignore::Error::WithDepth { err, .. } => is_walk_loop(err),
        ignore::Error::Loop { .. } => true,
        ignore::Error::Io(_)
        | ignore::Error::Glob { .. }
        | ignore::Error::UnrecognizedFileType(_)
        | ignore::Error::InvalidDefinition => false,
    }
}

fn diagnostic_at_start(
    code: impl Into<mara_core::DiagnosticCode>,
    path: &str,
    message: impl Into<String>,
) -> Diagnostic {
    let primary = SourceSpan::try_new(path, "", 0, 0, 1, 1, 1, 1).ok();
    Diagnostic::new(code, message, primary).with_detail("path", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn non_utf8_candidate_paths_produce_pathless_diagnostics() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let root = Path::new("/project");
        let path = root.join(OsString::from_vec(b"invalid-\xff.mara.md".to_vec()));
        let diagnostic = candidate_source_path(root, &path).unwrap_err();

        assert_eq!(
            diagnostic.code(),
            mara_core::DiagnosticCode::Content(ContentDiagnosticCode::Io)
        );
        assert!(diagnostic.primary().is_none());
        assert_eq!(
            diagnostic.details().get("reason"),
            Some(&mara_core::DiagnosticValue::from("non_utf8_path"))
        );
    }

    #[test]
    fn pathless_failures_remain_independent_after_sorting() {
        let mut diagnostics = vec![
            Diagnostic::new(ContentDiagnosticCode::Io, "unsupported path", None),
            Diagnostic::new(ContentDiagnosticCode::Io, "unsupported path", None),
        ];

        finalize_diagnostics(&mut diagnostics);

        assert_eq!(diagnostics.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_directory_rejections_retain_failure_evidence() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let root = Path::new("/project");
        let path = root.join(OsString::from_vec(b"invalid-\xff".to_vec()));
        let rejected = Mutex::new(Vec::new());
        record_rejected_directory(&rejected, root, &path, "target could not be resolved");
        let [rejected] = rejected.into_inner().unwrap().try_into().unwrap();

        let diagnostic = rejected_directory_diagnostic(rejected);

        assert_eq!(
            diagnostic.code(),
            mara_core::DiagnosticCode::Project(ProjectDiagnosticCode::SymlinkRejected)
        );
        assert!(diagnostic.primary().is_none());
        assert_eq!(
            diagnostic.details().get("path_reason"),
            Some(&mara_core::DiagnosticValue::from("non_utf8_path"))
        );
    }
}
