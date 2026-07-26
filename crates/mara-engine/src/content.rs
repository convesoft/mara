//! Deterministic discovery and loading of configured Mara source documents.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

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

#[derive(Debug, Clone)]
struct WalkRoot {
    logical_path: PathBuf,
    physical_path: PathBuf,
    identity: Option<FileIdentity>,
    ancestor_identities: HashSet<FileIdentity>,
    ancestor_paths: HashSet<PathBuf>,
}

#[derive(Debug)]
struct RejectedDirectorySymlink {
    source_path: Option<String>,
    reason: &'static str,
}

#[derive(Debug)]
enum IncludeSegment {
    Recursive,
    Pattern(Glob<'static>),
}

#[derive(Debug)]
struct IncludePattern {
    segments: Vec<IncludeSegment>,
}

#[derive(Debug, Default)]
struct IgnoredPaths {
    exact: HashSet<PathBuf>,
    trees: Vec<PathBuf>,
}

impl IgnoredPaths {
    fn contains(&self, path: &Path) -> bool {
        self.exact.contains(path) || self.trees.iter().any(|tree| path.starts_with(tree))
    }
}

#[derive(Debug)]
struct IgnoreRuleFile {
    path: PathBuf,
    follow_symlink: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct IgnoreRuleFailure {
    path: PathBuf,
}

#[derive(Debug)]
struct GitContext {
    ignored_paths: io::Result<IgnoredPaths>,
    ignore_rule_files: Vec<IgnoreRuleFile>,
}

/// Discovers and decodes the content selected by an already validated project.
/// Documents and diagnostics are both returned in normalized project-relative
/// path order.
pub fn discover_content(project: &LoadedProject) -> ContentDiscovery {
    let includes = compile_globs(&project.content.include);
    let excludes = compile_globs(&project.content.exclude);
    let git_context = if project.content.respect_gitignore {
        git_context(&project.root)
    } else {
        Ok(None)
    };
    let rejected_directories = Arc::new(Mutex::new(Vec::new()));
    let ignore_rule_failures = Arc::new(Mutex::new(Vec::new()));
    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let (respect_gitignore, ignored_paths) = match git_context {
        Ok(Some(GitContext {
            ignored_paths,
            ignore_rule_files,
        })) => {
            for rule in ignore_rule_files {
                record_unreadable_ignore_file(
                    &ignore_rule_failures,
                    &rule.path,
                    rule.follow_symlink,
                );
            }
            match ignored_paths {
                Ok(paths) => (true, paths),
                Err(_) => {
                    diagnostics.push(gitignore_query_diagnostic(
                        project,
                        "ignore_query_failed",
                        "could not enumerate Git-ignored content paths",
                    ));
                    (true, IgnoredPaths::default())
                }
            }
        }
        Ok(None) => (false, IgnoredPaths::default()),
        Err(_) => {
            diagnostics.push(gitignore_query_diagnostic(
                project,
                "context_query_failed",
                "could not establish the Git worktree context",
            ));
            (false, IgnoredPaths::default())
        }
    };
    let ignored_paths = Arc::new(ignored_paths);
    let mut seen_logical_paths = HashSet::new();
    let root_identity = fs::metadata(&project.root)
        .ok()
        .and_then(|metadata| path_identity(&project.root, &metadata).ok().flatten());
    let root_walk = WalkRoot {
        logical_path: project.root.clone(),
        physical_path: project.root.clone(),
        identity: root_identity,
        ancestor_identities: root_identity.into_iter().collect(),
        ancestor_paths: HashSet::from([project.root.clone()]),
    };
    let mut queued_walk_roots = HashSet::from([project.root.clone()]);
    let mut walk_roots = VecDeque::from([root_walk]);

    while let Some(walk_root) = walk_roots.pop_front() {
        let walk_root = match revalidate_walk_root(project, walk_root) {
            Ok(walk_root) => walk_root,
            Err(diagnostic) => {
                diagnostics.push(*diagnostic);
                continue;
            }
        };
        for result in content_walker(
            project,
            &walk_root,
            respect_gitignore,
            Arc::clone(&ignored_paths),
            Arc::clone(&rejected_directories),
            Arc::clone(&ignore_rule_failures),
        )
        .build()
        {
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    for error in walk_error_parts(&error) {
                        let error_path = walk_error_path(error);
                        let logical_error_path =
                            error_path.map(|path| logical_walk_path(&walk_root, path));
                        let source_path = logical_error_path
                            .as_deref()
                            .and_then(|path| normalized_relative_path(&project.root, path));
                        if source_path.as_deref().is_some_and(|path| {
                            is_fully_excluded_tree(&project.content.exclude, path)
                        }) {
                            continue;
                        }
                        let is_loop = is_walk_loop(error);
                        if !is_loop
                            && error_path.is_some_and(|path| {
                                fs::symlink_metadata(path).is_ok_and(|metadata| !metadata.is_dir())
                            })
                            && source_path
                                .as_deref()
                                .is_some_and(|path| !is_selected(&includes, &excludes, path))
                        {
                            continue;
                        }
                        let reason = if is_loop {
                            "directory_cycle"
                        } else {
                            "walk_error"
                        };
                        let diagnostic = match source_path {
                            Some(source_path) => diagnostic_at_start(
                                ContentDiagnosticCode::Io,
                                &source_path,
                                "could not inspect a configured content path",
                            ),
                            None => Diagnostic::new(
                                ContentDiagnosticCode::Io,
                                "could not inspect a configured content path",
                                None,
                            ),
                        };
                        diagnostics.push(
                            diagnostic
                                .with_detail("operation", "discover")
                                .with_detail("reason", reason),
                        );
                    }
                    continue;
                }
            };
            if let Some(error) = entry.error() {
                let logical_entry_path = logical_walk_path(&walk_root, entry.path());
                let affected_path = normalized_relative_path(&project.root, &logical_entry_path);
                if affected_path
                    .as_deref()
                    .is_some_and(|path| is_fully_excluded_tree(&project.content.exclude, path))
                {
                    continue;
                }
                if entry.file_type().is_some_and(|kind| kind.is_file())
                    && affected_path
                        .as_deref()
                        .is_some_and(|path| !is_selected(&includes, &excludes, path))
                {
                    continue;
                }
                for error in walk_error_parts(error) {
                    let source_path = walk_error_path(error)
                        .map(|path| logical_walk_path(&walk_root, path))
                        .as_deref()
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
            if entry.path_is_symlink()
                && fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir())
            {
                if project.content.follow_directory_symlinks {
                    queue_directory_symlink(
                        project,
                        &walk_root,
                        entry.path(),
                        &logical_walk_path(&walk_root, entry.path()),
                        &mut queued_walk_roots,
                        &mut walk_roots,
                        &mut diagnostics,
                    );
                }
                continue;
            }
            let logical_path = logical_walk_path(&walk_root, entry.path());
            if !seen_logical_paths.insert(logical_path.clone()) {
                continue;
            }
            match candidate_from_path(&project.root, logical_path, &includes, &excludes) {
                Ok(Some(candidate)) => candidates.push(candidate),
                Ok(None) => {}
                Err(diagnostic) => diagnostics.push(*diagnostic),
            }
        }
    }

