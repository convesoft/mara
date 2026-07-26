//! Deterministic versioned JSON index projection and atomic writer.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
};

use mara_core::{
    AuthoredReference, AuthoredReferenceSyntax, Diagnostic, DiagnosticValue, NodeRef,
    NormalizedItem, NormalizedScalar, QueryGraph, ReferenceOrigin, SchemaDocument, SourceSpan,
};
use mara_markdown::{NarrativeKind, ParsedBlock, ParsedDocument, ParsedItem, ParsedSection};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    SemanticCompilation, ValidationResult,
    content::select_configured_content_paths,
    project::LoadedProject,
    semantic::{relation_inverse_wire_name, relation_occurrence_wire_origin},
};

const PROJECT_CONFIG_PATH: &str = ".mara/project.toml";

/// A complete canonical `mara.index` version-1 projection.
#[derive(Debug, Clone, Serialize)]
pub struct IndexProjection {
    format: &'static str,
    version: u8,
    project: IndexProjectWire,
    git: GitWire,
    documents: Vec<DocumentWire>,
    items: Vec<ItemWire>,
    source_nodes: Vec<SourceNodeWire>,
    edges: Vec<EdgeWire>,
    mentions: Vec<MentionWire>,
    external_nodes: Vec<ExternalNodeWire>,
    diagnostics: Vec<DiagnosticWire>,
}

#[derive(Debug, Clone, Copy)]
struct ValidatedSchemaSnapshot<'a> {
    model: &'a SchemaDocument,
    source: &'a [u8],
}

impl IndexProjection {
    /// Builds the complete projection from one already validated immutable project result.
    pub fn from_validation(result: &ValidationResult) -> Result<Self, IndexError> {
        if !result.is_valid() {
            return Err(IndexError::InvalidModel {
                reason: "validation policy rejects the project",
            });
        }
        let project = result.project().ok_or(IndexError::InvalidModel {
            reason: "project model is unavailable",
        })?;
        let schema = result.schema().ok_or(IndexError::InvalidModel {
            reason: "schema model is unavailable",
        })?;
        let semantic = result.semantic().ok_or(IndexError::InvalidModel {
            reason: "semantic model is unavailable",
        })?;
        let graph = result.graph().ok_or(IndexError::InvalidModel {
            reason: "graph model is unavailable",
        })?;
        let schema_source = result.schema_source().ok_or(IndexError::InvalidModel {
            reason: "validated schema source is unavailable",
        })?;

        let git = GitWire::discover(project, result.documents(), result.content_paths())?;
        result
            .git_anchor()
            .ok_or(IndexError::InvalidModel {
                reason: "validation Git anchor is unavailable",
            })?
            .verify(git.anchor.as_ref())?;
        Self::from_parts(
            project,
            ValidatedSchemaSnapshot {
                model: schema,
                source: schema_source,
            },
            result.documents(),
            semantic,
            graph,
            result.diagnostics(),
            git.wire,
        )
    }

    fn from_parts(
        project: &LoadedProject,
        schema: ValidatedSchemaSnapshot<'_>,
        documents: &[ParsedDocument],
        semantic: &SemanticCompilation,
        graph: &QueryGraph,
        diagnostics: &[Diagnostic],
        git: GitWire,
    ) -> Result<Self, IndexError> {
        let components = ProjectionComponents::build(schema.model, semantic, graph);

        let parsed_items = documents
            .iter()
            .flat_map(ParsedDocument::items)
            .map(|item| (item.mid().clone(), item))
            .collect::<BTreeMap<_, _>>();
        let mut items = semantic
            .items()
            .iter()
            .map(|item| {
                let parsed =
                    parsed_items
                        .get(item.mid())
                        .copied()
                        .ok_or(IndexError::InvalidModel {
                            reason: "normalized item has no parsed source item",
                        })?;
                Ok(ItemWire::new(
                    item,
                    parsed,
                    schema.model,
                    &components.edges,
                    &components.mentions,
                ))
            })
            .collect::<Result<Vec<_>, IndexError>>()?;
        items.sort_by(|left, right| left.mid.as_bytes().cmp(right.mid.as_bytes()));

        let mut documents = documents
            .iter()
            .map(|document| DocumentWire::new(document, &components.mentions))
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));

        Ok(Self {
            format: "mara.index",
            version: 1,
            project: IndexProjectWire {
                name: project.name.clone(),
                schema: IndexSchemaWire {
                    name: schema.model.schema().value().name().value().clone(),
                    version: schema.model.schema().value().version().value().clone(),
                    format_version: *schema.model.format_version().value(),
                    path: project.schema_source_path.clone(),
                    sha256: sha256_hex(schema.source),
                },
                content: IndexContentWire {
                    include: project.content.include.clone(),
                    exclude: project.content.exclude.clone(),
                },
            },
            git,
            documents,
            items,
            source_nodes: components
                .source_nodes
                .iter()
                .map(|node| node.wire.clone())
                .collect(),
            edges: components
                .edges
                .iter()
                .map(|edge| edge.wire.clone())
                .collect(),
            mentions: components
                .mentions
                .iter()
                .map(|mention| mention.wire.clone())
                .collect(),
            external_nodes: components.external_nodes,
            diagnostics: diagnostics.iter().map(DiagnosticWire::from).collect(),
        })
    }

    /// Serializes the projection using canonical pretty JSON and exactly one final LF.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, IndexError> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(IndexError::Serialization)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Successful configured-index replacement evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexWriteResult {
    path: String,
    sha256: String,
}

