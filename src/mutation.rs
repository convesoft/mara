use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use tempfile::NamedTempFile;

use crate::{
    BodyRequirement, Corpus, Error, FieldDefinition, FieldType, Item, Project, Schema,
    corpus::{document_is_discoverable, parse_document_source},
    is_item_id, load_corpus, load_corpus_for_validation, validate_corpus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCreationRequest {
    pub flavour: String,
    pub id: String,
    pub file: PathBuf,
    pub title: String,
    pub fields: Vec<(String, String)>,
    pub body: Option<String>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCreation {
    path: PathBuf,
    line: usize,
    mid: String,
    complete: bool,
}

impl ItemCreation {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn mid(&self) -> &str {
        &self.mid
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationMutation {
    source: String,
    source_mid: Option<String>,
    relation: String,
    target: String,
    target_mid: Option<String>,
    path: PathBuf,
}

impl RelationMutation {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_mid(&self) -> Option<&str> {
        self.source_mid.as_deref()
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn target_mid(&self) -> Option<&str> {
        self.target_mid.as_deref()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfilledMids {
    entries: Vec<BackfilledMid>,
}

impl BackfilledMids {
    pub fn entries(&self) -> &[BackfilledMid] {
        &self.entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfilledMid {
    id: String,
    mid: String,
    path: PathBuf,
    line: usize,
}

impl BackfilledMid {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn mid(&self) -> &str {
        &self.mid
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self) -> usize {
        self.line
    }
}

pub fn create_item(
    project: &Project,
    schema: &Schema,
    request: ItemCreationRequest,
) -> Result<ItemCreation, Error> {
    let corpus = load_corpus(project, schema)?;
    validate_new_item(&corpus, schema, &request)?;
    let (path, absolute, existed) = resolve_document_path(project, &request.file)?;
    if !document_is_discoverable(project, &path)? {
        return invalid(format!(
            "destination file '{}' is excluded by project content discovery",
            path.display()
        ));
    }
    let source = if existed {
        fs::read_to_string(&absolute).map_err(|source| Error::Io {
            action: "read Mara document",
            path: absolute.clone(),
            source,
        })?
    } else {
        String::new()
    };
    let document = parse_document_source(&path, &source, schema)?;
    if document.items().iter().any(|item| item.id() == request.id) {
        return invalid(format!("item '{}' already exists", request.id));
    }

    let position = insertion_position(&source, request.line)?;
    if let Some(item) = document.items().iter().find(|item| {
        let span = item.source().span();
        position > span.start_byte() && position < span.end_byte()
    }) {
        return invalid(format!(
            "line {} is inside item '{}'",
            request
                .line
                .expect("only explicit positions can be inside an item"),
            item.id()
        ));
    }

    let newline = newline_style(&source);
    let mid = generate_mid(corpus.items().filter_map(Item::mid));
    let block = render_item(&request, &mid, newline);
    let candidate = insert_block(&source, position, &block, newline);
    let projected = parse_document_source(&path, &candidate, schema)?;
    let created = projected
        .items()
        .iter()
        .find(|item| item.id() == request.id)
        .ok_or_else(|| Error::InvalidMutation {
            message: format!("created item '{}' could not be resolved", request.id),
        })?;
    let expected_body = render_body(request.body.as_deref(), newline);
    if projected.items().len() != document.items().len() + 1 || created.body() != expected_body {
        return invalid("item body must remain inside the created item");
    }
    let line = created.source().span().start_line();
    let complete = schema
        .flavours
        .get(&request.flavour)
        .is_some_and(|flavour| {
            flavour.body == BodyRequirement::Optional
                || request
                    .body
                    .as_deref()
                    .is_some_and(|body| !body.trim().is_empty())
        });

    atomic_replace(&absolute, &candidate, existed)?;
    Ok(ItemCreation {
        path,
        line,
        mid,
        complete,
    })
}

pub fn backfill_mids(project: &Project, schema: &Schema) -> Result<BackfilledMids, Error> {
    let (corpus, mut diagnostics) = load_corpus_for_validation(project, schema)?;
    diagnostics.extend(validate_corpus(&corpus, schema));
    let blocking = diagnostics
        .into_iter()
        .filter(|diagnostic| !diagnostic.message().contains(" is missing its MID"))
        .collect::<Vec<_>>();
    if let Some(diagnostic) = blocking.first() {
        return invalid(format!(
            "cannot backfill MIDs while validation fails at {}:{}: {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.message()
        ));
    }

    let mut known_mids = corpus
        .items()
        .filter_map(Item::mid)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut by_path = BTreeMap::<PathBuf, Vec<(usize, BackfilledMid)>>::new();
    for document in corpus.documents() {
        for item in document.items().iter().filter(|item| item.mid().is_none()) {
            let mid = generate_mid(known_mids.iter().map(String::as_str));
            known_mids.push(mid.clone());
            by_path
                .entry(document.path().to_path_buf())
                .or_default()
                .push((
                    end_of_line_containing(document.source(), item.source().span().start_byte()),
                    BackfilledMid {
                        id: item.id().to_owned(),
                        mid,
                        path: document.path().to_path_buf(),
                        line: item.source().span().start_line() + 1,
                    },
                ));
        }
    }

    let mut candidates = BTreeMap::<PathBuf, String>::new();
    let mut entries = Vec::new();
    for (path, mut insertions) in by_path {
        let document = corpus
            .documents()
            .iter()
            .find(|document| document.path() == path)
            .expect("backfill target belongs to the loaded corpus");
        let newline = newline_style(document.source());
        insertions.sort_by_key(|(position, _)| *position);
        let mut candidate = document.source().to_owned();
        for (position, entry) in insertions.iter().rev() {
            candidate.insert_str(*position, &format!(":mid: {}{newline}", entry.mid()));
        }
        parse_document_source(&path, &candidate, schema)?;
        entries.extend(insertions.into_iter().map(|(_, entry)| entry));
        candidates.insert(path, candidate);
    }

    for (path, candidate) in candidates {
        atomic_replace(&project.root().join(&path), &candidate, true)?;
    }

    Ok(BackfilledMids { entries })
}

pub fn add_relation(
    project: &Project,
    schema: &Schema,
    source: &str,
    relation: &str,
    target: &str,
) -> Result<RelationMutation, Error> {
    mutate_relation(project, schema, source, relation, target, MutationKind::Add)
}

pub fn remove_relation(
    project: &Project,
    schema: &Schema,
    source: &str,
    relation: &str,
    target: &str,
) -> Result<RelationMutation, Error> {
    mutate_relation(
        project,
        schema,
        source,
        relation,
        target,
        MutationKind::Remove,
    )
}

#[derive(Debug, Clone, Copy)]
enum MutationKind {
    Add,
    Remove,
}

fn mutate_relation(
    project: &Project,
    schema: &Schema,
    source_id: &str,
    relation_name: &str,
    target_id: &str,
    kind: MutationKind,
) -> Result<RelationMutation, Error> {
    let corpus = load_corpus(project, schema)?;
    ensure_unambiguous_item_identities(&corpus)?;
    let source_item = resolve_item(&corpus, source_id, "source")?;
    let target_item = resolve_item(&corpus, target_id, "target")?;
    let definition = schema
        .relations
        .get(relation_name)
        .ok_or_else(|| Error::InvalidMutation {
            message: format!("unknown relation '{relation_name}'"),
        })?;
    if !definition
        .source
        .iter()
        .any(|flavour| flavour == source_item.flavour())
    {
        return invalid(format!(
            "relation '{relation_name}' does not allow source flavour '{}'",
            source_item.flavour()
        ));
    }
    if !definition
        .target
        .iter()
        .any(|flavour| flavour == target_item.flavour())
    {
        return invalid(format!(
            "relation '{relation_name}' does not allow target flavour '{}'",
            target_item.flavour()
        ));
    }
    if definition.same_flavour && source_item.flavour() != target_item.flavour() {
        return invalid(format!(
            "relation '{relation_name}' requires matching source and target flavours"
        ));
    }

    let path = source_item.source().path().to_path_buf();
    let document = corpus
        .documents()
        .iter()
        .find(|document| document.path() == path)
        .expect("a corpus item belongs to a corpus document");
    let authored = source_item
        .relations()
        .iter()
        .filter(|existing| {
            existing.name() == relation_name
                && resolve_item(&corpus, existing.target(), "target")
                    .is_ok_and(|item| same_item_identity(item, target_item))
        })
        .collect::<Vec<_>>();
    let candidate = match kind {
        MutationKind::Add if !authored.is_empty() => {
            return invalid(format!(
                "item '{source_id}' already has relation '{relation_name}' to '{target_id}'"
            ));
        }
        MutationKind::Add => {
            let insertion = source_item
                .metadata()
                .last()
                .expect("every parsed item has title metadata")
                .source()
                .span()
                .end_byte();
            let newline = newline_style(document.source());
            let entry = format!("{newline}:{relation_name}: {}", target_item.id());
            let mut candidate = document.source().to_owned();
            candidate.insert_str(insertion, &entry);
            candidate
        }
        MutationKind::Remove if authored.is_empty() => {
            return invalid(format!(
                "item '{source_id}' has no relation '{relation_name}' to '{target_id}'"
            ));
        }
        MutationKind::Remove => {
            let mut candidate = document.source().to_owned();
            let mut spans = authored
                .iter()
                .map(|existing| existing.source().span())
                .collect::<Vec<_>>();
            spans.sort_by_key(|span| span.start_byte());
            for span in spans.into_iter().rev() {
                let end = full_line_end(&candidate, span.end_byte());
                candidate.replace_range(span.start_byte()..end, "");
            }
            candidate
        }
    };

    parse_document_source(&path, &candidate, schema)?;
    atomic_replace(&project.root().join(&path), &candidate, true)?;
    Ok(RelationMutation {
        source: source_item.id().to_owned(),
        source_mid: source_item.mid().map(ToOwned::to_owned),
        relation: relation_name.to_owned(),
        target: target_item.id().to_owned(),
        target_mid: target_item.mid().map(ToOwned::to_owned),
        path,
    })
}

fn validate_new_item(
    corpus: &Corpus,
    schema: &Schema,
    request: &ItemCreationRequest,
) -> Result<(), Error> {
    let flavour = schema
        .flavours
        .get(&request.flavour)
        .ok_or_else(|| Error::InvalidMutation {
            message: format!("unknown flavour '{}'", request.flavour),
        })?;
    if !is_item_id(&request.id) {
        return invalid(format!("invalid item ID '{}'", request.id));
    }
    if !request.id.starts_with(&flavour.id_prefix) {
        return invalid(format!(
            "item ID '{}' must start with '{}' for flavour '{}'",
            request.id, flavour.id_prefix, request.flavour
        ));
    }
    if corpus.items().any(|item| item.id() == request.id) {
        return invalid(format!("item '{}' already exists", request.id));
    }
    validate_scalar("title", &request.title)?;
    if request.title.trim().is_empty() {
        return invalid("title must not be empty");
    }

    let mut counts = BTreeMap::<&str, usize>::new();
    for (name, value) in &request.fields {
        let definition = flavour
            .fields
            .get(name)
            .ok_or_else(|| Error::InvalidMutation {
                message: format!("unknown field '{name}' for flavour '{}'", request.flavour),
            })?;
        validate_scalar(&format!("field '{name}'"), value)?;
        let value = value.trim();
        if !valid_field_value(definition, value) {
            return invalid(format!(
                "invalid {} value '{value}' for field '{name}'",
                field_type_name(definition.field_type)
            ));
        }
        *counts.entry(name).or_default() += 1;
    }
    for (name, definition) in &flavour.fields {
        let count = counts.get(name.as_str()).copied().unwrap_or_default();
        if definition.required && count == 0 {
            return invalid(format!("required field '{name}' is missing"));
        }
        if !definition.repeatable && count > 1 {
            return invalid(format!("field '{name}' is not repeatable"));
        }
    }
    Ok(())
}

fn validate_scalar(label: &str, value: &str) -> Result<(), Error> {
    if value.contains(['\n', '\r']) {
        return invalid(format!("{label} must be a single-line value"));
    }
    Ok(())
}

fn valid_field_value(definition: &FieldDefinition, value: &str) -> bool {
    match definition.field_type {
        FieldType::String => true,
        FieldType::Integer => value.parse::<i64>().is_ok(),
        FieldType::Number => value.parse::<f64>().is_ok(),
        FieldType::Boolean => matches!(value, "true" | "false"),
        FieldType::Enum => definition
            .values
            .as_ref()
            .is_some_and(|values| values.iter().any(|candidate| candidate == value)),
    }
}

fn field_type_name(field_type: FieldType) -> &'static str {
    match field_type {
        FieldType::String => "string",
        FieldType::Integer => "integer",
        FieldType::Number => "number",
        FieldType::Boolean => "boolean",
        FieldType::Enum => "enum",
    }
}

fn resolve_item<'a>(corpus: &'a Corpus, id: &str, endpoint: &str) -> Result<&'a Item, Error> {
    let by_mid = crate::is_mid(id);
    let matches = corpus
        .items()
        .filter(|item| {
            if by_mid {
                item.mid() == Some(id)
            } else {
                item.id() == id
            }
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [item] => Ok(item),
        [] => invalid(format!("relation {endpoint} item '{id}' was not found")),
        _ if by_mid => invalid(format!("relation {endpoint} item MID '{id}' is ambiguous")),
        _ => invalid(format!("relation {endpoint} item '{id}' is ambiguous")),
    }
}

fn same_item_identity(left: &Item, right: &Item) -> bool {
    match (left.mid(), right.mid()) {
        (Some(left_mid), Some(right_mid)) => left_mid == right_mid,
        _ => left.id() == right.id(),
    }
}

fn ensure_unambiguous_item_identities(corpus: &Corpus) -> Result<(), Error> {
    let mut ids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
    let mut mids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();

    for item in corpus.items() {
        ids.entry(item.id()).or_default().push(item);
        if let Some(mid) = item.mid() {
            mids.entry(mid).or_default().push(item);
        }
    }

    if let Some((id, _)) = ids.iter().find(|(_, items)| items.len() > 1) {
        return invalid(format!(
            "cannot mutate relations while item ID '{id}' is ambiguous; run project validate"
        ));
    }
    if let Some((mid, _)) = mids.iter().find(|(_, items)| items.len() > 1) {
        return invalid(format!(
            "cannot mutate relations while item MID '{mid}' is ambiguous; run project validate"
        ));
    }

    Ok(())
}

fn resolve_document_path(
    project: &Project,
    requested: &Path,
) -> Result<(PathBuf, PathBuf, bool), Error> {
    if requested.as_os_str().is_empty()
        || requested.is_absolute()
        || requested.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return invalid("destination file must be a project-relative path");
    }
    let mut relative = PathBuf::new();
    for component in requested.components() {
        if component != Component::CurDir {
            relative.push(component.as_os_str());
        }
    }
    if relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !name.ends_with(".mara.md"))
    {
        return invalid("destination file must be named '*.mara.md'");
    }
    let absolute = project.root().join(&relative);
    let parent = absolute
        .parent()
        .expect("a project-relative file has a parent");
    let canonical_parent = fs::canonicalize(parent).map_err(|source| Error::Io {
        action: "resolve destination parent",
        path: parent.to_path_buf(),
        source,
    })?;
    if !canonical_parent.starts_with(project.root()) {
        return invalid("destination file must remain inside the project root");
    }
    let existed = absolute.try_exists().map_err(|source| Error::Io {
        action: "inspect destination file",
        path: absolute.clone(),
        source,
    })?;
    if existed {
        let metadata = fs::symlink_metadata(&absolute).map_err(|source| Error::Io {
            action: "inspect destination file",
            path: absolute.clone(),
            source,
        })?;
        if !metadata.file_type().is_file() {
            return invalid("destination must be a regular file");
        }
    }
    Ok((relative, absolute, existed))
}

fn insertion_position(source: &str, line: Option<usize>) -> Result<usize, Error> {
    let Some(line) = line else {
        return Ok(source.len());
    };
    let line_count = if source.is_empty() {
        0
    } else {
        source.bytes().filter(|byte| *byte == b'\n').count() + usize::from(!source.ends_with('\n'))
    };
    if line == 0 || line > line_count + 1 {
        return invalid(format!(
            "line {line} is outside the document; expected 1 through {}",
            line_count + 1
        ));
    }
    if line == line_count + 1 {
        return Ok(source.len());
    }
    if line == 1 {
        return Ok(0);
    }
    Ok(source
        .match_indices('\n')
        .nth(line - 2)
        .map(|(index, _)| index + 1)
        .expect("a requested existing line has a preceding newline"))
}

fn render_item(request: &ItemCreationRequest, mid: &str, newline: &str) -> String {
    let mut source = format!(
        ":::mara {} {}{newline}:mid: {mid}{newline}:title: {}",
        request.flavour,
        request.id,
        request.title.trim()
    );
    for (name, value) in &request.fields {
        source.push_str(newline);
        source.push(':');
        source.push_str(name);
        source.push_str(": ");
        source.push_str(value.trim());
    }
    source.push_str(newline);
    source.push_str(newline);
    source.push_str(&render_body(request.body.as_deref(), newline));
    source.push_str(":::");
    source.push_str(newline);
    source
}

fn render_body(body: Option<&str>, newline: &str) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let mut rendered = body.to_owned();
    if !body.ends_with('\n') {
        rendered.push_str(newline);
    }
    rendered
}

fn insert_block(source: &str, position: usize, block: &str, newline: &str) -> String {
    let (before, after) = source.split_at(position);
    let mut candidate = String::with_capacity(source.len() + block.len() + newline.len() * 2);
    candidate.push_str(before);
    if !before.is_empty() && !before.ends_with("\n\n") && !before.ends_with("\r\n\r\n") {
        if !before.ends_with('\n') {
            candidate.push_str(newline);
        }
        candidate.push_str(newline);
    }
    candidate.push_str(block);
    if !after.is_empty() && !after.starts_with('\n') && !after.starts_with("\r\n") {
        candidate.push_str(newline);
    }
    candidate.push_str(after);
    candidate
}

fn newline_style(source: &str) -> &'static str {
    source
        .find('\n')
        .filter(|index| *index > 0 && source.as_bytes()[index - 1] == b'\r')
        .map_or("\n", |_| "\r\n")
}

fn full_line_end(source: &str, end: usize) -> usize {
    if source[end..].starts_with("\r\n") {
        end + 2
    } else if source.as_bytes().get(end) == Some(&b'\n') {
        end + 1
    } else {
        end
    }
}

fn end_of_line_containing(source: &str, start: usize) -> usize {
    source[start..]
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1)
}

fn generate_mid<'a>(existing: impl IntoIterator<Item = &'a str>) -> String {
    let existing = existing.into_iter().collect::<Vec<_>>();
    loop {
        let mid = ulid::Ulid::new().to_string();
        if !existing.iter().any(|value| **value == mid) {
            return mid;
        }
    }
}

fn atomic_replace(path: &Path, source: &str, existed: bool) -> Result<(), Error> {
    let parent = path.parent().expect("a destination file has a parent");
    let permissions = existed
        .then(|| fs::metadata(path).map(|metadata| metadata.permissions()))
        .transpose()
        .map_err(|source| Error::Io {
            action: "inspect destination permissions",
            path: path.to_path_buf(),
            source,
        })?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| Error::Io {
        action: "create temporary Mara document",
        path: parent.to_path_buf(),
        source,
    })?;
    temporary
        .as_file_mut()
        .write_all(source.as_bytes())
        .map_err(|source| Error::Io {
            action: "write temporary Mara document",
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|source| Error::Io {
            action: "flush temporary Mara document",
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|source| Error::Io {
                action: "preserve destination permissions",
                path: path.to_path_buf(),
                source,
            })?;
    }
    temporary.as_file().sync_all().map_err(|source| Error::Io {
        action: "sync temporary Mara document",
        path: path.to_path_buf(),
        source,
    })?;
    let persisted = if existed {
        temporary.persist(path)
    } else {
        temporary.persist_noclobber(path)
    };
    persisted.map_err(|error| Error::Io {
        action: "atomically replace Mara document",
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidMutation {
        message: message.into(),
    })
}