    let rejected_directories = Arc::try_unwrap(rejected_directories)
        .expect("the content walker released its directory filter")
        .into_inner()
        .expect("the directory rejection lock is not poisoned");
    for rejected in rejected_directories {
        diagnostics.push(rejected_directory_diagnostic(rejected));
    }
    let mut ignore_rule_failures = Arc::try_unwrap(ignore_rule_failures)
        .expect("the content walker released its ignore-rule failure recorder")
        .into_inner()
        .expect("the ignore-rule failure lock is not poisoned");
    ignore_rule_failures.sort();
    ignore_rule_failures.dedup();
    for failure in ignore_rule_failures {
        diagnostics.push(ignore_rule_file_diagnostic(&project.root, failure));
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

fn logical_walk_path(walk_root: &WalkRoot, physical_path: &Path) -> PathBuf {
    physical_path
        .strip_prefix(&walk_root.physical_path)
        .map(|relative| walk_root.logical_path.join(relative))
        .unwrap_or_else(|_| walk_root.logical_path.clone())
}

fn walk_root_diagnostic(
    project: &LoadedProject,
    walk_root: &WalkRoot,
    message: &'static str,
    reason: &'static str,
) -> Diagnostic {
    let source_path = normalized_relative_path(&project.root, &walk_root.logical_path);
    let diagnostic = match source_path {
        Some(source_path) => diagnostic_at_start(ContentDiagnosticCode::Io, &source_path, message),
        None => Diagnostic::new(ContentDiagnosticCode::Io, message, None),
    };
    diagnostic
        .with_detail("operation", "discover")
        .with_detail("reason", reason)
}

fn revalidate_walk_root(
    project: &LoadedProject,
    mut walk_root: WalkRoot,
) -> Result<WalkRoot, Box<Diagnostic>> {
    let resolved_path = fs::canonicalize(&walk_root.physical_path).map_err(|_| {
        Box::new(walk_root_diagnostic(
            project,
            &walk_root,
            "content directory changed before it could be inspected",
            "directory_changed",
        ))
    })?;
    if !resolved_path.starts_with(&project.root) {
        return Err(Box::new(walk_root_diagnostic(
            project,
            &walk_root,
            "content directory resolved outside the project root before inspection",
            "directory_outside_root",
        )));
    }
    let metadata = fs::metadata(&resolved_path).map_err(|_| {
        Box::new(walk_root_diagnostic(
            project,
            &walk_root,
            "content directory changed before it could be inspected",
            "directory_changed",
        ))
    })?;
    if !metadata.is_dir() {
        return Err(Box::new(walk_root_diagnostic(
            project,
            &walk_root,
            "content directory became a non-directory before inspection",
            "directory_changed",
        )));
    }
    let identity = path_identity(&resolved_path, &metadata)
        .map_err(|_| {
            Box::new(walk_root_diagnostic(
                project,
                &walk_root,
                "content directory identity could not be verified",
                "directory_identity_unavailable",
            ))
        })?
        .or(walk_root.identity);
    if matches!((walk_root.identity, identity), (Some(expected), Some(actual)) if expected != actual)
    {
        return Err(Box::new(walk_root_diagnostic(
            project,
            &walk_root,
            "content directory changed before it could be inspected",
            "directory_changed",
        )));
    }
    walk_root.physical_path = resolved_path;
    walk_root.identity = identity;
    Ok(walk_root)
}

fn content_walker(
    project: &LoadedProject,
    walk_root: &WalkRoot,
    respect_gitignore: bool,
    ignored_paths: Arc<IgnoredPaths>,
    rejected: Arc<Mutex<Vec<RejectedDirectorySymlink>>>,
    ignore_rule_failures: Arc<Mutex<Vec<IgnoreRuleFailure>>>,
) -> WalkBuilder {
    let mut builder = WalkBuilder::new(&walk_root.physical_path);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_exclude(false)
        .git_global(false)
        .parents(false)
        .require_git(true)
        .follow_links(false);

    let root = project.root.clone();
    let logical_walk_root = walk_root.logical_path.clone();
    let physical_walk_root = walk_root.physical_path.clone();
    let include_patterns = compile_include_patterns(&project.content.include);
    let exclude_patterns = project.content.exclude.clone();
    let follow_directory_symlinks = project.content.follow_directory_symlinks;
    builder.filter_entry(move |entry| {
        if entry.depth() == 0 {
            if respect_gitignore {
                record_unreadable_ignore_file(
                    &ignore_rule_failures,
                    &entry.path().join(".gitignore"),
                    false,
                );
            }
            return true;
        }
        if respect_gitignore && ignored_paths.contains(entry.path()) {
            return false;
        }
        let logical_path = entry
            .path()
            .strip_prefix(&physical_walk_root)
            .map(|relative| logical_walk_root.join(relative))
            .unwrap_or_else(|_| logical_walk_root.clone());
        let is_directory = entry.file_type().is_some_and(|kind| kind.is_dir())
            || (entry.path_is_symlink()
                && fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()));
        if is_directory
            && let Some(path) = lossy_relative_path(&root, &logical_path)
            && (is_fully_excluded_tree(&exclude_patterns, &path)
                || !directory_is_include_reachable(&include_patterns, &path))
        {
            return false;
        }
        if respect_gitignore && is_directory {
            record_unreadable_ignore_file(
                &ignore_rule_failures,
                &entry.path().join(".gitignore"),
                false,
            );
        }
        if !entry.path_is_symlink() {
            return true;
        }
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
                    &logical_path,
                    "resolved target is outside the project root",
                );
                false
            }
            Err(_) => {
                record_rejected_directory(
                    &rejected,
                    &root,
                    &logical_path,
                    "target could not be resolved",
                );
                false
            }
        }
    });
    builder
}