impl IndexWriteResult {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Builds, serializes, flushes, and atomically replaces the configured index.
pub fn write_index(result: &ValidationResult) -> Result<IndexWriteResult, IndexError> {
    write_index_with_checkpoint(result, &mut |_| Ok(()))
}

fn write_index_with_checkpoint(
    result: &ValidationResult,
    checkpoint: &mut dyn FnMut(IndexWriteCheckpoint) -> Result<(), IndexError>,
) -> Result<IndexWriteResult, IndexError> {
    checkpoint(IndexWriteCheckpoint::BeforeSerialization)?;
    let project = result.project().ok_or(IndexError::InvalidModel {
        reason: "project model is unavailable",
    })?;
    let projection = IndexProjection::from_validation(result)?;
    if project.git.require_clean_worktree_for_writes && projection.git.dirty == Some(true) {
        return Err(IndexError::DirtyWorktree);
    }
    let bytes = projection.to_canonical_json()?;
    checkpoint(IndexWriteCheckpoint::Serialized)?;

    atomic_replace(project, &bytes, checkpoint, &mut || {
        verify_current_git(result, &projection.git)
    })?;
    Ok(IndexWriteResult {
        path: normalized_relative_path(&project.root, &project.index_path)?,
        sha256: sha256_hex(&bytes),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexWriteCheckpoint {
    BeforeSerialization,
    Serialized,
    TemporaryWritten,
    TemporaryFlushed,
    BeforeReplace,
    Replaced,
    ParentFlushed,
}

fn atomic_replace(
    project: &LoadedProject,
    bytes: &[u8],
    checkpoint: &mut dyn FnMut(IndexWriteCheckpoint) -> Result<(), IndexError>,
    verify_before_replace: &mut dyn FnMut() -> Result<(), IndexError>,
) -> Result<(), IndexError> {
    let destination = &project.index_path;
    let parent = destination.parent().ok_or(IndexError::UnsafePath {
        reason: "configured index has no parent directory",
        path: destination.to_path_buf(),
    })?;
    prepare_output_path(project, destination, parent)?;

    let (temporary, mut file) = create_temporary(parent, destination)?;
    let mut replaced = false;
    let result = (|| {
        file.write_all(bytes).map_err(|source| IndexError::Io {
            operation: "write temporary index",
            path: destination.to_path_buf(),
            source,
        })?;
        checkpoint(IndexWriteCheckpoint::TemporaryWritten)?;
        file.sync_all().map_err(|source| IndexError::Io {
            operation: "flush temporary index",
            path: destination.to_path_buf(),
            source,
        })?;
        checkpoint(IndexWriteCheckpoint::TemporaryFlushed)?;
        drop(file);
        checkpoint(IndexWriteCheckpoint::BeforeReplace)?;
        verify_before_replace()?;
        fs::rename(&temporary, destination).map_err(|source| IndexError::Io {
            operation: "replace configured index",
            path: destination.to_path_buf(),
            source,
        })?;
        replaced = true;
        checkpoint(IndexWriteCheckpoint::Replaced)?;
        sync_directory(parent, destination)?;
        checkpoint(IndexWriteCheckpoint::ParentFlushed)
    })();

    if result.is_err() && !replaced {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn verify_current_git(result: &ValidationResult, expected: &GitWire) -> Result<(), IndexError> {
    let project = result.project().ok_or(IndexError::InvalidModel {
        reason: "project model is unavailable",
    })?;
    let current = GitWire::discover(project, result.documents(), result.content_paths())?;
    result
        .git_anchor()
        .ok_or(IndexError::InvalidModel {
            reason: "validation Git anchor is unavailable",
        })?
        .verify(current.anchor.as_ref())?;
    if current.wire == *expected {
        Ok(())
    } else {
        Err(IndexError::GitStateChanged)
    }
}

fn prepare_output_path(
    project: &LoadedProject,
    destination: &Path,
    parent: &Path,
) -> Result<(), IndexError> {
    let root = fs::canonicalize(&project.root).map_err(|source| IndexError::Io {
        operation: "resolve project root before index write",
        path: project.root.clone(),
        source,
    })?;
    let mut existing_ancestor = parent;
    loop {
        match fs::symlink_metadata(existing_ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                existing_ancestor = existing_ancestor.parent().ok_or(IndexError::UnsafePath {
                    reason: "configured index has no existing parent ancestor",
                    path: destination.to_path_buf(),
                })?;
            }
            Err(source) => {
                return Err(IndexError::Io {
                    operation: "inspect index parent before creation",
                    path: destination.to_path_buf(),
                    source,
                });
            }
        }
    }
    let resolved_ancestor =
        fs::canonicalize(existing_ancestor).map_err(|source| IndexError::Io {
            operation: "resolve index parent ancestor before creation",
            path: destination.to_path_buf(),
            source,
        })?;
    if !resolved_ancestor.starts_with(&root) || !resolved_ancestor.is_dir() {
        return Err(IndexError::UnsafePath {
            reason: "configured index parent ancestor is outside the project or not a directory",
            path: destination.to_path_buf(),
        });
    }
    fs::create_dir_all(parent).map_err(|source| IndexError::Io {
        operation: "create index parent directory",
        path: destination.to_path_buf(),
        source,
    })?;
    let resolved_parent = fs::canonicalize(parent).map_err(|source| IndexError::Io {
        operation: "resolve index parent before write",
        path: destination.to_path_buf(),
        source,
    })?;
    if !resolved_parent.starts_with(&root) {
        return Err(IndexError::UnsafePath {
            reason: "configured index parent escaped the project root",
            path: destination.to_path_buf(),
        });
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(IndexError::UnsafePath {
            reason: "configured index is not a regular file",
            path: destination.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(IndexError::Io {
            operation: "inspect configured index before write",
            path: destination.to_path_buf(),
            source,
        }),
    }
}

fn create_temporary(parent: &Path, destination: &Path) -> Result<(PathBuf, File), IndexError> {
    let stem = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(IndexError::UnsafePath {
            reason: "configured index file name is not UTF-8",
            path: destination.to_path_buf(),
        })?;
    for _ in 0..16 {
        let mut random = [0_u8; 12];
        getrandom::fill(&mut random).map_err(IndexError::Randomness)?;
        let suffix = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = parent.join(format!(".{stem}.mara-{suffix}.tmp"));
        match open_exclusive(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(IndexError::Io {
                    operation: "create temporary index",
                    path: destination.to_path_buf(),
                    source,
                });
            }
        }
    }
    Err(IndexError::UnsafePath {
        reason: "could not allocate a unique sibling index temporary",
        path: destination.to_path_buf(),
    })
}

#[cfg(unix)]
fn open_exclusive(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_exclusive(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn sync_directory(path: &Path, destination: &Path) -> Result<(), IndexError> {
    let directory = File::open(path).map_err(|source| IndexError::Io {
        operation: "open index parent for flush",
        path: destination.to_path_buf(),
        source,
    })?;
    directory.sync_all().map_err(|source| IndexError::Io {
        operation: "flush index parent directory",
        path: destination.to_path_buf(),
        source,
    })
}

/// Structured projection, Git, serialization, or write failure.
#[derive(Debug)]
pub enum IndexError {
    InvalidModel {
        reason: &'static str,
    },
    Serialization(serde_json::Error),
    Randomness(getrandom::Error),
    GitIo {
        operation: &'static str,
        source: io::Error,
    },
    GitCommand {
        operation: &'static str,
        status: Option<i32>,
    },
    DirtyWorktree,
    GitStateChanged,
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    UnsafePath {
        reason: &'static str,
        path: PathBuf,
    },
}

impl IndexError {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Io { path, .. } | Self::UnsafePath { path, .. } => Some(path),
            Self::InvalidModel { .. }
            | Self::Serialization(_)
            | Self::Randomness(_)
            | Self::GitIo { .. }
            | Self::GitCommand { .. }
            | Self::DirtyWorktree
            | Self::GitStateChanged => None,
        }
    }

    pub(crate) fn project_relative_path(&self, root: &Path) -> Option<String> {
        normalized_relative_path(root, self.path()?).ok()
    }

    pub const fn command_code(&self) -> &'static str {
        match self {
            Self::GitIo { .. }
            | Self::GitCommand { .. }
            | Self::DirtyWorktree
            | Self::GitStateChanged => "git.precondition",
            Self::Io { .. } | Self::UnsafePath { .. } => "io.failed",
            Self::InvalidModel { .. } | Self::Serialization(_) | Self::Randomness(_) => {
                "internal.failed"
            }
        }
    }

    pub const fn command_message(&self) -> &'static str {
        match self {
            Self::GitIo { .. } | Self::GitCommand { .. } => "Git provenance could not be collected",
            Self::DirtyWorktree => "relevant Git inputs must be clean before writing",
            Self::GitStateChanged => "Git state changed while the project was being validated",
            Self::Io { .. } | Self::UnsafePath { .. } => {
                "the configured index could not be written atomically"
            }
            Self::InvalidModel { .. } => "the validated index model is unavailable",
            Self::Serialization(_) | Self::Randomness(_) => {
                "the configured index could not be generated"
            }
        }
    }
}

impl fmt::Display for IndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModel { reason } | Self::UnsafePath { reason, .. } => {
                formatter.write_str(reason)
            }
            Self::Serialization(_) => formatter.write_str("index serialization failed"),
            Self::Randomness(_) => formatter.write_str("index temporary allocation failed"),
            Self::GitIo { operation, .. }
            | Self::GitCommand { operation, .. }
            | Self::Io { operation, .. } => formatter.write_str(operation),
            Self::DirtyWorktree => formatter.write_str("relevant Git inputs are dirty"),
            Self::GitStateChanged => {
                formatter.write_str("Git state changed while the project was being validated")
            }
        }
    }
}

impl Error for IndexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(source) => Some(source),
            Self::Randomness(source) => Some(source),
            Self::GitIo { source, .. } | Self::Io { source, .. } => Some(source),
            Self::InvalidModel { .. }
            | Self::GitCommand { .. }
            | Self::DirtyWorktree
            | Self::GitStateChanged
            | Self::UnsafePath { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct IndexProjectWire {
    name: String,
    schema: IndexSchemaWire,
    content: IndexContentWire,
}

#[derive(Debug, Clone, Serialize)]
struct IndexSchemaWire {
    name: String,
    version: String,
    format_version: u32,
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct IndexContentWire {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct GitWire {
    available: bool,
    commit: Option<String>,
    branch: Option<String>,
    project_path: Option<String>,
    dirty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitAnchor {
    repository_root: PathBuf,
    commit: String,
    branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidationGitAnchor {
    Unavailable,
    Available(GitAnchor),
    Failed {
        operation: &'static str,
        status: Option<i32>,
    },
}

impl ValidationGitAnchor {
    fn verify(&self, current: Option<&GitAnchor>) -> Result<(), IndexError> {
        match (self, current) {
            (Self::Unavailable, None) => Ok(()),
            (Self::Available(expected), Some(actual)) if expected == actual => Ok(()),
            (Self::Failed { operation, status }, _) => Err(IndexError::GitCommand {
                operation,
                status: *status,
            }),
            (Self::Unavailable | Self::Available(_), _) => Err(IndexError::GitStateChanged),
        }
    }
}

struct GitDiscovery {
    wire: GitWire,
    anchor: Option<GitAnchor>,
}

pub(crate) fn capture_validation_git_anchor(root: &Path) -> ValidationGitAnchor {
    match GitAnchor::discover_with_program(root, OsStr::new("git")) {
        Ok(Some(anchor)) => ValidationGitAnchor::Available(anchor),
        Ok(None) => ValidationGitAnchor::Unavailable,
        Err(IndexError::GitIo { operation, .. }) => ValidationGitAnchor::Failed {
            operation,
            status: None,
        },
        Err(IndexError::GitCommand { operation, status }) => {
            ValidationGitAnchor::Failed { operation, status }
        }
        Err(_) => ValidationGitAnchor::Failed {
            operation: "capture Git validation anchor",
            status: None,
        },
    }
}

impl GitAnchor {
    fn discover_with_program(root: &Path, program: &OsStr) -> Result<Option<Self>, IndexError> {
        let top_level = match git_output(
            program,
            root,
            [
                OsString::from("rev-parse"),
                OsString::from("--show-toplevel"),
            ],
            "discover Git worktree",
        ) {
            Ok(output) => output,
            Err(IndexError::GitIo { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if !top_level.status.success() {
            return Ok(None);
        }
        let repository_root = PathBuf::from(output_text(&top_level, "read Git worktree root")?);
        let repository_root =
            fs::canonicalize(repository_root).map_err(|source| IndexError::GitIo {
                operation: "resolve Git worktree root",
                source,
            })?;
        let commit_output = git_output(
            program,
            &repository_root,
            [
                OsString::from("rev-parse"),
                OsString::from("--verify"),
                OsString::from("HEAD^{commit}"),
            ],
            "resolve Git HEAD commit",
        )?;
        let commit = successful_text(commit_output, "resolve Git HEAD commit")?;
        let branch_output = git_output(
            program,
            &repository_root,
            [
                OsString::from("symbolic-ref"),
                OsString::from("--quiet"),
                OsString::from("--short"),
                OsString::from("HEAD"),
            ],
            "resolve Git branch",
        )?;
        let branch = if branch_output.status.success() {
            Some(output_text(&branch_output, "read Git branch")?)
        } else if branch_output.status.code() == Some(1) {
            None
        } else {
            return Err(IndexError::GitCommand {
                operation: "resolve Git branch",
                status: branch_output.status.code(),
            });
        };
        Ok(Some(Self {
            repository_root,
            commit,
            branch,
        }))
    }
}

impl GitWire {
    fn unavailable() -> Self {
        Self {
            available: false,
            commit: None,
            branch: None,
            project_path: None,
            dirty: None,
        }
    }

    fn discover(
        project: &LoadedProject,
        documents: &[ParsedDocument],
        content_paths: &[PathBuf],
    ) -> Result<GitDiscovery, IndexError> {
        Self::discover_with_program(project, documents, content_paths, OsStr::new("git"))
    }

    fn discover_with_program(
        project: &LoadedProject,
        documents: &[ParsedDocument],
        content_paths: &[PathBuf],
        program: &OsStr,
    ) -> Result<GitDiscovery, IndexError> {
        let Some(anchor) = GitAnchor::discover_with_program(&project.root, program)? else {
            return Ok(GitDiscovery {
                wire: Self::unavailable(),
                anchor: None,
            });
        };
        let repository_root = &anchor.repository_root;
        let project_path = normalized_relative_path(repository_root, &project.root)?;

        let mut relevant = BTreeSet::new();
        relevant.insert(join_project_path(&project_path, PROJECT_CONFIG_PATH));
        relevant.insert(normalized_relative_path(
            repository_root,
            &project.config_path,
        )?);
        relevant.insert(join_project_path(
            &project_path,
            &project.schema_source_path,
        ));
        relevant.insert(normalized_relative_path(
            repository_root,
            &project.schema_path,
        )?);
        relevant.extend(
            documents
                .iter()
                .map(|document| join_project_path(&project_path, document.source().path())),
        );
        for path in content_paths {
            relevant.insert(normalized_relative_path(repository_root, path)?);
        }
        let deleted_output = git_output(
            program,
            repository_root,
            [
                OsString::from("--literal-pathspecs"),
                OsString::from("diff"),
                OsString::from("--no-renames"),
                OsString::from("--name-only"),
                OsString::from("--diff-filter=D"),
                OsString::from("-z"),
                OsString::from("HEAD"),
                OsString::from("--"),
                OsString::from(&project_path),
            ],
            "enumerate content deleted from Git HEAD",
        )?;
        if !deleted_output.status.success() {
            return Err(IndexError::GitCommand {
                operation: "enumerate content deleted from Git HEAD",
                status: deleted_output.status.code(),
            });
        }
        let deleted_content = deleted_output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .filter_map(|path| std::str::from_utf8(path).ok())
            .filter_map(|path| project_relative_git_path(&project_path, path))
            .map(str::to_owned);
        relevant.extend(
            select_configured_content_paths(project, deleted_content)
                .into_iter()
                .map(|path| join_project_path(&project_path, &path)),
        );
        let mut arguments = vec![
            OsString::from("--literal-pathspecs"),
            OsString::from("status"),
            OsString::from("--porcelain=v1"),
            OsString::from("-z"),
            OsString::from("--untracked-files=all"),
            OsString::from("--ignored=matching"),
            OsString::from("--ignore-submodules=none"),
            OsString::from("--"),
        ];
        arguments.extend(relevant.iter().map(OsString::from));
        let status_output = git_output(
            program,
            repository_root,
            arguments,
            "inspect relevant Git status",
        )?;
        if !status_output.status.success() {
            return Err(IndexError::GitCommand {
                operation: "inspect relevant Git status",
                status: status_output.status.code(),
            });
        }

        let dirty = !status_output.stdout.is_empty()
            || flagged_relevant_input_differs(program, repository_root, &relevant)?;
        let wire = Self {
            available: true,
            commit: Some(anchor.commit.clone()),
            branch: anchor.branch.clone(),
            project_path: Some(project_path),
            dirty: Some(dirty),
        };
        Ok(GitDiscovery {
            wire,
            anchor: Some(anchor),
        })
    }
}

fn git_output(
    program: &OsStr,
    root: &Path,
    arguments: impl IntoIterator<Item = OsString>,
    operation: &'static str,
) -> Result<Output, IndexError> {
    git_command(program, root)
        .args(arguments)
        .output()
        .map_err(|source| IndexError::GitIo { operation, source })
}

fn git_output_with_input(
    program: &OsStr,
    root: &Path,
    arguments: impl IntoIterator<Item = OsString>,
    input: &[u8],
    operation: &'static str,
) -> Result<Output, IndexError> {
    let mut child = git_command(program, root)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| IndexError::GitIo { operation, source })?;
    child
        .stdin
        .take()
        .expect("piped Git input has a child handle")
        .write_all(input)
        .map_err(|source| IndexError::GitIo { operation, source })?;
    child
        .wait_with_output()
        .map_err(|source| IndexError::GitIo { operation, source })
}

fn git_command(program: &OsStr, root: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .args(["-c", "core.fsmonitor=false"])
        .arg("-C")
        .arg(root);
    command
}

fn project_relative_git_path<'a>(project_path: &str, repository_path: &'a str) -> Option<&'a str> {
    if project_path == "." {
        Some(repository_path)
    } else {
        repository_path
            .strip_prefix(project_path)
            .and_then(|path| path.strip_prefix('/'))
    }
}

fn flagged_relevant_input_differs(
    program: &OsStr,
    repository_root: &Path,
    relevant: &BTreeSet<String>,
) -> Result<bool, IndexError> {
    let mut arguments = vec![
        OsString::from("--literal-pathspecs"),
        OsString::from("ls-files"),
        OsString::from("-v"),
        OsString::from("-z"),
        OsString::from("--"),
    ];
    arguments.extend(relevant.iter().map(OsString::from));
    let output = git_output(
        program,
        repository_root,
        arguments,
        "inspect relevant Git index flags",
    )?;
    if !output.status.success() {
        return Err(IndexError::GitCommand {
            operation: "inspect relevant Git index flags",
            status: output.status.code(),
        });
    }
    for entry in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Some((&tag, path)) = entry.split_first() else {
            continue;
        };
        let path = path.strip_prefix(b" ").ok_or_else(|| IndexError::GitIo {
            operation: "decode relevant Git index flags",
            source: io::Error::new(
                io::ErrorKind::InvalidData,
                "Git returned a malformed flag entry",
            ),
        })?;
        if tag != b'S' && !tag.is_ascii_lowercase() {
            continue;
        }
        let path = std::str::from_utf8(path).map_err(|_| IndexError::GitIo {
            operation: "decode relevant Git index flags",
            source: io::Error::new(io::ErrorKind::InvalidData, "Git returned a non-UTF-8 path"),
        })?;
        if worktree_path_differs_from_head(program, repository_root, path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn worktree_path_differs_from_head(
    program: &OsStr,
    repository_root: &Path,
    path: &str,
) -> Result<bool, IndexError> {
    let revision = format!("HEAD:{path}");
    let head = git_output(
        program,
        repository_root,
        [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("--end-of-options"),
            OsString::from(revision),
        ],
        "resolve relevant Git HEAD blob",
    )?;
    if !head.status.success() {
        return Ok(true);
    }
    let head = output_text(&head, "read relevant Git HEAD blob")?;
    let absolute = repository_root.join(path);
    let metadata = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(source) => {
            return Err(IndexError::GitIo {
                operation: "inspect flagged relevant worktree path",
                source,
            });
        }
    };
    let current = if metadata.file_type().is_symlink() {
        let target = symlink_target_bytes(&absolute).map_err(|source| IndexError::GitIo {
            operation: "read flagged relevant symlink target",
            source,
        })?;
        let output = git_output_with_input(
            program,
            repository_root,
            [OsString::from("hash-object"), OsString::from("--stdin")],
            &target,
            "hash flagged relevant symlink target",
        )?;
        successful_text(output, "hash flagged relevant symlink target")?
    } else if metadata.is_file() {
        let output = git_output(
            program,
            repository_root,
            [
                OsString::from("hash-object"),
                OsString::from(format!("--path={path}")),
                OsString::from("--"),
                OsString::from(path),
            ],
            "hash flagged relevant worktree file",
        )?;
        successful_text(output, "hash flagged relevant worktree file")?
    } else {
        return Ok(true);
    };
    Ok(current != head)
}

#[cfg(unix)]
fn symlink_target_bytes(path: &Path) -> io::Result<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;

    fs::read_link(path).map(|target| target.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn symlink_target_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let target = fs::read_link(path)?;
    target
        .to_str()
        .map(|target| target.as_bytes().to_vec())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "symlink target is not UTF-8"))
}

fn successful_text(output: Output, operation: &'static str) -> Result<String, IndexError> {
    if !output.status.success() {
        return Err(IndexError::GitCommand {
            operation,
            status: output.status.code(),
        });
    }
    output_text(&output, operation)
}

fn output_text(output: &Output, operation: &'static str) -> Result<String, IndexError> {
    let text = std::str::from_utf8(&output.stdout).map_err(|_| IndexError::GitIo {
        operation,
        source: io::Error::new(
            io::ErrorKind::InvalidData,
            "Git returned non-UTF-8 provenance",
        ),
    })?;
    let text = text.trim_end_matches(['\r', '\n']);
    if text.is_empty() {
        return Err(IndexError::GitCommand {
            operation,
            status: output.status.code(),
        });
    }
    Ok(text.to_owned())
}

fn join_project_path(project_path: &str, path: &str) -> String {
    if project_path == "." {
        path.to_owned()
    } else {
        format!("{project_path}/{path}")
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct SourceSpanWire {
    path: String,
    start_byte: u64,
    end_byte: u64,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

impl From<&SourceSpan> for SourceSpanWire {
    fn from(span: &SourceSpan) -> Self {
        Self {
            path: span.path().to_owned(),
            start_byte: span.start_byte(),
            end_byte: span.end_byte(),
            start_line: span.start_line(),
            start_column: span.start_column(),
            end_line: span.end_line(),
            end_column: span.end_column(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RelatedDiagnosticWire {
    message: String,
    span: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticItemWire {
    mid: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticContextWire {
    field: Option<String>,
    relation: Option<String>,
    target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticWire {
    code: String,
    severity: String,
    message: String,
    primary: Option<SourceSpanWire>,
    related: Vec<RelatedDiagnosticWire>,
    item: Option<DiagnosticItemWire>,
    context: DiagnosticContextWire,
    details: BTreeMap<String, serde_json::Value>,
}

impl From<&Diagnostic> for DiagnosticWire {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            code: diagnostic.code().as_str().to_owned(),
            severity: diagnostic.severity().as_str().to_owned(),
            message: diagnostic.message().to_owned(),
            primary: diagnostic.primary().map(SourceSpanWire::from),
            related: diagnostic
                .related()
                .iter()
                .map(|related| RelatedDiagnosticWire {
                    message: related.message().to_owned(),
                    span: SourceSpanWire::from(related.span()),
                })
                .collect(),
            item: diagnostic.item().map(|item| DiagnosticItemWire {
                mid: item.mid().as_str().to_owned(),
                id: item.id().map(str::to_owned),
            }),
            context: DiagnosticContextWire {
                field: diagnostic.context().field().map(str::to_owned),
                relation: diagnostic.context().relation().map(str::to_owned),
                target: diagnostic.context().target().map(str::to_owned),
            },
            details: diagnostic
                .details()
                .iter()
                .map(|(key, value)| (key.clone(), diagnostic_value(value)))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DocumentWire {
    path: String,
    sha256: String,
    line_ending: String,
    span: SourceSpanWire,
    preamble: Vec<PlacementWire>,
    sections: Vec<SectionWire>,
    item_mids: Vec<String>,
}

impl DocumentWire {
    fn new(document: &ParsedDocument, mentions: &[MentionProjection]) -> Self {
        Self {
            path: document.source().path().to_owned(),
            sha256: sha256_hex(document.source().source().as_str().as_bytes()),
            line_ending: document.source().source().line_ending().as_str().to_owned(),
            span: SourceSpanWire::from(document.source().span()),
            preamble: placements(document.preamble(), mentions),
            sections: document
                .sections()
                .iter()
                .map(|section| SectionWire::new(document, section, mentions))
                .collect(),
            item_mids: document
                .items()
                .map(|item| item.mid().as_str().to_owned())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SectionWire {
    level: u8,
    title: String,
    span: SourceSpanWire,
    heading_span: SourceSpanWire,
    content: Vec<PlacementWire>,
    children: Vec<SectionWire>,
}

impl SectionWire {
    fn new(
        document: &ParsedDocument,
        section: &ParsedSection,
        mentions: &[MentionProjection],
    ) -> Self {
        Self {
            level: section.level(),
            title: section.title().to_owned(),
            span: SourceSpanWire::from(section.source()),
            heading_span: SourceSpanWire::from(section.heading_source()),
            content: placements(&document.blocks()[section.content_range()], mentions),
            children: section
                .children()
                .iter()
                .map(|child| Self::new(document, child, mentions))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
enum PlacementWire {
    #[serde(rename = "narrative")]
    Narrative { block: NarrativeBlockWire },
    #[serde(rename = "item")]
    Item { mid: String, span: SourceSpanWire },
}

fn placements(blocks: &[ParsedBlock], mentions: &[MentionProjection]) -> Vec<PlacementWire> {
    blocks
        .iter()
        .map(|block| match block {
            ParsedBlock::Markdown(markdown) => PlacementWire::Narrative {
                block: NarrativeBlockWire {
                    kind: narrative_kind(markdown.kind()).to_owned(),
                    markdown: markdown.raw().to_owned(),
                    span: SourceSpanWire::from(markdown.source()),
                    mentions: mentions
                        .iter()
                        .filter(|mention| span_contains(markdown.source(), &mention.source))
                        .map(|mention| mention.wire.clone())
                        .collect(),
                },
            },
            ParsedBlock::Item(item) => PlacementWire::Item {
                mid: item.mid().as_str().to_owned(),
                span: SourceSpanWire::from(item.source()),
            },
        })
        .collect()
}

fn narrative_kind(kind: NarrativeKind) -> &'static str {
    match kind {
        NarrativeKind::Paragraph => "paragraph",
        NarrativeKind::Heading => "heading",
        NarrativeKind::List => "list",
        NarrativeKind::Quote => "quote",
        NarrativeKind::Code => "code",
        NarrativeKind::Table => "table",
        NarrativeKind::ThematicBreak => "thematic_break",
        NarrativeKind::Html => "html",
        NarrativeKind::Other => "other",
    }
}

fn span_contains(container: &SourceSpan, candidate: &SourceSpan) -> bool {
    container.path() == candidate.path()
        && container.start_byte() <= candidate.start_byte()
        && candidate.end_byte() <= container.end_byte()
}

#[derive(Debug, Clone, Serialize)]
struct NarrativeBlockWire {
    kind: String,
    markdown: String,
    span: SourceSpanWire,
    mentions: Vec<MentionWire>,
}

#[derive(Debug, Clone, Serialize)]
struct MetadataWire {
    key: String,
    raw_value: String,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
struct FieldValueWire {
    value: serde_json::Value,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
struct ItemFieldWire {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    values: Vec<FieldValueWire>,
}

#[derive(Debug, Clone, Serialize)]
struct ItemWire {
    mid: String,
    id: Option<String>,
    flavour: String,
    title: Option<String>,
    body_markdown: String,
    document: String,
    source: SourceSpanWire,
    header_source: SourceSpanWire,
    body_source: SourceSpanWire,
    metadata: Vec<MetadataWire>,
    fields: Vec<ItemFieldWire>,
    outgoing: Vec<EdgeWire>,
    incoming: Vec<EdgeWire>,
    mentions: Vec<MentionWire>,
}

impl ItemWire {
    fn new(
        item: &NormalizedItem,
        parsed: &ParsedItem,
        schema: &SchemaDocument,
        edges: &[EdgeProjection],
        mentions: &[MentionProjection],
    ) -> Self {
        Self {
            mid: item.mid().as_str().to_owned(),
            id: item.display_id().map(|value| value.value().clone()),
            flavour: item.flavour().to_owned(),
            title: item.title().map(|value| value.value().clone()),
            body_markdown: parsed.body_markdown().to_owned(),
            document: item.source().path().to_owned(),
            source: SourceSpanWire::from(item.source()),
            header_source: SourceSpanWire::from(item.header_source()),
            body_source: SourceSpanWire::from(parsed.body_source()),
            metadata: parsed
                .metadata()
                .iter()
                .map(|entry| MetadataWire {
                    key: entry.key().to_owned(),
                    raw_value: entry.raw_value().to_owned(),
                    source: SourceSpanWire::from(entry.source()),
                })
                .collect(),
            fields: item
                .fields()
                .iter()
                .map(|(name, values)| {
                    let definition = schema
                        .flavours()
                        .get(item.flavour())
                        .and_then(|flavour| flavour.fields().get(name))
                        .expect("normalized fields retain a schema declaration");
                    ItemFieldWire {
                        name: name.clone(),
                        field_type: definition.field_type().value().as_str().to_owned(),
                        values: values
                            .iter()
                            .map(|value| FieldValueWire {
                                value: scalar_value(value.value()),
                                source: SourceSpanWire::from(value.source()),
                            })
                            .collect(),
                    }
                })
                .collect(),
            outgoing: edges
                .iter()
                .filter(|edge| edge.source.mid() == Some(item.mid()))
                .map(|edge| edge.wire.clone())
                .collect(),
            incoming: edges
                .iter()
                .filter(|edge| edge.target.mid() == Some(item.mid()))
                .map(|edge| edge.wire.clone())
                .collect(),
            mentions: mentions
                .iter()
                .filter(|mention| mention.source_item_mid.as_deref() == Some(item.mid().as_str()))
                .map(|mention| mention.wire.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
enum NodeRefWire {
    #[serde(rename = "item")]
    Item { mid: String },
    #[serde(rename = "source_span")]
    SourceSpan {
        source: SourceSpanWire,
        symbol: Option<String>,
    },
    #[serde(rename = "external")]
    External { uri: String },
}

impl From<&NodeRef> for NodeRefWire {
    fn from(node: &NodeRef) -> Self {
        match node {
            NodeRef::Item { mid } => Self::Item {
                mid: mid.as_str().to_owned(),
            },
            NodeRef::SourceSpan { source, symbol } => Self::SourceSpan {
                source: SourceSpanWire::from(source),
                symbol: symbol.clone(),
            },
            NodeRef::External { uri } => Self::External { uri: uri.clone() },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct EdgeOccurrenceWire {
    origin: String,
    authoring_name: String,
    source: SourceSpanWire,
}

#[derive(Debug, Clone, Serialize)]
struct EdgeWire {
    source: NodeRefWire,
    relation: String,
    inverse_name: Option<String>,
    target: NodeRefWire,
    occurrences: Vec<EdgeOccurrenceWire>,
}

#[derive(Debug, Clone)]
struct EdgeProjection {
    source: NodeRef,
    target: NodeRef,
    wire: EdgeWire,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    source: NodeRef,
    relation: String,
    target: NodeRef,
}

#[derive(Debug, Clone)]
struct EdgeAccumulator {
    source: NodeRef,
    relation: String,
    inverse_name: Option<String>,
    target: NodeRef,
    occurrences: Vec<EdgeOccurrenceWire>,
}

fn edge_projections(
    schema: &SchemaDocument,
    semantic: &SemanticCompilation,
    graph: &QueryGraph,
) -> Vec<EdgeProjection> {
    let mut edges = BTreeMap::<EdgeKey, EdgeAccumulator>::new();
    for edge in semantic.relations().edges() {
        let source = NodeRef::item(edge.source().clone());
        let target = NodeRef::item(edge.target().clone());
        let key = EdgeKey {
            source: source.clone(),
            relation: edge.relation().to_owned(),
            target: target.clone(),
        };
        let occurrences = edge
            .occurrences()
            .iter()
            .map(|occurrence| {
                let authored = occurrence.reference().authored();
                let origin =
                    relation_occurrence_wire_origin(occurrence.origin(), authored.syntax());
                EdgeOccurrenceWire {
                    origin: origin.to_owned(),
                    authoring_name: authored.relation().unwrap_or(edge.relation()).to_owned(),
                    source: SourceSpanWire::from(authored.source()),
                }
            })
            .collect();
        edges.insert(
            key,
            EdgeAccumulator {
                source,
                relation: edge.relation().to_owned(),
                inverse_name: inverse_name(schema, edge.relation()),
                target,
                occurrences,
            },
        );
    }

    for edge in semantic.projection_edges() {
        let key = EdgeKey {
            source: edge.source().clone(),
            relation: edge.relation().to_owned(),
            target: edge.target().clone(),
        };
        edges.entry(key).or_insert_with(|| EdgeAccumulator {
            source: edge.source().clone(),
            relation: edge.relation().to_owned(),
            inverse_name: inverse_name(schema, edge.relation()),
            target: edge.target().clone(),
            occurrences: projection_occurrences(semantic, edge),
        });
    }

    for edge in graph.edges() {
        let key = EdgeKey {
            source: edge.source().clone(),
            relation: edge.relation().to_owned(),
            target: edge.target().clone(),
        };
        edges.entry(key).or_insert_with(|| {
            let occurrences = match edge.source() {
                NodeRef::SourceSpan { source, .. } => vec![EdgeOccurrenceWire {
                    origin: "derived_source".to_owned(),
                    authoring_name: edge.relation().to_owned(),
                    source: SourceSpanWire::from(source),
                }],
                NodeRef::Item { .. } | NodeRef::External { .. } => Vec::new(),
            };
            EdgeAccumulator {
                source: edge.source().clone(),
                relation: edge.relation().to_owned(),
                inverse_name: inverse_name(schema, edge.relation()),
                target: edge.target().clone(),
                occurrences,
            }
        });
    }

    let mut projections = edges
        .into_values()
        .map(|mut edge| {
            edge.occurrences.sort_by(|left, right| {
                left.source
                    .path
                    .as_bytes()
                    .cmp(right.source.path.as_bytes())
                    .then_with(|| left.source.start_byte.cmp(&right.source.start_byte))
                    .then_with(|| left.source.end_byte.cmp(&right.source.end_byte))
                    .then_with(|| left.origin.as_bytes().cmp(right.origin.as_bytes()))
                    .then_with(|| {
                        left.authoring_name
                            .as_bytes()
                            .cmp(right.authoring_name.as_bytes())
                    })
            });
            edge.occurrences.dedup();
            EdgeProjection {
                source: edge.source.clone(),
                target: edge.target.clone(),
                wire: EdgeWire {
                    source: NodeRefWire::from(&edge.source),
                    relation: edge.relation,
                    inverse_name: edge.inverse_name,
                    target: NodeRefWire::from(&edge.target),
                    occurrences: edge.occurrences,
                },
            }
        })
        .collect::<Vec<_>>();
    projections.sort_by_cached_key(|edge| {
        (
            canonical_node_ref_bytes(&edge.wire.source),
            edge.wire.relation.as_bytes().to_vec(),
            canonical_node_ref_bytes(&edge.wire.target),
        )
    });
    projections
}

fn canonical_node_ref_bytes(node: &NodeRefWire) -> Vec<u8> {
    serde_json::to_vec(node).expect("NodeRef wire values always serialize to canonical JSON")
}

fn projection_occurrences(
    semantic: &SemanticCompilation,
    edge: &mara_core::ProjectionEdge,
) -> Vec<EdgeOccurrenceWire> {
    let Some(source_mid) = edge.source().mid() else {
        return Vec::new();
    };
    let Some(target_uri) = edge.target().uri() else {
        return Vec::new();
    };
    semantic
        .items()
        .iter()
        .find(|item| item.mid() == source_mid)
        .into_iter()
        .flat_map(NormalizedItem::authored_references)
        .filter(|reference| {
            reference.relation() == Some(edge.relation()) && reference.target() == target_uri
        })
        .map(|reference| EdgeOccurrenceWire {
            origin: match reference.syntax() {
                AuthoredReferenceSyntax::Inline => "typed_inline",
                AuthoredReferenceSyntax::Metadata => "canonical_metadata",
                AuthoredReferenceSyntax::Narrative => "derived_source",
            }
            .to_owned(),
            authoring_name: reference
                .relation()
                .expect("typed projection occurrence has an authoring name")
                .to_owned(),
            source: SourceSpanWire::from(reference.source()),
        })
        .collect()
}

fn inverse_name(schema: &SchemaDocument, relation: &str) -> Option<String> {
    relation_inverse_wire_name(schema, relation)
}

#[derive(Debug, Clone, Serialize)]
struct MentionWire {
    document: String,
    source_item_mid: Option<String>,
    target: NodeRefWire,
    label: Option<String>,
    source: SourceSpanWire,
}

#[derive(Debug, Clone)]
struct MentionProjection {
    source: SourceSpan,
    source_item_mid: Option<String>,
    target: NodeRef,
    wire: MentionWire,
}

fn mention_projections(semantic: &SemanticCompilation) -> Vec<MentionProjection> {
    let mut mentions =
        semantic
            .relations()
            .weak_mentions()
            .iter()
            .map(|mention| {
                let reference = mention.reference();
                mention_projection(
                    reference.authored(),
                    NodeRef::item(reference.target().clone()),
                )
            })
            .chain(semantic.external_mentions().iter().map(|reference| {
                mention_projection(reference, NodeRef::external(reference.target()))
            }))
            .collect::<Vec<_>>();
    mentions.sort_by(|left, right| {
        left.source
            .path()
            .as_bytes()
            .cmp(right.source.path().as_bytes())
            .then_with(|| left.source.start_byte().cmp(&right.source.start_byte()))
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.source.end_byte().cmp(&right.source.end_byte()))
    });
    mentions
}

fn mention_projection(reference: &AuthoredReference, target: NodeRef) -> MentionProjection {
    let source_item_mid = match reference.origin() {
        ReferenceOrigin::Item { mid, .. } => Some(mid.as_str().to_owned()),
        ReferenceOrigin::Narrative(_) => None,
    };
    let source = reference.source().clone();
    MentionProjection {
        source: source.clone(),
        source_item_mid: source_item_mid.clone(),
        target: target.clone(),
        wire: MentionWire {
            document: source.path().to_owned(),
            source_item_mid,
            target: NodeRefWire::from(&target),
            label: reference.label().map(str::to_owned),
            source: SourceSpanWire::from(&source),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
struct SourceNodeWire {
    source: SourceSpanWire,
    symbol: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceNodeProjection {
    wire: SourceNodeWire,
}

#[derive(Debug, Clone, Serialize)]
struct ExternalNodeWire {
    uri: String,
    scheme: String,
}

struct ProjectionComponents {
    edges: Vec<EdgeProjection>,
    mentions: Vec<MentionProjection>,
    source_nodes: Vec<SourceNodeProjection>,
    external_nodes: Vec<ExternalNodeWire>,
}

impl ProjectionComponents {
    fn build(schema: &SchemaDocument, semantic: &SemanticCompilation, graph: &QueryGraph) -> Self {
        let edges = edge_projections(schema, semantic, graph);
        let mentions = mention_projections(semantic);

        let mut source_nodes = BTreeMap::<NodeRef, SourceNodeWire>::new();
        let mut external_uris = BTreeSet::new();
        for node in graph
            .nodes()
            .iter()
            .chain(edges.iter().flat_map(|edge| [&edge.source, &edge.target]))
            .chain(mentions.iter().map(|mention| &mention.target))
        {
            match node {
                NodeRef::SourceSpan { source, symbol } => {
                    source_nodes
                        .entry(node.clone())
                        .or_insert_with(|| SourceNodeWire {
                            source: SourceSpanWire::from(source),
                            symbol: symbol.clone(),
                        });
                }
                NodeRef::External { uri } => {
                    external_uris.insert(uri.clone());
                }
                NodeRef::Item { .. } => {}
            }
        }

        Self {
            edges,
            mentions,
            source_nodes: source_nodes
                .into_values()
                .map(|wire| SourceNodeProjection { wire })
                .collect(),
            external_nodes: external_uris
                .into_iter()
                .map(|uri| ExternalNodeWire {
                    scheme: uri
                        .split_once(':')
                        .map_or("", |(scheme, _)| scheme)
                        .to_owned(),
                    uri,
                })
                .collect(),
        }
    }
}

fn scalar_value(value: &NormalizedScalar) -> serde_json::Value {
    match value {
        NormalizedScalar::String(value) | NormalizedScalar::Enum(value) => {
            serde_json::Value::String(value.clone())
        }
        NormalizedScalar::Integer(value) => serde_json::Value::Number((*value).into()),
        NormalizedScalar::Number(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(value.get()).expect("normalized numbers are finite"),
        ),
        NormalizedScalar::Boolean(value) => serde_json::Value::Bool(*value),
    }
}

fn diagnostic_value(value: &DiagnosticValue) -> serde_json::Value {
    match value {
        DiagnosticValue::Null => serde_json::Value::Null,
        DiagnosticValue::Boolean(value) => serde_json::Value::Bool(*value),
        DiagnosticValue::Integer(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Unsigned(value) => serde_json::Value::Number((*value).into()),
        DiagnosticValue::Number(value) => serde_json::Value::Number(
            serde_json::Number::from_f64(value.get()).expect("diagnostic numbers are finite"),
        ),
        DiagnosticValue::String(value) => serde_json::Value::String(value.clone()),
        DiagnosticValue::Array(values) => {
            serde_json::Value::Array(values.iter().map(diagnostic_value).collect())
        }
        DiagnosticValue::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), diagnostic_value(value)))
                .collect(),
        ),
    }
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, IndexError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| IndexError::UnsafePath {
            reason: "path is outside its expected root",
            path: path.to_path_buf(),
        })?;
    if relative.as_os_str().is_empty() {
        return Ok(".".to_owned());
    }
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                parts.push(value.to_str().ok_or(IndexError::UnsafePath {
                    reason: "project-relative path is not UTF-8",
                    path: path.to_path_buf(),
                })?)
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(IndexError::UnsafePath {
                    reason: "project-relative path is not normalized",
                    path: path.to_path_buf(),
                });
            }
        }
    }
    if parts.is_empty() {
        Ok(".".to_owned())
    } else {
        Ok(parts.join("/"))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use mara_core::{ProjectionEdge, QueryGraph, SourceSpan};

    use super::*;

    const PROJECT: &str = r#"format_version = 1
[project]
name = "index-unit"
schema = ".mara/schema.yaml"
[content]
include = ["docs/**/*.mara.md"]
exclude = []
respect_gitignore = true
follow_directory_symlinks = false
allow_internal_file_symlinks = true
[index]
path = ".mara/index.json"
[validation]
warnings_as_errors = false
[git]
require_clean_worktree_for_writes = true
"#;

    const SCHEMA: &str = r#"format_version: 1
schema:
  name: index-unit
  version: 1.0.0
identity:
  mid:
    format: ulid
    prefix: m_
flavours:
  alpha:
    label: Alpha
    description: Index unit item.
    guidance:
      use_when: [Testing index internals.]
      avoid_when: [Testing another capability.]
    id: {}
    title: {}
    body: {}
    fields: {}
relations: {}
rules: []
"#;

    const CONTENT: &str = r#":::alpha m_00000000000000000000000001
:id: ALPHA-A
:title: Alpha

Body.
:::
"#;

    fn fixture() -> tempfile::TempDir {
        let fixture = tempfile::tempdir().unwrap();
        fs::create_dir_all(fixture.path().join(".mara")).unwrap();
        fs::create_dir_all(fixture.path().join("docs")).unwrap();
        fs::write(fixture.path().join(".mara/project.toml"), PROJECT).unwrap();
        fs::write(fixture.path().join(".mara/schema.yaml"), SCHEMA).unwrap();
        fs::write(fixture.path().join("docs/item.mara.md"), CONTENT).unwrap();
        fixture
    }

    fn injected(
        selected: IndexWriteCheckpoint,
    ) -> impl FnMut(IndexWriteCheckpoint) -> Result<(), IndexError> {
        move |point| {
            if point == selected {
                Err(IndexError::InvalidModel {
                    reason: "injected index failure",
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn fault_injection_never_exposes_partial_index_bytes() {
        let fixture = fixture();
        let result = crate::check_project(fixture.path()).unwrap();
        let destination = fixture.path().join(".mara/index.json");
        let previous = b"previous complete index\n";

        for point in [
            IndexWriteCheckpoint::BeforeSerialization,
            IndexWriteCheckpoint::Serialized,
            IndexWriteCheckpoint::TemporaryWritten,
            IndexWriteCheckpoint::TemporaryFlushed,
            IndexWriteCheckpoint::BeforeReplace,
        ] {
            fs::write(&destination, previous).unwrap();
            let error = write_index_with_checkpoint(&result, &mut injected(point)).unwrap_err();
            assert_eq!(error.to_string(), "injected index failure");
            assert_eq!(
                fs::read(&destination).unwrap(),
                previous,
                "fault: {point:?}"
            );
            assert!(
                fs::read_dir(fixture.path().join(".mara"))
                    .unwrap()
                    .all(|entry| !entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .contains(".mara-")),
                "fault left a handled temporary: {point:?}"
            );
        }

        for point in [
            IndexWriteCheckpoint::Replaced,
            IndexWriteCheckpoint::ParentFlushed,
        ] {
            fs::write(&destination, previous).unwrap();
            write_index_with_checkpoint(&result, &mut injected(point)).unwrap_err();
            let replacement = fs::read(&destination).unwrap();
            assert_ne!(replacement, previous, "fault: {point:?}");
            assert!(replacement.ends_with(b"\n"));
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&replacement).unwrap()["format"],
                "mara.index"
            );
        }

        fs::write(&destination, previous).unwrap();
        write_index_with_checkpoint(&result, &mut |_| Ok(())).unwrap();
        let replacement = fs::read(&destination).unwrap();
        assert_ne!(replacement, previous);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&replacement).unwrap()["format"],
            "mara.index"
        );
    }

    #[test]
    fn git_change_before_replace_preserves_the_previous_index() {
        let fixture = fixture();
        let git = |arguments: &[&str]| {
            let output = Command::new("git")
                .arg("-c")
                .arg("commit.gpgsign=false")
                .arg("-C")
                .arg(fixture.path())
                .args(arguments)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {arguments:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-b", "main"]);
        git(&["config", "user.name", "Mara Test"]);
        git(&["config", "user.email", "mara@example.test"]);
        git(&[
            "add",
            ".mara/project.toml",
            ".mara/schema.yaml",
            "docs/item.mara.md",
        ]);
        git(&["commit", "-m", "fixture"]);

        let result = crate::check_project(fixture.path()).unwrap();
        let destination = fixture.path().join(".mara/index.json");
        let previous = b"previous complete index\n";
        fs::write(&destination, previous).unwrap();
        let mut changed = false;
        let error = write_index_with_checkpoint(&result, &mut |point| {
            if point == IndexWriteCheckpoint::BeforeReplace && !changed {
                fs::write(
                    fixture.path().join("docs/item.mara.md"),
                    format!("{CONTENT}\n"),
                )
                .unwrap();
                git(&["add", "docs/item.mara.md"]);
                git(&["commit", "-m", "change HEAD before replacement"]);
                changed = true;
            }
            Ok(())
        })
        .unwrap_err();

        assert!(matches!(error, IndexError::GitStateChanged));
        assert_eq!(fs::read(&destination).unwrap(), previous);
        assert!(
            fs::read_dir(fixture.path().join(".mara"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".mara-"))
        );
    }

    #[test]
    fn missing_git_executable_is_explicitly_unavailable() {
        let fixture = fixture();
        let result = crate::check_project(fixture.path()).unwrap();
        let missing = fixture.path().join("missing-git-executable");

        let git = GitWire::discover_with_program(
            result.project().unwrap(),
            result.documents(),
            result.content_paths(),
            missing.as_os_str(),
        )
        .unwrap();

        assert!(git.anchor.is_none());
        assert!(!git.wire.available);
        assert!(git.wire.commit.is_none());
        assert!(git.wire.branch.is_none());
        assert!(git.wire.project_path.is_none());
        assert!(git.wire.dirty.is_none());
    }

    #[test]
    fn git_commands_clear_repository_overrides_and_disable_fsmonitor() {
        let fixture = fixture();
        let command = git_command(OsStr::new("git"), fixture.path());
        let environment = command.get_envs().collect::<BTreeMap<_, _>>();

        for name in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
        ] {
            assert_eq!(environment.get(OsStr::new(name)), Some(&None));
        }
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(arguments[0..2], ["-c", "core.fsmonitor=false"]);
        assert_eq!(arguments[2], "-C");
    }

    #[test]
    fn index_io_errors_retain_the_logical_destination_path() {
        let fixture = fixture();
        let destination = fixture.path().join(".mara/index.json");
        let missing_parent = fixture.path().join("missing-parent");

        let error = sync_directory(&missing_parent, &destination).unwrap_err();

        assert_eq!(error.path(), Some(destination.as_path()));
        assert_eq!(
            error.project_relative_path(fixture.path()).as_deref(),
            Some(".mara/index.json")
        );

        let error = IndexError::UnsafePath {
            reason: "fixture unsafe path",
            path: destination.clone(),
        };
        assert_eq!(error.path(), Some(destination.as_path()));
        assert_eq!(
            error.project_relative_path(fixture.path()).as_deref(),
            Some(".mara/index.json")
        );
    }

    #[test]
    fn canonical_edges_sort_by_serialized_node_ref_bytes() {
        let fixture = fixture();
        let result = crate::check_project(fixture.path()).unwrap();
        let semantic = result.semantic().unwrap();
        let target = semantic.items()[0].mid().clone();
        let source_text = "01234567890";
        let at_two = SourceSpan::try_new("src/lib.rs", source_text, 2, 3, 1, 3, 1, 4).unwrap();
        let at_ten = SourceSpan::try_new("src/lib.rs", source_text, 10, 11, 1, 11, 1, 12).unwrap();
        let edge_at_two = ProjectionEdge::new(
            "implements",
            NodeRef::source_span(at_two, None),
            NodeRef::item(target.clone()),
        )
        .unwrap();
        let edge_at_ten = ProjectionEdge::new(
            "implements",
            NodeRef::source_span(at_ten, None),
            NodeRef::item(target),
        )
        .unwrap();
        let graph = QueryGraph::build(
            result.graph().unwrap().nodes().iter().cloned().chain([
                edge_at_two.source().clone(),
                edge_at_ten.source().clone(),
                edge_at_two.target().clone(),
            ]),
            [edge_at_two, edge_at_ten],
        );
        let projection = IndexProjection::from_parts(
            result.project().unwrap(),
            ValidatedSchemaSnapshot {
                model: result.schema().unwrap(),
                source: result.schema_source().unwrap(),
            },
            result.documents(),
            semantic,
            &graph,
            result.diagnostics(),
            GitWire::unavailable(),
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&projection.to_canonical_json().unwrap()).unwrap();

        assert_eq!(value["edges"][0]["source"]["source"]["start_byte"], 10);
        assert_eq!(value["edges"][1]["source"]["source"]["start_byte"], 2);
    }

    #[test]
    fn derived_source_nodes_are_deduplicated_and_embedded_as_incoming_backlinks() {
        let fixture = fixture();
        let result = crate::check_project(fixture.path()).unwrap();
        let semantic = result.semantic().unwrap();
        let source = SourceSpan::try_new("src/lib.rs", "symbol", 0, 6, 1, 1, 1, 7).unwrap();
        let target = semantic.items()[0].mid().clone();
        let derived = ProjectionEdge::new(
            "implements",
            NodeRef::source_span(source.clone(), Some("symbol".to_owned())),
            NodeRef::item(target.clone()),
        )
        .unwrap();
        let graph = QueryGraph::build(
            result
                .graph()
                .unwrap()
                .nodes()
                .iter()
                .cloned()
                .chain([derived.source().clone(), derived.target().clone()]),
            result
                .graph()
                .unwrap()
                .edges()
                .iter()
                .cloned()
                .chain([derived.clone(), derived]),
        );
        let projection = IndexProjection::from_parts(
            result.project().unwrap(),
            ValidatedSchemaSnapshot {
                model: result.schema().unwrap(),
                source: result.schema_source().unwrap(),
            },
            result.documents(),
            semantic,
            &graph,
            result.diagnostics(),
            GitWire::unavailable(),
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&projection.to_canonical_json().unwrap()).unwrap();

        assert_eq!(value["source_nodes"].as_array().unwrap().len(), 1);
        assert_eq!(value["source_nodes"][0]["source"]["path"], "src/lib.rs");
        assert_eq!(value["source_nodes"][0]["symbol"], "symbol");
        assert_eq!(value["edges"].as_array().unwrap().len(), 1);
        assert_eq!(value["edges"][0]["source"]["kind"], "source_span");
        assert_eq!(value["edges"][0]["target"]["kind"], "item");
        assert_eq!(
            value["edges"][0]["occurrences"][0]["origin"],
            "derived_source"
        );
        let item = value["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["mid"] == target.as_str())
            .unwrap();
        assert_eq!(item["incoming"].as_array().unwrap().len(), 1);
        assert_eq!(item["incoming"][0], value["edges"][0]);
    }
}
