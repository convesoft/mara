use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{BodyRequirement, Error, FieldType, PROJECT_FILE, Project, Schema};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

mod markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    documents: Vec<Document>,
}

impl Corpus {
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents.iter().flat_map(|document| document.items())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    path: PathBuf,
    source: String,
    items: Vec<Item>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    flavour: String,
    id: String,
    title: String,
    metadata: Vec<MetadataEntry>,
    body: String,
    relations: Vec<Relation>,
    mentions: Vec<Mention>,
    source: SourceLocation,
    body_source: SourceLocation,
}

impl Item {
    pub fn flavour(&self) -> &str {
        &self.flavour
    }

    pub fn id(&self) -> &str {
        &self.id
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
    message: String,
}

impl Diagnostic {
    pub fn source(&self) -> &SourceLocation {
        &self.source
    }
    pub fn applies_to_item(&self, item_id: &str) -> bool {
        self.item_ids.iter().any(|existing| existing == item_id)
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn validate_corpus(corpus: &Corpus, schema: &Schema) -> Vec<Diagnostic> {
    let mut diagnostics = validate_corpus_independent(corpus);
    let ids = item_index(corpus);

    for item in corpus.items() {
        let Some(flavour) = schema.flavour_for_validation(item.flavour()) else {
            diagnostic(
                &mut diagnostics,
                item.source(),
                format!("unknown flavour '{}'", item.flavour()),
            );
            for relation in item.relations() {
                if schema.relation_is_valid(relation.name()) && !ids.contains_key(relation.target())
                {
                    diagnostic(
                        &mut diagnostics,
                        relation.source(),
                        format!(
                            "relation '{}' references missing item '{}'",
                            relation.name(),
                            relation.target()
                        ),
                    );
                }
            }
            continue;
        };
        if schema.id_prefix_is_valid(item.flavour()) && !item.id().starts_with(&flavour.id_prefix) {
            diagnostic(
                &mut diagnostics,
                item.source(),
                format!(
                    "item ID '{}' must start with '{}' for flavour '{}'",
                    item.id(),
                    flavour.id_prefix,
                    item.flavour()
                ),
            );
        }
        if flavour.body == BodyRequirement::Required && item.body().trim().is_empty() {
            diagnostic(
                &mut diagnostics,
                item.body_source(),
                "required body is empty".into(),
            );
        }

        let mut fields: BTreeMap<&str, Vec<&MetadataEntry>> = BTreeMap::new();
        for entry in item
            .metadata()
            .iter()
            .filter(|entry| entry.key() != "title")
        {
            if let Some(field) = flavour.fields.get(entry.key()) {
                if schema.field_is_valid(item.flavour(), entry.key()) {
                    fields.entry(entry.key()).or_default().push(entry);
                    if schema.field_values_are_valid(item.flavour(), entry.key())
                        && !valid_field_value(
                            field.field_type,
                            field.values.as_deref(),
                            entry.value(),
                        )
                    {
                        diagnostic(
                            &mut diagnostics,
                            entry.source(),
                            format!(
                                "invalid {} value '{}' for field '{}'",
                                field_type_name(field.field_type),
                                entry.value(),
                                entry.key()
                            ),
                        );
                    }
                }
            } else if let Some(relation) = schema.relations.get(entry.key()) {
                if schema.relation_is_valid(entry.key())
                    && schema.relation_source_is_valid(entry.key())
                    && !relation
                        .source
                        .iter()
                        .any(|source| source == item.flavour())
                {
                    diagnostic(
                        &mut diagnostics,
                        entry.source(),
                        format!(
                            "relation '{}' does not allow source flavour '{}'",
                            entry.key(),
                            item.flavour()
                        ),
                    );
                }
            } else {
                diagnostic(
                    &mut diagnostics,
                    entry.source(),
                    format!("unknown metadata field '{}'", entry.key()),
                );
            }
        }
        for (name, field) in &flavour.fields {
            if !schema.field_is_valid(item.flavour(), name) {
                continue;
            }
            let entries = fields
                .get(name.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            if field.required && entries.is_empty() {
                diagnostic(
                    &mut diagnostics,
                    item.source(),
                    format!("required field '{name}' is missing"),
                );
            }
            if !field.repeatable && entries.len() > 1 {
                for entry in &entries[1..] {
                    diagnostic(
                        &mut diagnostics,
                        entry.source(),
                        format!("field '{name}' is not repeatable"),
                    );
                }
            }
        }
        for relation in item.relations() {
            if let Some(definition) = schema.relations.get(relation.name()) {
                if !schema.relation_is_valid(relation.name()) {
                    continue;
                }
                if let Some(targets) = ids.get(relation.target()) {
                    if targets.len() == 1 {
                        let target = targets[0];
                        if schema.relation_target_is_valid(relation.name())
                            && !definition
                                .target
                                .iter()
                                .any(|flavour| flavour == target.flavour())
                        {
                            diagnostic(
                                &mut diagnostics,
                                relation.source(),
                                format!(
                                    "relation '{}' does not allow target flavour '{}'",
                                    relation.name(),
                                    target.flavour()
                                ),
                            );
                        }
                        if definition.same_flavour
                            && schema.same_flavour_is_valid(relation.name())
                            && target.flavour() != item.flavour()
                        {
                            diagnostic(
                                &mut diagnostics,
                                relation.source(),
                                format!(
                                    "relation '{}' requires matching source and target flavours",
                                    relation.name()
                                ),
                            );
                        }
                    }
                } else {
                    diagnostic(
                        &mut diagnostics,
                        relation.source(),
                        format!(
                            "relation '{}' references missing item '{}'",
                            relation.name(),
                            relation.target()
                        ),
                    );
                }
            }
        }
    }
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

pub fn validate_corpus_independent(corpus: &Corpus) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let ids = item_index(corpus);

    for duplicates in ids.values().filter(|items| items.len() > 1) {
        for item in duplicates {
            diagnostic(
                &mut diagnostics,
                item.source(),
                format!("duplicate item ID '{}'", item.id()),
            );
        }
    }

    for item in corpus.items() {
        for mention in item.mentions() {
            if !ids.contains_key(mention.target()) {
                diagnostic(
                    &mut diagnostics,
                    mention.source(),
                    format!("mention references missing item '{}'", mention.target()),
                );
            }
        }
    }
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn item_index(corpus: &Corpus) -> BTreeMap<&str, Vec<&Item>> {
    let mut ids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
    for item in corpus.items() {
        ids.entry(item.id()).or_default().push(item);
    }
    ids
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (a.source.path(), a.source.span().start_line(), &a.message).cmp(&(
            b.source.path(),
            b.source.span().start_line(),
            &b.message,
        ))
    });
}

pub fn load_corpus_for_validation(
    project: &Project,
    schema: &Schema,
) -> Result<(Corpus, Vec<Diagnostic>), Error> {
    load_corpus_for_validation_with_schema(project, Some(schema))
}

pub fn load_corpus_syntax_for_validation(
    project: &Project,
) -> Result<(Corpus, Vec<Diagnostic>), Error> {
    load_corpus_for_validation_with_schema(project, None)
}

fn load_corpus_for_validation_with_schema(
    project: &Project,
    schema: Option<&Schema>,
) -> Result<(Corpus, Vec<Diagnostic>), Error> {
    let matcher = content_matcher(project)?;
    let paths = discover(project.root(), &matcher)?;
    let mut documents = Vec::with_capacity(paths.len());
    let mut diagnostics = Vec::new();
    for relative_path in paths {
        let absolute_path = project.root().join(&relative_path);
        let source = match fs::read_to_string(&absolute_path) {
            Ok(source) => source,
            Err(error) => {
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
                    message: format!("could not read Mara document: {error}"),
                });
                continue;
            }
        };
        let (document, errors) =
            parse_document_for_validation(relative_path.clone(), source, schema);
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
            message: error.message,
        }));
        if retain_document {
            documents.push(document);
        }
    }
    Ok((Corpus { documents }, diagnostics))
}