fn queue_directory_symlink(
    project: &LoadedProject,
    walk_root: &WalkRoot,
    physical_path: &Path,
    logical_path: &Path,
    queued: &mut HashSet<PathBuf>,
    pending: &mut VecDeque<WalkRoot>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = match fs::canonicalize(physical_path) {
        Ok(target) => target,
        Err(_) => {
            diagnostics.push(directory_discovery_diagnostic(
                project,
                logical_path,
                "content directory symlink changed before it could be queued",
                "directory_changed",
            ));
            return;
        }
    };
    if !target.starts_with(&project.root) {
        diagnostics.push(directory_discovery_diagnostic(
            project,
            logical_path,
            "content directory symlink resolved outside the project root before it could be queued",
            "directory_outside_root",
        ));
        return;
    }
    let metadata = match fs::metadata(&target) {
        Ok(metadata) => metadata,
        Err(_) => {
            diagnostics.push(directory_discovery_diagnostic(
                project,
                logical_path,
                "content directory symlink changed before it could be queued",
                "directory_changed",
            ));
            return;
        }
    };
    if !metadata.is_dir() {
        diagnostics.push(directory_discovery_diagnostic(
            project,
            logical_path,
            "content directory symlink became a non-directory before it could be queued",
            "directory_changed",
        ));
        return;
    }
    let identity = path_identity(&target, &metadata).ok().flatten();
    let mut ancestor_identities = walk_root.ancestor_identities.clone();
    let mut ancestor_paths = walk_root.ancestor_paths.clone();
    let mut ancestor = physical_path.parent();
    while let Some(path) = ancestor.filter(|path| path.starts_with(&walk_root.physical_path)) {
        if let Ok(path) = fs::canonicalize(path) {
            ancestor_paths.insert(path);
        }
        if let Ok(metadata) = fs::metadata(path)
            && let Ok(Some(identity)) = path_identity(path, &metadata)
        {
            ancestor_identities.insert(identity);
        }
        if path == walk_root.physical_path {
            break;
        }
        ancestor = path.parent();
    }
    let canonical_parent = physical_path
        .parent()
        .and_then(|parent| fs::canonicalize(parent).ok());
    let forms_cycle = identity.is_some_and(|identity| ancestor_identities.contains(&identity))
        || ancestor_paths.contains(&target)
        || canonical_parent.is_some_and(|parent| parent.starts_with(&target));
    if forms_cycle {
        let source_path = normalized_relative_path(&project.root, logical_path);
        let diagnostic = match source_path {
            Some(source_path) => diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &source_path,
                "content directory symlink forms a traversal cycle",
            ),
            None => Diagnostic::new(
                ContentDiagnosticCode::Io,
                "content directory symlink with a non-UTF-8 path forms a traversal cycle",
                None,
            ),
        };
        diagnostics.push(
            diagnostic
                .with_detail("operation", "discover")
                .with_detail("reason", "directory_cycle"),
        );
        return;
    }
    if let Some(identity) = identity {
        ancestor_identities.insert(identity);
    }
    let logical_path = logical_path.to_path_buf();
    if queued.insert(logical_path.clone()) {
        ancestor_paths.insert(target.clone());
        pending.push_back(WalkRoot {
            logical_path,
            physical_path: target,
            identity,
            ancestor_identities,
            ancestor_paths,
        });
    }
}

