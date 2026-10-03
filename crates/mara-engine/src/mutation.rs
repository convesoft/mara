use crate::corpus::parse_document_source;
use crate::{BodyRequirement, Corpus, FieldDefinition, FieldType, is_item_id, validate_corpus};
use crate::{ChangePlan, ChangePlanner, Error, Item, Schema};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
pub(super) mod delete;
pub use delete::ItemDeletion;
pub(super) mod rename;
pub use rename::ItemRename;
mod references;
pub(super) mod update;
pub use update::{ItemUpdate, ItemUpdateWarning};
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

// @mara implements REQ-MID-BACKFILL
// @mara implements DES-MID-BACKFILL
pub(super) fn backfill_mids(
    context: &ChangePlanner<'_>,
    mids: &[String],
) -> Result<ChangePlan<BackfilledMids>, Error> {
    let corpus = &context.corpus;
    let schema = &context.schema;
    let diagnostics = validate_corpus(corpus, schema);
    let mut mids = mids.iter();
    let blocking = diagnostics
        .into_iter()
        .filter(|diagnostic| !diagnostic.is_missing_mid())
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
            let mid = supplied_mid(
                mids.next().map(String::as_str),
                known_mids.iter().map(String::as_str),
            )?;
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
        for (offset, (_, entry)) in insertions.iter_mut().enumerate() {
            entry.line += offset;
        }
        let mut candidate = document.source().to_owned();
        for (position, entry) in insertions.iter().rev() {
            candidate.insert_str(*position, &format!(":mid: {}{newline}", entry.mid()));
        }
        parse_document_source(&path, &candidate, schema)?;
        entries.extend(insertions.into_iter().map(|(_, entry)| entry));
        candidates.insert(path, candidate);
    }

    let projected = corpus.with_replacements(&candidates, schema)?;
    if let Some(diagnostic) = validate_corpus(&projected, schema).first() {
        return invalid(diagnostic.message());
    }
    references::preflight_backfill(corpus, &projected)?;
    context.finish(candidates, BackfilledMids { entries })
}

fn newline_style(source: &str) -> &'static str {
    source
        .find('\n')
        .filter(|index| *index > 0 && source.as_bytes()[index - 1] == b'\r')
        .map_or("\n", |_| "\r\n")
}

fn end_of_line_containing(source: &str, start: usize) -> usize {
    source[start..]
        .find('\n')
        .map_or(source.len(), |offset| start + offset + 1)
}

