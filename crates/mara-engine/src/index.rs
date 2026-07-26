//! Deterministic versioned JSON index projection and atomic writer.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Output},
};

use mara_core::{
    AuthoredReference, AuthoredReferenceSyntax, AuthoredRelationOrigin, Diagnostic,
    DiagnosticValue, NodeRef, NormalizedItem, NormalizedScalar, QueryGraph, ReferenceOrigin,
    SchemaDocument, SourceSpan,
};
use mara_markdown::{NarrativeKind, ParsedBlock, ParsedDocument, ParsedItem, ParsedSection};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{SemanticCompilation, ValidationResult, project::LoadedProject};

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

        let git = GitWire::discover(project, result.documents())?;
        Self::from_parts(
            project,
            schema,
            result.documents(),
            semantic,
            graph,
            result.diagnostics(),
            git,
        )
    }

    fn from_parts(
        project: &LoadedProject,
        schema: &SchemaDocument,
        documents: &[ParsedDocument],
        semantic: &SemanticCompilation,
        graph: &QueryGraph,
        diagnostics: &[Diagnostic],
        git: GitWire,
    ) -> Result<Self, IndexError> {
        let schema_bytes = fs::read(&project.schema_path).map_err(|source| IndexError::Io {
            operation: "read schema for index digest",
            source,
        })?;
        let components = ProjectionComponents::build(schema, semantic, graph);

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
                    schema,
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
                    name: schema.schema().value().name().value().clone(),
                    version: schema.schema().value().version().value().clone(),
                    format_version: *schema.format_version().value(),
                    path: project.schema_source_path.clone(),
                    sha256: sha256_hex(&schema_bytes),
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
    let projection = IndexProjection::from_validation(result)?;
    let bytes = projection.to_canonical_json()?;
    checkpoint(IndexWriteCheckpoint::Serialized)?;

    let project = result.project().ok_or(IndexError::InvalidModel {
        reason: "project model is unavailable",
    })?;
    atomic_replace(project, &bytes, checkpoint)?;
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
) -> Result<(), IndexError> {
    let destination = &project.index_path;
    let parent = destination.parent().ok_or(IndexError::UnsafePath {
        reason: "configured index has no parent directory",
    })?;
    prepare_output_path(project, destination, parent)?;

    let (temporary, mut file) = create_temporary(parent, destination)?;
    let mut replaced = false;
    let result = (|| {
        file.write_all(bytes).map_err(|source| IndexError::Io {
            operation: "write temporary index",
            source,
        })?;
        checkpoint(IndexWriteCheckpoint::TemporaryWritten)?;
        file.sync_all().map_err(|source| IndexError::Io {
            operation: "flush temporary index",
            source,
        })?;
        checkpoint(IndexWriteCheckpoint::TemporaryFlushed)?;
        drop(file);
        checkpoint(IndexWriteCheckpoint::BeforeReplace)?;
        fs::rename(&temporary, destination).map_err(|source| IndexError::Io {
            operation: "replace configured index",
            source,
        })?;
        replaced = true;
        checkpoint(IndexWriteCheckpoint::Replaced)?;
        sync_directory(parent)?;
        checkpoint(IndexWriteCheckpoint::ParentFlushed)
    })();

    if result.is_err() && !replaced {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn prepare_output_path(
    project: &LoadedProject,
    destination: &Path,
    parent: &Path,
) -> Result<(), IndexError> {
    let root = fs::canonicalize(&project.root).map_err(|source| IndexError::Io {
        operation: "resolve project root before index write",
        source,
    })?;
    let mut existing_ancestor = parent;
    loop {
        match fs::symlink_metadata(existing_ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                existing_ancestor = existing_ancestor.parent().ok_or(IndexError::UnsafePath {
                    reason: "configured index has no existing parent ancestor",
                })?;
            }
            Err(source) => {
                return Err(IndexError::Io {
                    operation: "inspect index parent before creation",
                    source,
                });
            }
        }
    }
    let resolved_ancestor =
        fs::canonicalize(existing_ancestor).map_err(|source| IndexError::Io {
            operation: "resolve index parent ancestor before creation",
            source,
        })?;
    if !resolved_ancestor.starts_with(&root) || !resolved_ancestor.is_dir() {
        return Err(IndexError::UnsafePath {
            reason: "configured index parent ancestor is outside the project or not a directory",
        });
    }
    fs::create_dir_all(parent).map_err(|source| IndexError::Io {
        operation: "create index parent directory",
        source,
    })?;
    let resolved_parent = fs::canonicalize(parent).map_err(|source| IndexError::Io {
        operation: "resolve index parent before write",
        source,
    })?;
    if !resolved_parent.starts_with(&root) {
        return Err(IndexError::UnsafePath {
            reason: "configured index parent escaped the project root",
        });
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(IndexError::UnsafePath {
            reason: "configured index is not a regular file",
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(IndexError::Io {
            operation: "inspect configured index before write",
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
                    source,
                });
            }
        }
    }
    Err(IndexError::UnsafePath {
        reason: "could not allocate a unique sibling index temporary",
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

fn sync_directory(path: &Path) -> Result<(), IndexError> {
    let directory = File::open(path).map_err(|source| IndexError::Io {
        operation: "open index parent for flush",
        source,
    })?;
    directory.sync_all().map_err(|source| IndexError::Io {
        operation: "flush index parent directory",
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
    Io {
        operation: &'static str,
        source: io::Error,
    },
    UnsafePath {
        reason: &'static str,
    },
}

impl IndexError {
    pub const fn command_code(&self) -> &'static str {
        match self {
            Self::GitIo { .. } | Self::GitCommand { .. } => "git.precondition",
            Self::Io { .. } | Self::UnsafePath { .. } => "io.failed",
            Self::InvalidModel { .. } | Self::Serialization(_) | Self::Randomness(_) => {
                "internal.failed"
            }
        }
    }

    pub const fn command_message(&self) -> &'static str {
        match self {
            Self::GitIo { .. } | Self::GitCommand { .. } => "Git provenance could not be collected",
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
            Self::InvalidModel { reason } | Self::UnsafePath { reason } => {
                formatter.write_str(reason)
            }
            Self::Serialization(_) => formatter.write_str("index serialization failed"),
            Self::Randomness(_) => formatter.write_str("index temporary allocation failed"),
            Self::GitIo { operation, .. }
            | Self::GitCommand { operation, .. }
            | Self::Io { operation, .. } => formatter.write_str(operation),
        }
    }
}

impl Error for IndexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(source) => Some(source),
            Self::Randomness(source) => Some(source),
            Self::GitIo { source, .. } | Self::Io { source, .. } => Some(source),
            Self::InvalidModel { .. } | Self::GitCommand { .. } | Self::UnsafePath { .. } => None,
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

#[derive(Debug, Clone, Serialize)]
struct GitWire {
    available: bool,
    commit: Option<String>,
    branch: Option<String>,
    project_path: Option<String>,
    dirty: Option<bool>,
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

    fn discover(project: &LoadedProject, documents: &[ParsedDocument]) -> Result<Self, IndexError> {
        let top_level = git_output(
            &project.root,
            [
                OsString::from("rev-parse"),
                OsString::from("--show-toplevel"),
            ],
            "discover Git worktree",
        )?;
        if !top_level.status.success() {
            return Ok(Self::unavailable());
        }
        let repository_root = PathBuf::from(output_text(&top_level, "read Git worktree root")?);
        let repository_root =
            fs::canonicalize(repository_root).map_err(|source| IndexError::GitIo {
                operation: "resolve Git worktree root",
                source,
            })?;
        let project_path = normalized_relative_path(&repository_root, &project.root)?;

        let commit_output = git_output(
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

        let mut relevant = BTreeSet::new();
        relevant.insert(join_project_path(&project_path, PROJECT_CONFIG_PATH));
        relevant.insert(join_project_path(
            &project_path,
            &project.schema_source_path,
        ));
        relevant.extend(
            documents
                .iter()
                .map(|document| join_project_path(&project_path, document.source().path())),
        );
        let mut arguments = vec![
            OsString::from("--literal-pathspecs"),
            OsString::from("status"),
            OsString::from("--porcelain=v1"),
            OsString::from("-z"),
            OsString::from("--untracked-files=all"),
            OsString::from("--ignore-submodules=none"),
            OsString::from("--"),
        ];
        arguments.extend(relevant.into_iter().map(OsString::from));
        let status_output = git_output(&repository_root, arguments, "inspect relevant Git status")?;
        if !status_output.status.success() {
            return Err(IndexError::GitCommand {
                operation: "inspect relevant Git status",
                status: status_output.status.code(),
            });
        }

        Ok(Self {
            available: true,
            commit: Some(commit),
            branch,
            project_path: Some(project_path),
            dirty: Some(!status_output.stdout.is_empty()),
        })
    }
}

fn git_output(
    root: &Path,
    arguments: impl IntoIterator<Item = OsString>,
    operation: &'static str,
) -> Result<Output, IndexError> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|source| IndexError::GitIo { operation, source })
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
    let text = std::str::from_utf8(&output.stdout).map_err(|_| IndexError::UnsafePath {
        reason: "Git returned non-UTF-8 provenance",
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
                let origin = match authored.syntax() {
                    AuthoredReferenceSyntax::Inline => "typed_inline",
                    AuthoredReferenceSyntax::Narrative => "derived_source",
                    AuthoredReferenceSyntax::Metadata
                        if occurrence.origin() == AuthoredRelationOrigin::InverseNormalized =>
                    {
                        "inverse_metadata"
                    }
                    AuthoredReferenceSyntax::Metadata => "canonical_metadata",
                };
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

    edges
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
        .collect()
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
    schema
        .relations()
        .and_then(|relations| relations.get(relation))
        .and_then(|definition| definition.inverse())
        .map(|inverse| inverse.value().clone())
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
                })?)
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(IndexError::UnsafePath {
                    reason: "project-relative path is not normalized",
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
            result.schema().unwrap(),
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