fn directory_discovery_diagnostic(
    project: &LoadedProject,
    logical_path: &Path,
    message: &'static str,
    reason: &'static str,
) -> Diagnostic {
    let source_path = normalized_relative_path(&project.root, logical_path);
    let diagnostic = match source_path {
        Some(source_path) => diagnostic_at_start(ContentDiagnosticCode::Io, &source_path, message),
        None => Diagnostic::new(ContentDiagnosticCode::Io, message, None),
    };
    diagnostic
        .with_detail("operation", "discover")
        .with_detail("reason", reason)
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
            let close = content_glob_class_close(&characters, index)
                .expect("the project loader validated every character class");
            expression.push('[');
            let mut content_index = index + 1;
            if characters[content_index] == '!' {
                expression.push('!');
                content_index += 1;
            }
            while content_index < close {
                let character = characters[content_index];
                if matches!(character, '[' | ']') {
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

pub(crate) fn content_glob_class_close(characters: &[char], open: usize) -> Option<usize> {
    let mut search_start = open.checked_add(1)?;
    if characters.get(search_start) == Some(&'!') {
        search_start += 1;
    }
    if characters.get(search_start) == Some(&']') {
        search_start += 1;
    }
    characters
        .get(search_start..)?
        .iter()
        .position(|character| *character == ']')
        .map(|offset| search_start + offset)
}

fn matches_any(patterns: &[Glob<'_>], path: &str) -> bool {
    patterns.iter().any(|pattern| pattern.is_match(path))
}

fn is_selected(includes: &[Glob<'_>], excludes: &[Glob<'_>], path: &str) -> bool {
    matches_any(includes, path) && !matches_any(excludes, path)
}

pub(crate) fn select_configured_content_paths(
    project: &LoadedProject,
    paths: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let includes = compile_globs(&project.content.include);
    let excludes = compile_globs(&project.content.exclude);
    paths
        .into_iter()
        .filter(|path| is_selected(&includes, &excludes, path))
        .collect()
}

fn is_fully_excluded_tree(excludes: &[String], path: &str) -> bool {
    excludes.iter().any(|pattern| {
        let segments = pattern.split('/').collect::<Vec<_>>();
        let mut suffix_start = segments.len();
        let mut recursive = false;
        let mut single_star = false;
        for (index, segment) in segments.iter().enumerate().rev() {
            match *segment {
                "**" => {
                    recursive = true;
                    suffix_start = index;
                }
                "*" if !single_star => {
                    single_star = true;
                    suffix_start = index;
                }
                _ => break,
            }
        }
        if !recursive {
            return false;
        }
        let root_pattern = segments[..suffix_start].join("/");
        if root_pattern.is_empty() {
            return true;
        }
        let Ok(root_pattern) = compile_content_glob(&root_pattern) else {
            return false;
        };
        path.split('/')
            .scan(String::new(), |ancestor, segment| {
                if !ancestor.is_empty() {
                    ancestor.push('/');
                }
                ancestor.push_str(segment);
                Some(ancestor.clone())
            })
            .any(|ancestor| root_pattern.is_match(ancestor.as_str()))
    })
}

fn compile_include_patterns(patterns: &[String]) -> Vec<IncludePattern> {
    patterns
        .iter()
        .map(|pattern| {
            let mut segments = Vec::new();
            for segment in pattern.split('/') {
                if segment == "**" {
                    if !matches!(segments.last(), Some(IncludeSegment::Recursive)) {
                        segments.push(IncludeSegment::Recursive);
                    }
                } else {
                    segments.push(IncludeSegment::Pattern(
                        compile_content_glob(segment)
                            .expect("the project loader validated every content glob segment"),
                    ));
                }
            }
            IncludePattern { segments }
        })
        .collect()
}

fn directory_is_include_reachable(includes: &[IncludePattern], path: &str) -> bool {
    includes
        .iter()
        .any(|pattern| include_can_match_descendant(pattern, path))
}

fn include_can_match_descendant(pattern: &IncludePattern, path: &str) -> bool {
    let mut states = epsilon_closure(&pattern.segments, vec![0]);
    for segment in path.split('/') {
        let mut next = Vec::new();
        for state in states {
            match pattern.segments.get(state) {
                Some(IncludeSegment::Recursive) => next.push(state),
                Some(IncludeSegment::Pattern(glob)) if glob.is_match(segment) => {
                    next.push(state + 1);
                }
                Some(IncludeSegment::Pattern(_)) | None => {}
            }
        }
        states = epsilon_closure(&pattern.segments, next);
        if states.is_empty() {
            return false;
        }
    }
    states
        .into_iter()
        .any(|state| state < pattern.segments.len())
}

fn epsilon_closure(segments: &[IncludeSegment], mut states: Vec<usize>) -> Vec<usize> {
    let mut index = 0;
    while index < states.len() {
        let state = states[index];
        if matches!(segments.get(state), Some(IncludeSegment::Recursive))
            && !states.contains(&(state + 1))
        {
            states.push(state + 1);
        }
        index += 1;
    }
    states.sort_unstable();
    states.dedup();
    states
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

fn candidate_from_path(
    root: &Path,
    logical_path: PathBuf,
    includes: &[Glob<'_>],
    excludes: &[Glob<'_>],
) -> Result<Option<Candidate>, Box<Diagnostic>> {
    let source_path = match candidate_source_path(root, &logical_path) {
        Ok(source_path) => source_path,
        Err(diagnostic) => {
            if lossy_relative_path(root, &logical_path)
                .as_deref()
                .is_some_and(|path| !is_selected(includes, excludes, path))
            {
                return Ok(None);
            }
            return Err(diagnostic);
        }
    };
    if !is_selected(includes, excludes, &source_path) {
        return Ok(None);
    }
    if SourceSpan::try_new(&source_path, "", 0, 0, 1, 1, 1, 1).is_err() {
        return Err(Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &source_path,
                "selected content path cannot be represented as Mara source provenance",
            )
            .with_detail("operation", "identify")
            .with_detail("reason", "invalid_source_path"),
        ));
    }
    Ok(Some(Candidate {
        logical_path,
        source_path,
    }))
}

fn git_context(root: &Path) -> io::Result<Option<GitContext>> {
    let inside = match git_output(root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !inside.status.success() {
        return Ok(None);
    }
    if !parse_git_worktree_response(&inside.stdout)? {
        return Ok(None);
    }
    let worktree_root = git_output(root, &["rev-parse", "--show-toplevel"])?;
    if !worktree_root.status.success() {
        return Err(io::Error::other("Git could not locate the worktree root"));
    }
    let worktree_root = git_path_from_output(root, &worktree_root.stdout)?;
    let mut ignore_rule_files = vec![IgnoreRuleFile {
        path: root.join(".gitignore"),
        follow_symlink: false,
    }];
    let mut ancestor = root.parent();
    while let Some(directory) = ancestor.filter(|directory| directory.starts_with(&worktree_root)) {
        ignore_rule_files.push(IgnoreRuleFile {
            path: directory.join(".gitignore"),
            follow_symlink: false,
        });
        if directory == worktree_root {
            break;
        }
        ancestor = directory.parent();
    }
    let info_exclude = git_output(root, &["rev-parse", "--git-path", "info/exclude"])?;
    if !info_exclude.status.success() {
        return Err(io::Error::other("Git could not locate info/exclude"));
    }
    ignore_rule_files.push(IgnoreRuleFile {
        path: git_path_from_output(root, &info_exclude.stdout)?,
        follow_symlink: true,
    });
    let configured_excludes = git_output(
        root,
        &["config", "--null", "--path", "--get", "core.excludesFile"],
    )?;
    if configured_excludes.status.success() {
        ignore_rule_files.push(IgnoreRuleFile {
            path: git_path_from_output(root, &configured_excludes.stdout)?,
            follow_symlink: true,
        });
    } else if configured_excludes.status.code() != Some(1) {
        return Err(io::Error::other(
            "Git could not resolve the configured excludes file",
        ));
    }
    ignore_rule_files.sort_by(|left, right| left.path.cmp(&right.path));
    ignore_rule_files.dedup_by(|left, right| left.path == right.path);
    Ok(Some(GitContext {
        ignored_paths: ignored_content_paths(root),
        ignore_rule_files,
    }))
}

fn parse_git_worktree_response(output: &[u8]) -> io::Result<bool> {
    match output.strip_suffix(b"\n").unwrap_or(output) {
        b"true" => Ok(true),
        b"false" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git returned an invalid worktree response",
        )),
    }
}

fn ignored_content_paths(root: &Path) -> io::Result<IgnoredPaths> {
    let output = git_output(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "--",
            ".",
        ],
    )?;
    if !output.status.success() {
        return Err(io::Error::other(
            "Git could not enumerate ignored project paths",
        ));
    }
    let mut ignored = IgnoredPaths::default();
    for path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let is_tree = path.last() == Some(&b'/');
        let path = if is_tree {
            &path[..path.len() - 1]
        } else {
            path
        };
        let path = git_path(root, path)?;
        if is_tree {
            ignored.trees.push(path);
        } else {
            ignored.exact.insert(path);
        }
    }
    ignored.trees.sort();
    ignored.trees.dedup();
    Ok(ignored)
}

fn git_output(root: &Path, args: &[&str]) -> io::Result<std::process::Output> {
    Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .args(["-c", "core.fsmonitor=false"])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
}

fn git_path_from_output(root: &Path, output: &[u8]) -> io::Result<PathBuf> {
    let output = output
        .strip_suffix(b"\0")
        .or_else(|| output.strip_suffix(b"\r\n"))
        .or_else(|| output.strip_suffix(b"\n"))
        .unwrap_or(output);
    if output.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git returned an empty path",
        ));
    }
    git_path(root, output)
}