fn supplied_mid<'a>(
    mid: Option<&str>,
    existing: impl IntoIterator<Item = &'a str>,
) -> Result<String, Error> {
    let mid = mid.ok_or_else(|| Error::InvalidMutation {
        message: "missing host-supplied MID".into(),
    })?;
    if !crate::is_mid(mid) || existing.into_iter().any(|value| value == mid) {
        return invalid("host-supplied MID must be canonical and unique");
    }
    Ok(mid.to_owned())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidMutation {
        message: message.into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemCreationRequest {
    pub flavour: String,
    pub id: String,
    pub file: PathBuf,
    pub title: String,
    pub fields: Vec<(String, String)>,
    pub relations: Vec<InitialRelation>,
    pub body: Option<String>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitialRelation {
    /// Schema-declared outgoing relation name, not a custom field.
    pub relation: String,
    /// Exact human ID, canonical MID, or external:HTTP(S) URL; the new item's human ID may target itself.
    pub target: String,
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

// @mara implements REQ-ITEM-CREATION
// @mara implements REQ-ITEM-INSERTION-SAFETY
// @mara implements DES-ITEM-CREATION
pub(super) fn create_item(
    context: &ChangePlanner<'_>,
    mut request: ItemCreationRequest,
    mid: &str,
) -> Result<ChangePlan<ItemCreation>, Error> {
    let project = &context.project;
    let schema = &context.schema;
    let corpus = &context.corpus;
    validate_new_item(corpus, schema, &request)?;
    if !request.relations.is_empty() {
        ensure_unambiguous_item_identities(corpus, "create an item with relations")?;
        let mut edges = BTreeSet::new();
        for edge in &mut request.relations {
            let (target_id, target_flavour) = if crate::external::address(&edge.target).is_some() {
                (edge.target.as_str(), None)
            } else if edge.target == request.id {
                (request.id.as_str(), Some(request.flavour.as_str()))
            } else {
                let target = resolve_item(corpus, &edge.target, "target")?;
                (target.id(), Some(target.flavour()))
            };
            if let Some(target_flavour) = target_flavour {
                validate_relation_endpoints(
                    schema,
                    &edge.relation,
                    &request.flavour,
                    target_flavour,
                )?;
            } else {
                validate_external_relation(schema, &edge.relation, &request.flavour, target_id)?;
            }
            let (canonical, _, inverse) = schema
                .resolve_relation(&edge.relation)
                .expect("validated relation");
            if !edges.insert((
                canonical.to_owned(),
                inverse && target_id != request.id,
                target_id.to_owned(),
            )) {
                return invalid(format!(
                    "item '{}' already has relation '{}' to '{}'",
                    request.id, edge.relation, edge.target
                ));
            }
            edge.target = target_id.to_owned();
        }
    }
    let path = resolve_document_path(context.engine, &request.file)?;
    if !context
        .engine
        .document_is_selected(project, path.to_str().expect("normalized path"))?
    {
        return invalid(format!(
            "destination file '{}' is excluded by project content discovery",
            path.display()
        ));
    }
    let source = context
        .engine
        .files()
        .get(path.to_str().expect("normalized path"))
        .cloned()
        .unwrap_or_default();
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
    let mid = supplied_mid(Some(mid), corpus.items().filter_map(Item::mid))?;
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
        .flavours()
        .get(&request.flavour)
        .is_some_and(|flavour| {
            flavour.body == BodyRequirement::Optional
                || request
                    .body
                    .as_deref()
                    .is_some_and(|body| !body.trim().is_empty())
        });

    let candidate_corpus =
        corpus.with_replacements(&BTreeMap::from([(path.clone(), candidate.clone())]), schema)?;
    references::preflight(corpus, &candidate_corpus, None, None)?;
    if let Some(diagnostic) = candidate_corpus
        .discovery()
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.source().path() == created.source().path()
                && diagnostic.source().span().start_byte() >= created.source().span().start_byte()
                && diagnostic.source().span().end_byte() <= created.source().span().end_byte()
        })
    {
        return invalid(format!(
            "cannot create item while reference validation fails at {}:{} (bytes {}..{}): {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.source().span().start_byte(),
            diagnostic.source().span().end_byte(),
            diagnostic.message()
        ));
    }
    if let Some(diagnostic) =
        validate_corpus(&candidate_corpus, schema)
            .into_iter()
            .find(|diagnostic| {
                (diagnostic.applies_to_item(&request.id)
                    || (diagnostic.source().path() == created.source().path()
                        && diagnostic.source().span().start_byte()
                            >= created.source().span().start_byte()
                        && diagnostic.source().span().end_byte()
                            <= created.source().span().end_byte()))
                    && !(!complete && diagnostic.is_missing_body())
            })
    {
        return invalid(diagnostic.message());
    }
    for initial in &request.relations {
        let edge = crate::relations::resolve_edge(
            &candidate_corpus,
            schema,
            &request.id,
            &initial.relation,
            &initial.target,
        )
        .map_err(|error| Error::InvalidMutation {
            message: error.to_string(),
        })?;
        let count = crate::relations::occurrences(project, &candidate_corpus, schema, &edge)
            .map_err(|error| Error::InvalidMutation {
                message: error.to_string(),
            })?
            .len();
        if count > 1 {
            return invalid("initial relation already has an equivalent assertion in the project");
        }
    }
    context.finish(
        BTreeMap::from([(path.clone(), candidate)]),
        ItemCreation {
            path,
            line,
            mid,
            complete,
        },
    )
}

fn validate_relation_endpoints(
    schema: &Schema,
    relation_name: &str,
    source_flavour: &str,
    target_flavour: &str,
) -> Result<(), Error> {
    let (_, definition, inverse) =
        schema
            .resolve_relation(relation_name)
            .ok_or_else(|| Error::InvalidMutation {
                message: format!("unknown relation '{relation_name}'"),
            })?;
    let (source_flavour, target_flavour) = if inverse {
        (target_flavour, source_flavour)
    } else {
        (source_flavour, target_flavour)
    };
    if !definition
        .source
        .iter()
        .any(|flavour| flavour == source_flavour)
    {
        return invalid(format!(
            "relation '{relation_name}' does not allow source flavour '{}'",
            source_flavour
        ));
    }
    if !definition
        .target
        .iter()
        .any(|flavour| flavour == target_flavour)
    {
        return invalid(format!(
            "relation '{relation_name}' does not allow target flavour '{}'",
            target_flavour
        ));
    }
    if definition.same_flavour && source_flavour != target_flavour {
        return invalid(format!(
            "relation '{relation_name}' requires matching source and target flavours"
        ));
    }

    Ok(())
}

fn validate_external_relation(
    schema: &Schema,
    relation_name: &str,
    source_flavour: &str,
    target: &str,
) -> Result<(), Error> {
    let Some((_, definition, inverse)) = schema.resolve_relation(relation_name) else {
        return invalid(format!("unknown relation '{relation_name}'"));
    };
    let Some(address) = crate::external::address(target) else {
        return invalid("expected explicit external: target");
    };
    if inverse
        || !definition.external
        || !definition.source.iter().any(|f| f == source_flavour)
        || !crate::external::valid_address(address)
    {
        return invalid(format!(
            "relation '{relation_name}' does not allow this external target"
        ));
    }
    Ok(())
}

fn validate_new_item(
    corpus: &Corpus,
    schema: &Schema,
    request: &ItemCreationRequest,
) -> Result<(), Error> {
    let flavour =
        schema
            .flavours()
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

fn ensure_unambiguous_item_identities(corpus: &Corpus, operation: &str) -> Result<(), Error> {
    let mut ids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
    let mut mids: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();

    for item in corpus.items() {
        ids.entry(item.id()).or_default().push(item);
        let mid_entries = item
            .metadata()
            .iter()
            .filter(|entry| entry.key() == "mid")
            .collect::<Vec<_>>();
        let [mid_entry] = mid_entries.as_slice() else {
            return invalid(format!(
                "cannot {operation} while item '{}' does not have exactly one MID; run project validate",
                item.id()
            ));
        };
        if !crate::is_mid(mid_entry.value()) {
            return invalid(format!(
                "cannot {operation} while item '{}' has invalid MID '{}'; run project validate",
                item.id(),
                mid_entry.value()
            ));
        }
        mids.entry(mid_entry.value()).or_default().push(item);
    }

    if let Some((id, _)) = ids.iter().find(|(_, items)| items.len() > 1) {
        return invalid(format!(
            "cannot {operation} while item ID '{id}' is ambiguous; run project validate"
        ));
    }
    if let Some((mid, _)) = mids.iter().find(|(_, items)| items.len() > 1) {
        return invalid(format!(
            "cannot {operation} while item MID '{mid}' is ambiguous; run project validate"
        ));
    }

    Ok(())
}

fn resolve_document_path(engine: &crate::SourceEngine, requested: &Path) -> Result<PathBuf, Error> {
    let path = requested.to_str().ok_or_else(|| Error::InvalidMutation {
        message: "destination path must be UTF-8".into(),
    })?;
    let relative = engine
        .normalize_path(path)
        .map_err(|error| Error::InvalidMutation {
            message: error.to_string(),
        })?;
    if !relative.ends_with(".mara.md") {
        return invalid("destination file must be named '*.mara.md'");
    }
    Ok(PathBuf::from(relative))
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
    for edge in &request.relations {
        source.push_str(&format!("{newline}:{}: {}", edge.relation, edge.target));
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
    if !after.is_empty() && !block.ends_with('\n') {
        candidate.push_str(newline);
    }
    if !after.is_empty() && !after.starts_with('\n') && !after.starts_with("\r\n") {
        candidate.push_str(newline);
    }
    candidate.push_str(after);
    candidate
}

// @mara implements REQ-RELATION-MUTATION
// @mara implements DES-RELATION-MUTATION
pub(super) fn mutate_semantic_relation(
    context: &ChangePlanner<'_>,
    params: &crate::RelationParams,
    add: bool,
    occurrence: Option<&str>,
) -> Result<ChangePlan<crate::RelationMutationResult>, crate::RelationError> {
    use crate::{RelationAction, RelationError};
    if params.source.starts_with("code:") {
        return Err(RelationError::new(
            "unsupported_mutation",
            "relation add/remove cannot modify code source files; use the declared inverse with an item source, or edit the code comment directly",
        ));
    }
    let project = &context.project;
    let schema = &context.schema;
    let corpus = &context.corpus;
    ensure_unambiguous_item_identities(corpus, "mutate relations")?;
    let edge = crate::relations::resolve_edge(
        corpus,
        schema,
        &params.source,
        &params.relation,
        &params.target,
    )?;
    let code_edge = matches!(edge.source, crate::RelationEndpoint::Code { .. });
    let authored = crate::relations::occurrences(project, corpus, schema, &edge)?;
    let editable = authored
        .iter()
        .filter(|entry| !code_edge || matches!(entry.author, crate::RelationEndpoint::Item { .. }))
        .collect::<Vec<_>>();
    if add && !authored.is_empty() {
        let description = if code_edge {
            "code-to-item edge already exists"
        } else {
            "item already has relation"
        };
        return Err(RelationError::new(
            "relation_exists",
            format!(
                "{description}; inspect with relation get {} {} {} ({} occurrences)",
                params.source,
                params.relation,
                params.target,
                authored.len()
            ),
        )
        .on_edge(&edge, authored.len()));
    }
    // Validate selectors even when their previous edge no longer exists.
    if let Some(token) = occurrence {
        let snapshot = crate::query::page::fingerprint(corpus, schema, &"relation-occurrences-v1")?;
        let prefix = format!("occ-1-{snapshot}-");
        if !token
            .strip_prefix(&prefix)
            .is_some_and(|tail| tail.len() == 16 && tail.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(RelationError::new(
                "stale_occurrence",
                "invalid or stale occurrence; inspect the relation again",
            )
            .on_edge(&edge, authored.len()));
        }
        if !authored.iter().any(|entry| entry.reference == token) {
            return Err(RelationError::new(
                "occurrence_mismatch",
                "occurrence does not belong to the requested edge",
            )
            .on_edge(&edge, authored.len()));
        }
        if code_edge && !editable.iter().any(|entry| entry.reference == token) {
            return Err(RelationError::new(
                "unsupported_mutation",
                "selected occurrence is a code comment; relation commands do not modify code source files",
            )
            .on_edge(&edge, authored.len()));
        }
    }
    if !add && editable.is_empty() {
        if code_edge && !authored.is_empty() {
            return Err(RelationError::new(
                "unsupported_mutation",
                "no item-authored assertion remains; edit the code comment directly to remove this edge",
            )
            .on_edge(&edge, authored.len()));
        }
        return Err(
            RelationError::new("relation_not_found", "relation does not exist").on_edge(&edge, 0),
        );
    }
    let mut candidates = BTreeMap::new();
    let changed;
    if add {
        let source = resolve_item(corpus, &params.source, "source")?;
        let target = if code_edge || crate::external::address(&params.target).is_some() {
            None
        } else {
            Some(resolve_item(corpus, &params.target, "target")?)
        };
        let document = corpus
            .documents()
            .iter()
            .find(|d| d.path() == source.source().path())
            .unwrap();
        let insertion = source
            .metadata()
            .last()
            .expect("title metadata")
            .source()
            .span()
            .end_byte();
        let mut candidate = document.source().to_owned();
        candidate.insert_str(
            insertion,
            &format!(
                "{}:{}: {}",
                newline_style(document.source()),
                params.relation,
                target.map_or(params.target.as_str(), |item| item.id())
            ),
        );
        candidates.insert(document.path().to_path_buf(), candidate);
        changed = 1;
    } else {
        let selected = editable
            .iter()
            .filter(|entry| occurrence.is_none_or(|token| token == entry.reference))
            .collect::<Vec<_>>();
        changed = selected.len();
        for entry in selected.into_iter().rev() {
            let path = entry.source.path();
            let candidate = candidates.entry(path.to_path_buf()).or_insert_with(|| {
                corpus
                    .documents()
                    .iter()
                    .find(|d| d.path() == path)
                    .unwrap()
                    .source()
                    .to_owned()
            });
            if entry.kind == "inline" {
                candidate.replace_range(
                    entry.source.start_byte()..entry.source.end_byte(),
                    &if entry.target.starts_with("code:") {
                        entry.target.clone()
                    } else if let Some(address) = crate::external::address(&entry.target) {
                        format!("<{address}>")
                    } else {
                        format!("[[{}]]", entry.target)
                    },
                );
            } else {
                let end = full_line_end(candidate, entry.source.end_byte());
                candidate.replace_range(entry.source.start_byte()..end, "");
            }
        }
    }
    let projected = corpus.with_replacements(&candidates, schema)?;
    references::preflight(corpus, &projected, None, None)?;
    let remaining = if add {
        authored.len() + 1
    } else {
        authored.len() - changed
    };
    Ok(context.finish(
        candidates,
        crate::RelationMutationResult {
            format_version: 1,
            action: if add {
                RelationAction::Added
            } else {
                RelationAction::Removed
            },
            scope: if occurrence.is_some() {
                "occurrence"
            } else if code_edge {
                "item"
            } else {
                "relationship"
            }
            .into(),
            edge,
            changed_occurrences: changed,
            remaining_occurrences: remaining,
            edge_exists: remaining > 0,
        },
    )?)
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

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ItemMove {
    pub id: String,
    pub mid: String,
    pub old_location: ItemLocation,
    pub new_location: ItemLocation,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ItemLocation {
    pub path: PathBuf,
    pub line: usize,
}

// @mara implements REQ-ITEM-MOVEMENT
// @mara implements DES-ITEM-MOVEMENT
pub(super) fn move_item(
    context: &ChangePlanner<'_>,
    reference: &str,
    file: &Path,
    line: Option<usize>,
) -> Result<ChangePlan<ItemMove>, Error> {
    let schema = &context.schema;
    let corpus = &context.corpus;
    require_valid_move_corpus(corpus, schema)?;
    let item =
        crate::query::resolve_item(corpus, reference).map_err(|error| Error::InvalidMutation {
            message: error.to_string(),
        })?;
    let source_path = item.source().path();
    let source = corpus
        .documents()
        .iter()
        .find(|document| document.path() == source_path)
        .expect("item belongs to document");
    let destination_path = resolve_document_path(context.engine, file)?;
    if !context.engine.document_is_selected(
        &context.project,
        destination_path.to_str().expect("normalized path"),
    )? {
        return invalid(format!(
            "destination file '{}' is excluded by project content discovery",
            destination_path.display()
        ));
    }
    let destination_source = context
        .engine
        .files()
        .get(destination_path.to_str().expect("normalized path"))
        .cloned()
        .unwrap_or_default();
    let destination = parse_document_source(&destination_path, &destination_source, schema)?;
    let position = insertion_position(&destination_source, line)?;
    if let Some(containing) = destination.items().iter().find(|item| {
        let span = item.source().span();
        position > span.start_byte() && position < span.end_byte()
    }) {
        return invalid(format!(
            "insertion point is inside item '{}'",
            containing.id()
        ));
    }
    let span = item.source().span();
    let block = &source.source()[span.start_byte()..span.end_byte()];
    let mut removed = source.source().to_owned();
    removed.replace_range(span.start_byte()..span.end_byte(), "");
    let same_document = source_path == destination_path;
    let mut candidates = BTreeMap::new();
    if same_document {
        // Coordinates refer to the original document, as they do for creation.
        let candidate = if position == span.start_byte() || position == span.end_byte() {
            source.source().to_owned()
        } else {
            let position = if position > span.end_byte() {
                position - block.len()
            } else {
                position
            };
            insert_block(
                &removed,
                position,
                block,
                newline_style(&destination_source),
            )
        };
        candidates.insert(destination_path.clone(), candidate);
    } else {
        candidates.insert(source_path.to_path_buf(), removed);
        candidates.insert(
            destination_path.clone(),
            insert_block(
                &destination_source,
                position,
                block,
                newline_style(&destination_source),
            ),
        );
    }
    let projected = corpus.with_replacements(&candidates, schema)?;
    references::preflight(corpus, &projected, None, None)?;
    require_valid_move_corpus(&projected, schema)?;
    // Validity alone cannot detect a block hidden by Markdown context or changed mentions.
    if projected.items().count() != corpus.items().count() {
        return invalid("move changes item recognition in the document's Markdown context");
    }
    for original in corpus.items() {
        let Some(candidate) = projected
            .items()
            .find(|candidate| candidate.mid() == original.mid())
        else {
            return invalid("move would hide an existing item in Markdown context");
        };
        if candidate.id() != original.id()
            || candidate.body() != original.body()
            || candidate
                .metadata()
                .iter()
                .map(|entry| (entry.key(), entry.value()))
                .collect::<Vec<_>>()
                != original
                    .metadata()
                    .iter()
                    .map(|entry| (entry.key(), entry.value()))
                    .collect::<Vec<_>>()
            || candidate
                .mentions()
                .iter()
                .map(|mention| mention.target())
                .collect::<Vec<_>>()
                != original
                    .mentions()
                    .iter()
                    .map(|mention| mention.target())
                    .collect::<Vec<_>>()
        {
            return invalid("move would change an item's content or references");
        }
    }
    let moved = projected
        .items()
        .find(|candidate| candidate.mid() == item.mid())
        .expect("item preservation checked");
    if moved.source().path() != destination_path {
        return invalid("moved item did not resolve in the destination");
    }
    let result = ItemMove {
        id: item.id().to_owned(),
        mid: item.mid().expect("validated MID").to_owned(),
        old_location: ItemLocation {
            path: source_path.to_path_buf(),
            line: span.start_line(),
        },
        new_location: ItemLocation {
            path: destination_path.clone(),
            line: moved.source().span().start_line(),
        },
    };
    context.finish(candidates, result)
}

fn require_valid_move_corpus(corpus: &Corpus, schema: &Schema) -> Result<(), Error> {
    if let Some(diagnostic) = validate_corpus(corpus, schema).first() {
        return invalid(format!(
            "cannot move item while validation fails at {}:{}: {}",
            diagnostic.source().path().display(),
            diagnostic.source().span().start_line(),
            diagnostic.message()
        ));
    }
    Ok(())
}