fn diagnostic(diagnostics: &mut Vec<Diagnostic>, source: &SourceLocation, message: String) {
    diagnostics.push(Diagnostic {
        source: source.clone(),
        item_ids: Vec::new(),
        message,
    });
}

fn valid_field_value(kind: FieldType, values: Option<&[String]>, value: &str) -> bool {
    match kind {
        FieldType::String => true,
        FieldType::Integer => value.parse::<i64>().is_ok(),
        FieldType::Number => value.parse::<f64>().is_ok(),
        FieldType::Boolean => matches!(value, "true" | "false"),
        FieldType::Enum => {
            values.is_some_and(|values| values.iter().any(|candidate| candidate == value))
        }
    }
}

fn field_type_name(kind: FieldType) -> &'static str {
    match kind {
        FieldType::String => "string",
        FieldType::Integer => "integer",
        FieldType::Number => "number",
        FieldType::Boolean => "boolean",
        FieldType::Enum => "enum",
    }
}

pub fn load_corpus(project: &Project, schema: &Schema) -> Result<Corpus, Error> {
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
    Ok(Corpus { documents })
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

    let mut paths = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(|source| Error::Io {
            action: "discover Mara documents",
            path: root.to_path_buf(),
            source: io::Error::other(source),
        })?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("discovered content remains below the project root")
            .to_path_buf();
        if is_mara_document(&relative) && matcher.is_match(&relative) {
            paths.push(relative);
        }
    }
    paths.sort();
    Ok(paths)
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
) -> (Document, Vec<markdown::ParseError>) {
    let (parsed, errors) = markdown::parse_for_validation(&source);
    (project_document(path, source, schema, parsed), errors)
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
            let relations = metadata
                .iter()
                .filter(|entry| {
                    schema.is_some_and(|schema| schema.relations().contains_key(&entry.key))
                })
                .map(|entry| Relation {
                    name: entry.key.clone(),
                    target: entry.value.clone(),
                    source: entry.source.clone(),
                })
                .collect();
            let mentions = parsed
                .mentions
                .into_iter()
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
                title: parsed.title,
                metadata,
                body: source[parsed.body.clone()].to_owned(),
                relations,
                mentions,
                source: location(&path, &line_starts, parsed.source.start, parsed.source.end),
                body_source: location(&path, &line_starts, parsed.body.start, parsed.body.end),
            }
        })
        .collect();

    Document {
        path,
        source,
        items,
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

fn location(path: &Path, line_starts: &[usize], start: usize, end: usize) -> SourceLocation {
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