fn record_unreadable_ignore_file(
    failures: &Mutex<Vec<IgnoreRuleFailure>>,
    path: &Path,
    follow_symlink: bool,
) {
    let logical_metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(_) => None,
        Ok(metadata) => Some(metadata),
    };
    if logical_metadata
        .as_ref()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
        && !follow_symlink
    {
        return;
    }
    if logical_metadata.is_none() || !ignore_rule_is_readable(path) {
        failures
            .lock()
            .expect("the ignore-rule failure lock is not poisoned")
            .push(IgnoreRuleFailure {
                path: path.to_path_buf(),
            });
    }
}

#[cfg(unix)]
fn ignore_rule_is_readable(path: &Path) -> bool {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .and_then(|file| file.metadata())
        .is_ok_and(|metadata| !metadata.is_dir())
}

#[cfg(not(unix))]
fn ignore_rule_is_readable(path: &Path) -> bool {
    fs::File::open(path)
        .and_then(|file| file.metadata())
        .is_ok_and(|metadata| !metadata.is_dir())
}

fn gitignore_query_diagnostic(
    project: &LoadedProject,
    reason: &'static str,
    message: &'static str,
) -> Diagnostic {
    let source_path = normalized_relative_path(&project.root, &project.config_path)
        .unwrap_or_else(|| ".mara/project.toml".to_owned());
    diagnostic_at_start(ContentDiagnosticCode::Io, &source_path, message)
        .with_detail("operation", "gitignore")
        .with_detail("reason", reason)
}

