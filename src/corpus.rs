use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use crate::{BodyRequirement, Error, FieldType, PROJECT_FILE, Project, Schema};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, Error as WalkError, Walk, WalkBuilder};

mod markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpus {
    documents: Vec<Document>,
    complete: bool,
}

impl Corpus {
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents.iter().flat_map(|document| document.items())
    }

    pub fn is_complete(&self) -> bool {
        self.complete
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
    mid: Option<String>,
    title: String,
    metadata: Vec<MetadataEntry>,
    body: String,
    relations: Vec<Relation>,
    mentions: Vec<Mention>,
    source: SourceLocation,
    body_source: SourceLocation,
    metadata_valid: bool,
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

    fn metadata_is_valid(&self) -> bool {
        self.metadata_valid
    }

    fn body_is_valid(&self) -> bool {
        self.body_valid
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
    applies_to_all_items: bool,
    message: String,
}

impl Diagnostic {
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

pub fn validate_corpus(corpus: &Corpus, schema: &Schema) -> Vec<Diagnostic> {
    let mut diagnostics = validate_corpus_independent(corpus);
    let ids = item_index(corpus);
    let mids = mid_index(corpus);

    for item in corpus.items() {
        let Some(flavour) = schema.flavour_for_validation(item.flavour()) else {
            if !schema.flavour_is_declared(item.flavour()) {
                diagnostic(
                    &mut diagnostics,
                    item.source(),
                    format!("unknown flavour '{}'", item.flavour()),
                );
            }
            for relation in item.relations() {
                if schema.relation_is_valid(relation.name()) {
                    match resolve_indexed_item(&ids, &mids, relation.target()) {
                        IndexedItem::Missing if corpus.is_complete() => diagnostic(
                            &mut diagnostics,
                            relation.source(),
                            format!(
                                "relation '{}' references missing item '{}'",
                                relation.name(),
                                relation.target()
                            ),
                        ),
                        IndexedItem::Missing | IndexedItem::One(_) => {}
                        IndexedItem::Ambiguous => diagnostic(
                            &mut diagnostics,
                            relation.source(),
                            format!(
                                "relation '{}' references ambiguous item '{}'",
                                relation.name(),
                                relation.target()
                            ),
                        ),
                    }
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
        if item.body_is_valid()
            && schema.body_is_valid(item.flavour())
            && flavour.body == BodyRequirement::Required
            && item.body().trim().is_empty()
        {
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
            .filter(|entry| !matches!(entry.key(), "mid" | "title"))
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
            } else if schema.field_is_declared(item.flavour(), entry.key()) {
                continue;
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
            } else if schema.relation_is_valid(entry.key()) {
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
            if item.metadata_is_valid() && field.required && entries.is_empty() {
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
                match resolve_indexed_item(&ids, &mids, relation.target()) {
                    IndexedItem::One(target) => {
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
                    IndexedItem::Ambiguous => diagnostic(
                        &mut diagnostics,
                        relation.source(),
                        format!(
                            "relation '{}' references ambiguous item '{}'",
                            relation.name(),
                            relation.target()
                        ),
                    ),
                    IndexedItem::Missing if corpus.is_complete() => diagnostic(
                        &mut diagnostics,
                        relation.source(),
                        format!(
                            "relation '{}' references missing item '{}'",
                            relation.name(),
                            relation.target()
                        ),
                    ),
                    IndexedItem::Missing => {}
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
    let mids = mid_index(corpus);

    for duplicates in ids.values().filter(|items| items.len() > 1) {
        for item in duplicates {
            diagnostic(
                &mut diagnostics,
                item.source(),
                format!("duplicate item ID '{}'", item.id()),
            );
        }
    }

    for (mid, duplicates) in mids.iter().filter(|(_, items)| items.len() > 1) {
        for item in duplicates {
            if let Some(entry) = mid_entries(item)
                .into_iter()
                .find(|entry| entry.value() == *mid)
            {
                diagnostic(
                    &mut diagnostics,
                    entry.source(),
                    format!("duplicate item MID '{mid}'"),
                );
            }
        }
    }

    for item in corpus.items() {
        let mids = mid_entries(item);
        match mids.as_slice() {
            [] => diagnostic(
                &mut diagnostics,
                item.source(),
                format!("item '{}' is missing its MID", item.id()),
            ),
            [entry] => {
                if !crate::is_mid(entry.value()) {
                    diagnostic(
                        &mut diagnostics,
                        entry.source(),
                        format!("invalid item MID '{}'", entry.value()),
                    );
                }
                if entry.source().span().start_line() != item.source().span().start_line() + 1 {
                    diagnostic(
                        &mut diagnostics,
                        entry.source(),
                        format!(
                            "item '{}' MID must immediately follow its opener",
                            item.id()
                        ),
                    );
                }
            }
            [first, rest @ ..] => {
                if !crate::is_mid(first.value()) {
                    diagnostic(
                        &mut diagnostics,
                        first.source(),
                        format!("invalid item MID '{}'", first.value()),
                    );
                }
                if first.source().span().start_line() != item.source().span().start_line() + 1 {
                    diagnostic(
                        &mut diagnostics,
                        first.source(),
                        format!(
                            "item '{}' MID must immediately follow its opener",
                            item.id()
                        ),
                    );
                }
                for entry in rest {
                    diagnostic(
                        &mut diagnostics,
                        entry.source(),
                        format!("item '{}' has more than one MID entry", item.id()),
                    );
                    if !crate::is_mid(entry.value()) {
                        diagnostic(
                            &mut diagnostics,
                            entry.source(),
                            format!("invalid item MID '{}'", entry.value()),
                        );
                    }
                }
            }
        }
        for mention in item.mentions() {
            match ids.get(mention.target()).map(Vec::len) {
                None if corpus.is_complete() => diagnostic(
                    &mut diagnostics,
                    mention.source(),
                    format!("mention references missing item '{}'", mention.target()),
                ),
                None | Some(1) => {}
                Some(_) => diagnostic(
                    &mut diagnostics,
                    mention.source(),
                    format!("mention references ambiguous item '{}'", mention.target()),
                ),
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

fn mid_index(corpus: &Corpus) -> BTreeMap<&str, Vec<&Item>> {
    let mut mids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
    for item in corpus.items() {
        let mut seen = BTreeSet::new();
        for entry in mid_entries(item)
            .into_iter()
            .filter(|entry| crate::is_mid(entry.value()))
        {
            if seen.insert(entry.value()) {
                mids.entry(entry.value()).or_default().push(item);
            }
        }
    }
    mids
}

enum IndexedItem<'a> {
    Missing,
    One(&'a Item),
    Ambiguous,
}

fn resolve_indexed_item<'a>(
    ids: &BTreeMap<&str, Vec<&'a Item>>,
    mids: &BTreeMap<&str, Vec<&'a Item>>,
    handle: &str,
) -> IndexedItem<'a> {
    let matches = if crate::is_mid(handle) {
        mids.get(handle)
    } else {
        ids.get(handle)
    };
    match matches.map(Vec::as_slice) {
        Some([item]) => IndexedItem::One(item),
        Some(_) => IndexedItem::Ambiguous,
        None => IndexedItem::Missing,
    }
}

fn mid_entries(item: &Item) -> Vec<&MetadataEntry> {
    item.metadata()
        .iter()
        .filter(|entry| entry.key() == "mid")
        .collect()
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
            message: error.message,
        }));
        if retain_document {
            documents.push(document);
        }
    }
    Ok((
        Corpus {
            documents,
            complete,
        },
        diagnostics,
    ))
}

fn diagnostic(diagnostics: &mut Vec<Diagnostic>, source: &SourceLocation, message: String) {
    diagnostics.push(Diagnostic {
        source: source.clone(),
        item_ids: Vec::new(),
        applies_to_all_items: false,
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
    Ok(Corpus {
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

fn walk_error_path(root: &Path, error: &WalkError) -> PathBuf {
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

pub(crate) fn parse_document_source(
    path: &Path,
    source: &str,
    schema: &Schema,
) -> Result<Document, Error> {
    parse_document(path.to_path_buf(), source.to_owned(), schema)
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
                mid: metadata
                    .iter()
                    .find(|entry| entry.key == "mid" && crate::is_mid(&entry.value))
                    .map(|entry| entry.value.clone()),
                title: parsed.title,
                metadata,
                body: source[parsed.body.clone()].to_owned(),
                relations,
                mentions,
                source: location(&path, &line_starts, parsed.source.start, parsed.source.end),
                body_source: location(&path, &line_starts, parsed.body.start, parsed.body.end),
                metadata_valid: parsed.metadata_valid,
                body_valid: parsed.body_valid,
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
