use crate::DiagnosticCode;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{Error, PROJECT_FILE, Project, Schema};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, Error as WalkError, Walk, WalkBuilder};

mod markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Disposable document-only source snapshot; code discovery is a separate dependency.
pub struct DocumentSet {
    documents: Vec<Document>,
    complete: bool,
}

impl DocumentSet {
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents.iter().flat_map(Document::items)
    }
    /// Whether all selected document sources were discovered and parsed.
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    path: PathBuf,
    source: String,
    items: Vec<Item>,
    blocks: Vec<MarkdownBlock>,
    references: Vec<DocumentReference>,
}

impl Document {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Explicit source references, including unresolved links and anchor declarations.
    pub fn references(&self) -> &[DocumentReference] {
        &self.references
    }

    /// Ordinary document blocks outside items, retaining Markdown containers.
    pub fn blocks(&self) -> &[MarkdownBlock] {
        &self.blocks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Item,
    MarkdownLink,
    Anchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentReference {
    kind: ReferenceKind,
    target: String,
    source: SourceLocation,
}

impl DocumentReference {
    pub fn kind(&self) -> ReferenceKind {
        self.kind
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    flavour: String,
    id: String,
    mid: Option<String>,
    title: String,
    metadata: Vec<MetadataEntry>,
    body: String,
    body_blocks: Vec<MarkdownBlock>,
    relations: Vec<Relation>,
    inline_diagnostics: Vec<Diagnostic>,
    mentions: Vec<Mention>,
    source: SourceLocation,
    body_source: SourceLocation,
    metadata_valid: bool,
    title_valid: bool,
    body_valid: bool,
}

impl Item {
    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn mid(&self) -> Option<&str> {
        self.mid.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn metadata(&self) -> &[MetadataEntry] {
        &self.metadata
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    /// Ordinary Markdown blocks within this item's body, in source order.
    pub fn body_blocks(&self) -> &[MarkdownBlock] {
        &self.body_blocks
    }

    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    pub fn mentions(&self) -> &[Mention] {
        &self.mentions
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }

    pub fn body_source(&self) -> &SourceLocation {
        &self.body_source
    }
}

/// A Markdown block retained in a document or item. Its source is canonical; this
/// projection is never used to render or rewrite authored Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownBlock {
    kind: MarkdownBlockKind,
    heading_text: Option<String>,
    heading_source_offsets: Vec<usize>,
    source: SourceLocation,
    children: Vec<MarkdownBlock>,
}

impl MarkdownBlock {
    pub(crate) fn heading_source_offset(&self, byte: usize) -> Option<usize> {
        self.heading_source_offsets.get(byte).copied()
    }
    pub fn kind(&self) -> MarkdownBlockKind {
        self.kind
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }

    /// Parsed heading text without Markdown formatting; absent on other blocks.
    pub fn heading_text(&self) -> Option<&str> {
        self.heading_text.as_deref()
    }

    pub fn children(&self) -> &[MarkdownBlock] {
        &self.children
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownBlockKind {
    Paragraph,
    Heading { level: u8 },
    ThematicBreak,
    CodeBlock,
    Blockquote,
    List,
    ListItem,
    HtmlBlock,
    LinkReferenceDefinition,
    Table,
    TableHeader,
    TableBody,
    TableRow,
    TableCell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataEntry {
    key: String,
    value: String,
    source: SourceLocation,
}

impl MetadataEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    name: String,
    pub(crate) canonical: String,
    pub(crate) inverse: bool,
    pub(crate) symmetric: bool,
    pub(crate) inline: bool,
    target: String,
    source: SourceLocation,
}

impl Relation {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    target: String,
    source: SourceLocation,
}

impl Mention {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    path: PathBuf,
    span: SourceSpan,
}

impl SourceLocation {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    start_byte: usize,
    end_byte: usize,
    start_line: usize,
    end_line: usize,
}

impl SourceSpan {
    pub fn start_byte(self) -> usize {
        self.start_byte
    }

    pub fn end_byte(self) -> usize {
        self.end_byte
    }

    pub fn start_line(self) -> usize {
        self.start_line
    }

    pub fn end_line(self) -> usize {
        self.end_line
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    source: SourceLocation,
    item_ids: Vec<String>,
    applies_to_all_items: bool,
    code: DiagnosticCode,
    coordinates_available: bool,
    message: String,
}

impl Diagnostic {
    pub fn code(&self) -> DiagnosticCode {
        self.code
    }
    pub fn coordinates_available(&self) -> bool {
        self.coordinates_available
    }
    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
    pub fn applies_to_item(&self, item_id: &str) -> bool {
        self.applies_to_all_items || self.item_ids.iter().any(|existing| existing == item_id)
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

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
    let mut documents = Vec::with_capacity(paths.len());
    for relative_path in paths {
        let absolute_path = project.root().join(&relative_path);
        let source = match fs::read_to_string(&absolute_path) {
            Ok(source) => source,
            Err(error) => {
                complete = false;
                diagnostics.push(Diagnostic {
                    source: SourceLocation {
                        path: relative_path,
                        span: SourceSpan {
                            start_byte: 0,
                            end_byte: 0,
                            start_line: 1,
                            end_line: 1,
                        },
                    },
                    item_ids: Vec::new(),
                    applies_to_all_items: true,
                    code: DiagnosticCode::SourceInvalid,
                    coordinates_available: false,
                    message: format!("could not read Mara document: {error}"),
                });
                continue;
            }
        };
        let (document, errors, document_complete) =
            parse_document_for_validation(relative_path.clone(), source, schema);
        complete &= document_complete;
        let retain_document = errors.is_empty() || !document.items.is_empty();
        diagnostics.extend(errors.into_iter().map(|error| Diagnostic {
            source: SourceLocation {
                path: relative_path.clone(),
                span: SourceSpan {
                    start_byte: error.source.start,
                    end_byte: error.source.end,
                    start_line: error.line,
                    end_line: error.line,
                },
            },
            item_ids: error.item_ids,
            applies_to_all_items: false,
            code: error.code,
            coordinates_available: true,
            message: error.message,
        }));
        if retain_document {
            documents.push(document);
        }
    }
    Ok((
        DocumentSet {
            documents,
            complete,
        },
        diagnostics,
    ))
}

pub(crate) fn diagnostic(
    code: DiagnosticCode,
    diagnostics: &mut Vec<Diagnostic>,
    source: &SourceLocation,
    message: String,
) {
    diagnostics.push(Diagnostic {
        source: source.clone(),
        item_ids: Vec::new(),
        applies_to_all_items: false,
        code,
        coordinates_available: true,
        message,
    });
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
        documents.push(parse_document(relative_path, source, schema)?);
    }
    Ok(DocumentSet {
        documents,
        complete: true,
    })
}

fn content_matcher(project: &Project) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in project.content_patterns() {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| Error::InvalidProject {
                path: project.root().join(PROJECT_FILE),
                message: format!("invalid content.include pattern '{pattern}': {error}"),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| Error::InvalidProject {
        path: project.root().join(PROJECT_FILE),
        message: format!("could not compile content.include patterns: {error}"),
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
            Err(error) => diagnostics.push(Diagnostic {
                source: SourceLocation {
                    path: walk_error_path(root, &error),
                    span: SourceSpan {
                        start_byte: 0,
                        end_byte: 0,
                        start_line: 1,
                        end_line: 1,
                    },
                },
                item_ids: Vec::new(),
                applies_to_all_items: true,
                code: DiagnosticCode::SourceInvalid,
                coordinates_available: false,
                message: format!("could not discover Mara documents: {error}"),
            }),
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

fn is_mara_document(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".mara.md"))
}

fn parse_document(path: PathBuf, source: String, schema: &Schema) -> Result<Document, Error> {
    let parsed =
        markdown::parse(&source).map_err(|error| invalid(&path, error.line, error.message))?;
    Ok(project_document(path, source, Some(schema), parsed))
}

fn parse_document_for_validation(
    path: PathBuf,
    source: String,
    schema: Option<&Schema>,
) -> (Document, Vec<markdown::ParseError>, bool) {
    let (parsed, errors) = markdown::parse_for_validation(&source);
    let complete = parsed.complete;
    (
        project_document(path, source, schema, parsed),
        errors,
        complete,
    )
}

fn project_document(
    path: PathBuf,
    source: String,
    schema: Option<&Schema>,
    parsed: markdown::ParsedDocument,
) -> Document {
    let line_starts = source_lines(&source)
        .iter()
        .map(|line| line.start)
        .collect::<Vec<_>>();
    let items = parsed
        .items
        .into_iter()
        .map(|parsed| {
            let metadata = parsed
                .metadata
                .into_iter()
                .map(|entry| MetadataEntry {
                    key: entry.key,
                    value: entry.value,
                    source: location(&path, &line_starts, entry.source.start, entry.source.end),
                })
                .collect::<Vec<_>>();
            let mut relations = metadata
                .iter()
                .filter_map(|entry| {
                    let schema = schema?;
                    if schema
                        .flavours()
                        .get(&parsed.flavour)
                        .is_some_and(|flavour| flavour.fields.contains_key(&entry.key))
                    {
                        return None;
                    }
                    let (canonical, definition, inverse) = schema.resolve_relation(&entry.key)?;
                    Some(Relation {
                        name: entry.key.clone(),
                        canonical: canonical.to_owned(),
                        inverse,
                        symmetric: definition.symmetric,
                        inline: false,
                        target: entry.value.clone(),
                        source: entry.source.clone(),
                    })
                })
                .collect::<Vec<_>>();
            let mut inline_diagnostics = Vec::new();
            for token in parsed.mentions.iter().filter(|token| token.typed) {
                let (name, target) = token.target.split_once(':').unwrap_or(("", ""));
                let source_location = location(&path, &line_starts, token.source.start, token.source.end);
                if !crate::is_snake_name(name)
                    || (!crate::is_item_id(target) && !crate::is_mid(target) && !target.starts_with("external:") && !target.starts_with("code:"))
                    || source[token.source.clone()] != format!("[[{}]]", token.target)
                {
                    diagnostic(DiagnosticCode::RelationInvalid, &mut inline_diagnostics, &source_location,
                        "invalid typed inline reference; expected [[relation:ID]], [[relation:MID]] or [[relation:external:URL]] without whitespace or markup".into());
                    continue;
                }
                let Some(schema) = schema else { continue };
                let Some((canonical, definition, inverse)) = schema.resolve_relation(name) else {
                    diagnostic(DiagnosticCode::RelationInvalid, &mut inline_diagnostics, &source_location, format!("unknown inline relation '{name}'"));
                    continue;
                };
                relations.push(Relation {
                    name: name.to_owned(),
                    canonical: canonical.to_owned(),
                    inverse,
                    symmetric: definition.symmetric,
                    inline: true,
                    target: target.to_owned(),
                    source: source_location,
                });
            }
            let mentions = parsed
                .mentions
                .into_iter()
                .filter(|mention| !mention.typed)
                .map(|mention| Mention {
                    target: mention.target,
                    source: location(
                        &path,
                        &line_starts,
                        mention.source.start,
                        mention.source.end,
                    ),
                })
                .collect();
            Item {
                flavour: parsed.flavour,
                id: parsed.id,
                mid: metadata
                    .iter()
                    .find(|entry| entry.key == "mid" && crate::is_mid(&entry.value))
                    .map(|entry| entry.value.clone()),
                title: parsed.title,
                metadata,
                body: source[parsed.body.clone()].to_owned(),
                body_blocks: parsed
                    .blocks
                    .into_iter()
                    .map(|block| project_block(&path, &line_starts, block))
                    .collect(),
                relations,
                inline_diagnostics,
                mentions,
                source: location(&path, &line_starts, parsed.source.start, parsed.source.end),
                body_source: location(&path, &line_starts, parsed.body.start, parsed.body.end),
                metadata_valid: parsed.metadata_valid,
                title_valid: parsed.title_valid,
                body_valid: parsed.body_valid,
            }
        })
        .collect();

    let blocks = parsed
        .blocks
        .into_iter()
        .map(|block| project_block(&path, &line_starts, block))
        .collect();
    let references = parsed
        .references
        .into_iter()
        .map(|reference| DocumentReference {
            kind: reference.kind,
            target: reference.target,
            source: location(
                &path,
                &line_starts,
                reference.source.start,
                reference.source.end,
            ),
        })
        .collect();
    Document {
        references,
        path,
        source,
        items,
        blocks,
    }
}

fn project_block(
    path: &Path,
    line_starts: &[usize],
    block: markdown::ParsedBlock,
) -> MarkdownBlock {
    MarkdownBlock {
        kind: block.kind,
        heading_text: block.heading_text,
        heading_source_offsets: block.heading_source_offsets,
        source: location(path, line_starts, block.source.start, block.source.end),
        children: block
            .children
            .into_iter()
            .map(|child| project_block(path, line_starts, child))
            .collect(),
    }
}

#[derive(Clone, Copy)]
struct SourceLine {
    start: usize,
}

fn source_lines(source: &str) -> Vec<SourceLine> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset);
        let full_end = newline.map_or(bytes.len(), |offset| offset + 1);
        lines.push(SourceLine { start });
        start = full_end;
    }
    lines
}

pub(crate) fn location(
    path: &Path,
    line_starts: &[usize],
    start: usize,
    end: usize,
) -> SourceLocation {
    SourceLocation {
        path: path.to_path_buf(),
        span: SourceSpan {
            start_byte: start,
            end_byte: end,
            start_line: line_number(line_starts, start),
            end_line: line_number(line_starts, end.saturating_sub(1).max(start)),
        },
    }
}

fn line_number(line_starts: &[usize], byte: usize) -> usize {
    if line_starts.is_empty() {
        return 1;
    }
    line_starts.partition_point(|start| *start <= byte).max(1)
}

fn invalid(path: &Path, line: usize, message: impl Into<String>) -> Error {
    Error::InvalidDocument {
        path: path.to_path_buf(),
        line,
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    documents: DocumentSet,
    code: crate::CodeIndex,
}
impl Corpus {
    pub fn discovery(&self) -> crate::DiscoveryGraph<'_> {
        crate::DiscoveryGraph::new(self)
    }
    pub fn documents(&self) -> &[Document] {
        self.documents.documents()
    }
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents.items()
    }
    pub fn is_complete(&self) -> bool {
        self.documents.is_complete()
    }
    pub(crate) fn code(&self) -> &crate::CodeIndex {
        &self.code
    }
    pub(crate) fn file_only_code_paths(&self) -> std::collections::BTreeSet<PathBuf> {
        self.items()
            .flat_map(Item::relations)
            .filter_map(|relation| {
                let (path, selector) = crate::code::split_reference(relation.target()).ok()?;
                selector.is_none().then_some(path)
            })
            .collect()
    }
}

pub fn load_corpus(project: &Project, schema: &Schema) -> Result<Corpus, Error> {
    let documents = load_documents(project, schema)?;
    let (code, problems) = crate::CodeIndex::load(project);
    if let Some(problem) = problems.into_iter().next() {
        return Err(Error::InvalidProject {
            path: project.root().join(problem.source.path()),
            message: problem.message,
        });
    }
    Ok(Corpus { documents, code })
}