fn ignore_rule_file_diagnostic(root: &Path, failure: IgnoreRuleFailure) -> Diagnostic {
    let diagnostic = match normalized_relative_path(root, &failure.path) {
        Some(source_path) => diagnostic_at_start(
            ContentDiagnosticCode::Io,
            &source_path,
            "could not read a Git ignore rule file",
        ),
        None => Diagnostic::new(
            ContentDiagnosticCode::Io,
            "could not read an external Git ignore rule file",
            None,
        ),
    };
    diagnostic
        .with_detail("operation", "gitignore")
        .with_detail("reason", "ignore_rule_io")
}

#[cfg(unix)]
fn git_path(root: &Path, path: &[u8]) -> std::io::Result<PathBuf> {
    Ok(root.join(OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
fn git_path(root: &Path, path: &[u8]) -> std::io::Result<PathBuf> {
    let path = std::str::from_utf8(path)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(root.join(OsString::from(path)))
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
        .map_err(|error| open_failure_diagnostic(project, candidate, "open", &error))?;
    verify_opened_regular_file(candidate, &file)?;
    let identity = file_identity(&file).map_err(|error| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not identify opened content input",
            )
            .with_detail("operation", "identify")
            .with_detail("cause", error.to_string()),
        )
    })?;
    verify_candidate_path(project, candidate, &resolved_path, identity)?;

    Ok(OpenedCandidate {
        file,
        resolved_path,
        identity,
    })
}

fn verify_opened_regular_file(
    candidate: &Candidate,
    file: &fs::File,
) -> Result<(), Box<Diagnostic>> {
    let metadata = file.metadata().map_err(|error| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not inspect opened content input",
            )
            .with_detail("operation", "inspect")
            .with_detail("cause", error.to_string()),
        )
    })?;
    if metadata.is_file() {
        return Ok(());
    }
    Err(Box::new(
        diagnostic_at_start(
            ContentDiagnosticCode::Io,
            &candidate.source_path,
            "opened content input is not a regular file",
        )
        .with_detail("operation", "inspect")
        .with_detail("reason", "opened_not_regular"),
    ))
}

fn decode_candidate(
    candidate: &Candidate,
    mut file: fs::File,
) -> Result<SourceDocument, Box<Diagnostic>> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        Box::new(
            diagnostic_at_start(
                ContentDiagnosticCode::Io,
                &candidate.source_path,
                "could not read content input",
            )
            .with_detail("operation", "read")
            .with_detail("cause", error.to_string()),
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
        .map_err(|error| open_failure_diagnostic(project, candidate, "reopen", &error))?;
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
    error: &std::io::Error,
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
            .with_detail("operation", operation)
            .with_detail("cause", error.to_string()),
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
    fn loaded_project(root: PathBuf) -> LoadedProject {
        LoadedProject {
            config_path: root.join(".mara/project.toml"),
            format_version: 1,
            name: "test".to_owned(),
            schema_source_path: ".mara/schema.yaml".to_owned(),
            schema_path: root.join(".mara/schema.yaml"),
            content: crate::project::ContentConfig {
                include: vec!["**/*.mara.md".to_owned()],
                exclude: Vec::new(),
                respect_gitignore: false,
                follow_directory_symlinks: true,
                allow_internal_file_symlinks: false,
            },
            index_path: root.join(".mara/index.json"),
            validation: crate::project::ValidationConfig {
                warnings_as_errors: false,
            },
            git: crate::project::GitConfig {
                require_clean_worktree_for_writes: true,
            },
            root,
        }
    }

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

    #[test]
    fn opened_content_handles_must_still_be_regular_files() {
        let directory = tempfile::tempdir().unwrap();

        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;

            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            fs::OpenOptions::new()
                .access_mode(0)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(directory.path())
                .unwrap()
        };
        #[cfg(not(windows))]
        let file = fs::File::open(directory.path()).unwrap();
        let candidate = Candidate {
            logical_path: directory.path().to_path_buf(),
            source_path: "source.mara.md".to_owned(),
        };

        let diagnostic = verify_opened_regular_file(&candidate, &file).unwrap_err();

        assert_eq!(
            diagnostic.details().get("reason"),
            Some(&mara_core::DiagnosticValue::from("opened_not_regular"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn queued_walk_roots_are_rejected_if_their_physical_path_is_redirected() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("project");
        let outside = fixture.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let metadata = fs::metadata(&target).unwrap();
        let identity = path_identity(&target, &metadata).unwrap();
        let walk_root = WalkRoot {
            logical_path: root.join("alias"),
            physical_path: target.clone(),
            identity,
            ancestor_identities: identity.into_iter().collect(),
            ancestor_paths: HashSet::from([target.clone()]),
        };
        fs::rename(&target, root.join("moved-target")).unwrap();
        symlink(&outside, &target).unwrap();

        let diagnostic = revalidate_walk_root(&loaded_project(root), walk_root).unwrap_err();

        assert_eq!(
            diagnostic.details().get("reason"),
            Some(&mara_core::DiagnosticValue::from("directory_outside_root"))
        );
    }

    #[test]
    fn git_false_response_means_worktree_context_is_unavailable() {
        assert!(parse_git_worktree_response(b"true\n").unwrap());
        assert!(!parse_git_worktree_response(b"false\n").unwrap());
        assert!(parse_git_worktree_response(b"unknown\n").is_err());
    }

    #[test]
    fn include_patterns_prune_only_unreachable_directories() {
        let includes = compile_include_patterns(&["docs/sources/**/*.mara.md".to_owned()]);
        assert!(directory_is_include_reachable(&includes, "docs"));
        assert!(directory_is_include_reachable(&includes, "docs/sources"));
        assert!(directory_is_include_reachable(
            &includes,
            "docs/sources/nested"
        ));
        assert!(!directory_is_include_reachable(&includes, "docs/other"));
        assert!(!directory_is_include_reachable(&includes, "vendor"));
        let recursive = compile_include_patterns(&["**/*.mara.md".to_owned()]);
        assert!(directory_is_include_reachable(&recursive, "vendor"));
        let shallow = compile_include_patterns(&["docs/*.mara.md".to_owned()]);
        assert!(directory_is_include_reachable(&shallow, "docs"));
        assert!(!directory_is_include_reachable(&shallow, "docs/nested"));
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
