use std::{cmp::Ordering, collections::BTreeMap};

use mara_core::{
    AuthoredReference, Diagnostic, DiagnosticContext, DiagnosticItem, FieldDefinition,
    FieldDiagnosticCode, FieldType, FlavourDefinition, IdentityDiagnosticCode, IdentityIndex,
    IdentityRecord, ItemDiagnosticCode, NormalizedFieldValue, NormalizedItem, NormalizedNumber,
    NormalizedScalar, Provenanced, ReferenceOrigin, RelationDiagnosticCode, ResolvedReference,
    SchemaDocument, SourceSpan, sort_diagnostics,
};
use mara_markdown::{
    InlineReference, ParsedBlock, ParsedDocument, ParsedItem, ParsedMetadataEntry,
};
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCompilation {
    items: Vec<NormalizedItem>,
    narrative_references: Vec<ResolvedReference>,
    identity_index: IdentityIndex,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticCompilation {
    pub fn items(&self) -> &[NormalizedItem] {
        &self.items
    }

    pub fn narrative_references(&self) -> &[ResolvedReference] {
        &self.narrative_references
    }

    pub const fn identity_index(&self) -> &IdentityIndex {
        &self.identity_index
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<NormalizedItem>,
        Vec<ResolvedReference>,
        IdentityIndex,
        Vec<Diagnostic>,
    ) {
        (
            self.items,
            self.narrative_references,
            self.identity_index,
            self.diagnostics,
        )
    }
}

/// Compiles already parsed documents and a compiled schema without performing I/O.
pub fn compile_documents(
    schema: &SchemaDocument,
    documents: &[ParsedDocument],
) -> SemanticCompilation {
    let parsed_items = documents
        .iter()
        .flat_map(ParsedDocument::items)
        .collect::<Vec<_>>();
    let records = parsed_items
        .iter()
        .map(|item| identity_record(schema, item))
        .collect::<Vec<_>>();
    let index_build = IdentityIndex::build(&records);
    let (identity_index, mut diagnostics) = index_build.into_parts();
    diagnostics.extend(
        documents
            .iter()
            .flat_map(|document| document.diagnostics().iter().cloned()),
    );

    let mut items = parsed_items
        .iter()
        .filter_map(|item| normalize_item(schema, item, &mut diagnostics))
        .collect::<Vec<_>>();
    items.sort_by(compare_items);

    let identity = schema.identity().value().mid().value();
    for item in &mut items {
        let mut resolved = Vec::new();
        for reference in item.authored_references() {
            if external_uri_scheme(reference.target()).is_some() {
                continue;
            }
            match identity_index.resolve(reference, identity) {
                Ok(reference) => resolved.push(reference),
                Err(diagnostic) => diagnostics.push(*diagnostic),
            }
        }
        item.set_resolved_references(resolved);
    }

    let mut narrative_authored = narrative_references(documents);
    narrative_authored.sort_by(|left, right| compare_spans(left.source(), right.source()));
    let mut narrative_references = Vec::new();
    for reference in narrative_authored {
        if external_uri_scheme(reference.target()).is_some() {
            continue;
        }
        match identity_index.resolve(&reference, identity) {
            Ok(reference) => narrative_references.push(reference),
            Err(diagnostic) => diagnostics.push(*diagnostic),
        }
    }

    sort_diagnostics(&mut diagnostics);
    SemanticCompilation {
        items,
        narrative_references,
        identity_index,
        diagnostics,
    }
}

fn identity_record(schema: &SchemaDocument, item: &ParsedItem) -> IdentityRecord {
    let entries = item
        .metadata()
        .iter()
        .filter(|entry| entry.key() == "id")
        .collect::<Vec<_>>();
    let display_id = schema
        .flavours()
        .get(item.flavour())
        .filter(|_| entries.len() == 1)
        .and_then(|flavour| {
            let entry = entries[0];
            if entry.value().is_empty()
                || flavour
                    .display_id()
                    .value()
                    .pattern()
                    .is_some_and(|pattern| !whole_pattern(pattern.value()).is_match(entry.value()))
            {
                return None;
            }
            Some(Provenanced::new(
                entry.value().to_owned(),
                entry.source().clone(),
            ))
        });
    IdentityRecord::new(item.mid().clone(), display_id, item.header_source().clone())
}

fn normalize_item(
    schema: &SchemaDocument,
    item: &ParsedItem,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<NormalizedItem> {
    let Some(flavour) = schema.flavours().get(item.flavour()) else {
        diagnostics.push(
            Diagnostic::new(
                ItemDiagnosticCode::UnknownFlavour,
                format!(
                    "item flavour {:?} is not declared by the schema",
                    item.flavour()
                ),
                Some(item.header_source().clone()),
            )
            .with_item(DiagnosticItem::new(
                item.mid().clone(),
                parsed_display_id(item),
            ))
            .with_detail("flavour", item.flavour()),
        );
        return None;
    };

    let display_id = normalize_display_id(flavour, item, diagnostics);
    let title = normalize_title(flavour, item, display_id.as_ref(), diagnostics);
    let body = Provenanced::new(item.body_markdown().to_owned(), item.body_source().clone());
    if flavour.body().value().is_required() && item.body_markdown().trim().is_empty() {
        diagnostics.push(missing_value(
            item,
            display_id.as_ref(),
            "body",
            item.body_source(),
        ));
    }

    let authorable_relations = authorable_relations(schema, item.flavour());
    let mut fields = BTreeMap::new();
    let mut references = Vec::new();
    for (key, entries) in metadata_by_key(item) {
        if matches!(key.as_str(), "id" | "title") {
            continue;
        }
        if let Some(field) = flavour.fields().get(&key) {
            let values = normalize_field(item, display_id.as_ref(), field, &entries, diagnostics);
            fields.insert(key, values);
        } else if authorable_relations.contains_key(&key) {
            for entry in entries {
                references.push(AuthoredReference::new(
                    entry.value().to_owned(),
                    None,
                    Some(key.clone()),
                    item_origin(item, display_id.as_ref()),
                    entry.source().clone(),
                ));
            }
        } else {
            for entry in entries {
                diagnostics.push(
                    item_diagnostic(
                        ItemDiagnosticCode::UnknownKey,
                        format!("metadata key {key:?} is not declared for this flavour"),
                        item,
                        display_id.as_ref(),
                        entry.source(),
                    )
                    .with_context(DiagnosticContext::new(Some(key.clone()), None, None))
                    .with_detail("key", key.clone()),
                );
            }
        }
    }
    for field in flavour.fields().values() {
        if field.is_required() && !fields.contains_key(field.name()) {
            diagnostics.push(missing_value(
                item,
                display_id.as_ref(),
                field.name(),
                item.header_source(),
            ));
        }
    }

    references.extend(normalize_inline_item_references(
        item,
        display_id.as_ref(),
        &authorable_relations,
        diagnostics,
    ));
    references.sort_by(|left, right| compare_spans(left.source(), right.source()));

    Some(NormalizedItem::new(
        item.mid().clone(),
        item.flavour().to_owned(),
        display_id,
        title,
        body,
        fields,
        references,
        item.source().clone(),
        item.header_source().clone(),
    ))
}

fn metadata_by_key(item: &ParsedItem) -> BTreeMap<String, Vec<&ParsedMetadataEntry>> {
    let mut entries = BTreeMap::<String, Vec<&ParsedMetadataEntry>>::new();
    for entry in item.metadata() {
        entries
            .entry(entry.key().to_owned())
            .or_default()
            .push(entry);
    }
    entries
}

fn parsed_display_id(item: &ParsedItem) -> Option<String> {
    item.metadata()
        .iter()
        .find(|entry| entry.key() == "id" && !entry.value().is_empty())
        .map(|entry| entry.value().to_owned())
}

fn normalize_display_id(
    flavour: &FlavourDefinition,
    item: &ParsedItem,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Provenanced<String>> {
    let entries = item
        .metadata()
        .iter()
        .filter(|entry| entry.key() == "id")
        .collect::<Vec<_>>();
    repetition_diagnostics(item, None, "id", &entries, diagnostics);
    let display_id = entries.first().and_then(|entry| {
        (!entry.value().is_empty())
            .then(|| Provenanced::new(entry.value().to_owned(), entry.source().clone()))
    });
    if flavour.display_id().value().is_required() && display_id.is_none() {
        let source = entries
            .first()
            .map_or(item.header_source(), |entry| entry.source());
        diagnostics.push(missing_value(item, None, "id", source));
    }
    if let (Some(display_id), Some(pattern)) =
        (display_id.as_ref(), flavour.display_id().value().pattern())
        && !whole_pattern(pattern.value()).is_match(display_id.value())
    {
        diagnostics.push(
            item_diagnostic(
                IdentityDiagnosticCode::InvalidDisplayId,
                "display ID does not match its flavour pattern",
                item,
                Some(display_id),
                display_id.source(),
            )
            .with_detail("display_id", display_id.value().clone())
            .with_detail("pattern", pattern.value().clone()),
        );
    }
    display_id
}

fn normalize_title(
    flavour: &FlavourDefinition,
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Provenanced<String>> {
    let entries = item
        .metadata()
        .iter()
        .filter(|entry| entry.key() == "title")
        .collect::<Vec<_>>();
    repetition_diagnostics(item, display_id, "title", &entries, diagnostics);
    let title = entries
        .first()
        .map(|entry| Provenanced::new(entry.value().to_owned(), entry.source().clone()));
    if flavour.title().value().is_required()
        && title.as_ref().is_none_or(|title| title.value().is_empty())
    {
        let source = entries
            .first()
            .map_or(item.header_source(), |entry| entry.source());
        diagnostics.push(missing_value(item, display_id, "title", source));
    }
    title
}

fn normalize_field(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    definition: &FieldDefinition,
    entries: &[&ParsedMetadataEntry],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<NormalizedFieldValue> {
    if !definition.is_repeatable() {
        repetition_diagnostics(item, display_id, definition.name(), entries, diagnostics);
    }
    entries
        .iter()
        .filter_map(|entry| normalize_scalar(item, display_id, definition, entry, diagnostics))
        .collect()
}

fn normalize_scalar(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    definition: &FieldDefinition,
    entry: &ParsedMetadataEntry,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<NormalizedFieldValue> {
    let value = entry.value();
    let normalized = match definition.field_type().value() {
        FieldType::String => {
            if let Some(pattern) = definition.pattern()
                && !whole_pattern(pattern.value()).is_match(value)
            {
                diagnostics.push(
                    field_diagnostic(
                        FieldDiagnosticCode::PatternMismatch,
                        "string value does not match its field pattern",
                        item,
                        display_id,
                        definition.name(),
                        entry.source(),
                    )
                    .with_detail("pattern", pattern.value().clone())
                    .with_detail("value", value),
                );
                return None;
            }
            NormalizedScalar::String(value.to_owned())
        }
        FieldType::Integer => match parse_integer(value) {
            Some(value) => NormalizedScalar::Integer(value),
            None => {
                diagnostics.push(invalid_scalar(item, display_id, definition, entry));
                return None;
            }
        },
        FieldType::Number => match serde_json::from_str::<f64>(value)
            .ok()
            .and_then(NormalizedNumber::new)
        {
            Some(value) => NormalizedScalar::Number(value),
            None => {
                diagnostics.push(invalid_scalar(item, display_id, definition, entry));
                return None;
            }
        },
        FieldType::Boolean => match value {
            "true" => NormalizedScalar::Boolean(true),
            "false" => NormalizedScalar::Boolean(false),
            _ => {
                diagnostics.push(invalid_scalar(item, display_id, definition, entry));
                return None;
            }
        },
        FieldType::Enum => {
            let valid = definition.values().is_some_and(|values| {
                values
                    .value()
                    .iter()
                    .any(|candidate| candidate.value() == value)
            });
            if !valid {
                diagnostics.push(
                    field_diagnostic(
                        FieldDiagnosticCode::InvalidEnum,
                        "value is not one of the field's declared enum values",
                        item,
                        display_id,
                        definition.name(),
                        entry.source(),
                    )
                    .with_detail("value", value),
                );
                return None;
            }
            NormalizedScalar::Enum(value.to_owned())
        }
    };
    Some(Provenanced::new(normalized, entry.source().clone()))
}

fn invalid_scalar(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    definition: &FieldDefinition,
    entry: &ParsedMetadataEntry,
) -> Diagnostic {
    field_diagnostic(
        FieldDiagnosticCode::InvalidScalar,
        format!(
            "value cannot be converted to declared {} scalar",
            definition.field_type().value().as_str()
        ),
        item,
        display_id,
        definition.name(),
        entry.source(),
    )
    .with_detail("value", entry.value())
}

fn parse_integer(value: &str) -> Option<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || (digits.len() > 1 && digits.starts_with('0'))
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn repetition_diagnostics(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    field: &str,
    entries: &[&ParsedMetadataEntry],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in entries.iter().skip(1) {
        diagnostics.push(field_diagnostic(
            FieldDiagnosticCode::Repetition,
            format!("non-repeatable field {field:?} occurs more than once"),
            item,
            display_id,
            field,
            entry.source(),
        ));
    }
}

fn authorable_relations(schema: &SchemaDocument, flavour: &str) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let Some(relations) = schema.relations() else {
        return names;
    };
    for relation in relations.definitions().values() {
        if relation
            .source()
            .value()
            .flavours()
            .value()
            .iter()
            .any(|candidate| candidate.value() == flavour)
        {
            names.insert(relation.name().to_owned(), relation.name().to_owned());
        }
        if relation.permits_inverse_authoring()
            && relation
                .target()
                .value()
                .flavours()
                .is_some_and(|flavours| {
                    flavours
                        .value()
                        .iter()
                        .any(|candidate| candidate.value() == flavour)
                })
            && let Some(inverse) = relation.inverse()
        {
            names.insert(inverse.value().clone(), relation.name().to_owned());
        }
    }
    names
}

fn normalize_inline_item_references(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    authorable_relations: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<AuthoredReference> {
    item.references()
        .iter()
        .filter_map(|reference| {
            let (relation, target) = split_relation_target(reference.target());
            if let Some(relation) = relation
                && !authorable_relations.contains_key(relation)
            {
                diagnostics.push(
                    item_diagnostic(
                        RelationDiagnosticCode::Unknown,
                        format!("relation {relation:?} is not authorable for this flavour"),
                        item,
                        display_id,
                        reference.source(),
                    )
                    .with_context(DiagnosticContext::new(
                        None,
                        Some(relation.to_owned()),
                        Some(target.to_owned()),
                    ))
                    .with_detail("relation", relation),
                );
                return None;
            }
            Some(AuthoredReference::new(
                target.to_owned(),
                reference.label().map(str::to_owned),
                relation.map(str::to_owned),
                item_origin(item, display_id),
                reference.source().clone(),
            ))
        })
        .collect()
}

fn split_relation_target(target: &str) -> (Option<&str>, &str) {
    if external_uri_scheme(target).is_some() {
        return (None, target);
    }
    let Some((qualifier, target)) = target.split_once(':') else {
        return (None, target);
    };
    if valid_snake_name(qualifier) && !target.is_empty() {
        (Some(qualifier), target)
    } else {
        (None, target)
    }
}

fn narrative_references(documents: &[ParsedDocument]) -> Vec<AuthoredReference> {
    documents
        .iter()
        .flat_map(|document| document.blocks())
        .filter_map(ParsedBlock::as_markdown)
        .flat_map(|markdown| {
            markdown.references().iter().map(move |reference| {
                authored_narrative_reference(reference, markdown.source().clone())
            })
        })
        .collect()
}

fn authored_narrative_reference(
    reference: &InlineReference,
    narrative_source: SourceSpan,
) -> AuthoredReference {
    AuthoredReference::new(
        reference.target().to_owned(),
        reference.label().map(str::to_owned),
        None,
        ReferenceOrigin::Narrative(narrative_source),
        reference.source().clone(),
    )
}

fn item_origin(item: &ParsedItem, display_id: Option<&Provenanced<String>>) -> ReferenceOrigin {
    ReferenceOrigin::Item {
        mid: item.mid().clone(),
        display_id: display_id.map(|id| id.value().clone()),
    }
}

fn external_uri_scheme(target: &str) -> Option<&str> {
    let (scheme, _) = target.split_once("://")?;
    (!scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')))
    .then_some(scheme)
}

fn valid_snake_name(value: &str) -> bool {
    let mut segments = value.split('_');
    let Some(first) = segments.next() else {
        return false;
    };
    valid_name_segment(first, true) && segments.all(|segment| valid_name_segment(segment, false))
}

fn valid_name_segment(segment: &str, require_letter_first: bool) -> bool {
    !segment.is_empty()
        && (!require_letter_first || segment.as_bytes()[0].is_ascii_lowercase())
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn whole_pattern(pattern: &str) -> Regex {
    Regex::new(&format!(r"\A(?:{pattern})\z"))
        .expect("compiled schema contains only validated regular expressions")
}

fn missing_value(
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    field: &str,
    source: &SourceSpan,
) -> Diagnostic {
    item_diagnostic(
        ItemDiagnosticCode::MissingValue,
        format!("required value {field:?} is missing or empty"),
        item,
        display_id,
        source,
    )
    .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
    .with_detail("field", field)
}

fn field_diagnostic(
    code: FieldDiagnosticCode,
    message: impl Into<String>,
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    field: &str,
    source: &SourceSpan,
) -> Diagnostic {
    item_diagnostic(code, message, item, display_id, source)
        .with_context(DiagnosticContext::new(Some(field.to_owned()), None, None))
        .with_detail("field", field)
}

fn item_diagnostic(
    code: impl Into<mara_core::DiagnosticCode>,
    message: impl Into<String>,
    item: &ParsedItem,
    display_id: Option<&Provenanced<String>>,
    source: &SourceSpan,
) -> Diagnostic {
    Diagnostic::new(code, message, Some(source.clone())).with_item(DiagnosticItem::new(
        item.mid().clone(),
        display_id.map(|id| id.value().clone()),
    ))
}

fn compare_items(left: &NormalizedItem, right: &NormalizedItem) -> Ordering {
    left.mid()
        .cmp(right.mid())
        .then_with(|| compare_spans(left.header_source(), right.header_source()))
}

fn compare_spans(left: &SourceSpan, right: &SourceSpan) -> Ordering {
    left.path()
        .as_bytes()
        .cmp(right.path().as_bytes())
        .then_with(|| left.start_byte().cmp(&right.start_byte()))
        .then_with(|| left.end_byte().cmp(&right.end_byte()))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use mara_core::{
        DisplayIdDefinition, FlavourDefinitions, FlavourGuidance, IdentityConfiguration, MidFormat,
        MidIdentity, RelationDefinition, RelationDefinitions, RelationSourceEndpoint,
        RelationTargetEndpoint, RequiredBuiltInDefinition, SchemaField, SchemaIdentity,
        SchemaValue, SourceDocument, SourceIndex, SourceText,
    };
    use mara_markdown::parse_document;

    fn span() -> SourceSpan {
        SourceIndex::try_new("schema.yaml", "x")
            .unwrap()
            .document_span()
    }

    fn field<T>(value: T) -> SchemaField<T> {
        SchemaField::new(span(), span(), value)
    }

    fn value<T>(value: T) -> SchemaValue<T> {
        SchemaValue::new(span(), value)
    }

    fn schema() -> SchemaDocument {
        let status = FieldDefinition::new(
            "custom_state".to_owned(),
            span(),
            span(),
            field(FieldType::Enum),
            Some(field(true)),
            None,
            Some(field(vec![
                value("approved".to_owned()),
                value("draft".to_owned()),
            ])),
            None,
        );
        let score = FieldDefinition::new(
            "score".to_owned(),
            span(),
            span(),
            field(FieldType::Number),
            None,
            None,
            None,
            None,
        );
        let tags = FieldDefinition::new(
            "tag".to_owned(),
            span(),
            span(),
            field(FieldType::String),
            None,
            Some(field(true)),
            None,
            None,
        );
        let fields = BTreeMap::from([
            (status.name().to_owned(), status),
            (score.name().to_owned(), score),
            (tags.name().to_owned(), tags),
        ]);
        let guidance = FlavourGuidance::new(
            field(vec![value("use".to_owned())]),
            field(vec![value("avoid".to_owned())]),
            None,
            BTreeMap::new(),
        );
        let req = FlavourDefinition::new(
            "req".to_owned(),
            span(),
            span(),
            field("Requirement".to_owned()),
            field("A requirement".to_owned()),
            field(guidance),
            field(DisplayIdDefinition::new(
                Some(field(true)),
                Some(field("REQ-[A-Z]+".to_owned())),
            )),
            field(RequiredBuiltInDefinition::new(Some(field(true)))),
            field(RequiredBuiltInDefinition::new(None)),
            None,
            fields,
        );
        let relation = RelationDefinition::new(
            "traces".to_owned(),
            span(),
            span(),
            field(RelationSourceEndpoint::new(
                field(vec![value("req".to_owned())]),
                None,
            )),
            field(RelationTargetEndpoint::new(
                Some(field(vec![value("req".to_owned())])),
                None,
            )),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let mid = MidIdentity::new(field(MidFormat::Ulid), field("m_".to_owned()));
        SchemaDocument::new(
            span(),
            field(1),
            field(SchemaIdentity::new(
                field("test".to_owned()),
                field("1.0.0".to_owned()),
            )),
            field(IdentityConfiguration::new(field(mid))),
            FlavourDefinitions::new(span(), span(), BTreeMap::from([("req".to_owned(), req)])),
            Some(RelationDefinitions::new(
                span(),
                span(),
                BTreeMap::from([("traces".to_owned(), relation)]),
            )),
            None,
        )
    }

    fn document(path: &str, source: &str, schema: &SchemaDocument) -> ParsedDocument {
        let source = SourceDocument::try_new(path, SourceText::new(source.to_owned())).unwrap();
        parse_document(source, schema.identity().value().mid().value())
    }

    fn first_source() -> &'static str {
        ":::req m_00000000000000000000000001\n\
:id: REQ-ONE\n\
:title: First\n\
:custom_state: approved\n\
:score: 1.5\n\
:tag: alpha\n\
:tag: beta\n\
\n\
See [[REQ-TWO]] and [[m_00000000000000000000000002]].\n\
:::\n"
    }

    fn second_source() -> &'static str {
        "Narrative [[REQ-ONE]].\n\
\n\
:::req m_00000000000000000000000002\n\
:id: REQ-TWO\n\
:title: Second\n\
:custom_state: draft\n\
:traces: REQ-ONE\n\
\n\
Body.\n\
:::\n"
    }

    #[test]
    fn compilation_is_deterministic_and_preserves_schema_owned_values_and_provenance() {
        let schema = schema();
        let first = document("z.mara.md", first_source(), &schema);
        let second = document("a.mara.md", second_source(), &schema);
        let forward = compile_documents(&schema, &[first.clone(), second.clone()]);
        let reverse = compile_documents(&schema, &[second, first]);

        assert_eq!(forward, reverse);
        assert!(forward.is_valid(), "{:?}", forward.diagnostics());
        assert_eq!(
            forward
                .items()
                .iter()
                .map(|item| item.mid().as_str())
                .collect::<Vec<_>>(),
            [
                "m_00000000000000000000000001",
                "m_00000000000000000000000002",
            ]
        );
        let first = &forward.items()[0];
        assert_eq!(
            first.fields()["custom_state"][0].value(),
            &NormalizedScalar::Enum("approved".to_owned())
        );
        assert_eq!(first.fields()["tag"].len(), 2);
        assert_eq!(first.resolved_references().len(), 2);
        assert!(
            first
                .resolved_references()
                .iter()
                .all(|reference| reference.target().as_str() == "m_00000000000000000000000002")
        );
        assert_eq!(forward.narrative_references().len(), 1);
        assert_eq!(
            forward.narrative_references()[0].authored().source().path(),
            "a.mara.md"
        );
    }

    #[test]
    fn classification_uses_only_the_selected_flavour_schema() {
        let schema = schema();
        let parsed = document(
            "item.mara.md",
            ":::req m_00000000000000000000000001\n\
:id: REQ-ONE\n\
:title: First\n\
:custom_state: approved\n\
:status: approved\n\
:traces: REQ-MISSING\n\
\n\
Body.\n\
:::\n",
            &schema,
        );
        let result = compile_documents(&schema, &[parsed]);
        let codes = result
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("item.unknown_key"));
        assert!(codes.contains("reference.unresolved"));
        assert!(!codes.contains("relation.unknown"));
        assert!(result.items()[0].fields().contains_key("custom_state"));
        assert_eq!(
            result.items()[0].authored_references()[0].relation(),
            Some("traces")
        );
    }

    #[test]
    fn scalar_required_repetition_and_pattern_failures_are_structured() {
        let schema = schema();
        let parsed = document(
            "invalid.mara.md",
            ":::req m_00000000000000000000000001\n\
:id: wrong\n\
:title:\n\
:title: duplicate\n\
:custom_state: unknown\n\
:score: NaN\n\
\n\
Body.\n\
:::\n",
            &schema,
        );
        let result = compile_documents(&schema, &[parsed]);
        let codes = result
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("identity.invalid_display_id"));
        assert!(codes.contains("item.missing_value"));
        assert!(codes.contains("field.repetition"));
        assert!(codes.contains("field.invalid_enum"));
        assert!(codes.contains("field.invalid_scalar"));
        assert!(
            result
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.primary().is_some())
        );
    }

    #[test]
    fn duplicate_and_unresolved_identity_diagnostics_keep_exact_occurrences() {
        let schema = schema();
        let first = document(
            "a.mara.md",
            ":::req m_00000000000000000000000001\n\
:id: REQ-DUP\n\
:title: First\n\
:custom_state: approved\n\
\n\
[[MISSING]]\n\
:::\n",
            &schema,
        );
        let second = document(
            "b.mara.md",
            ":::req m_00000000000000000000000002\n\
:id: REQ-DUP\n\
:title: Second\n\
:custom_state: approved\n\
\n\
[[REQ-DUP]]\n\
:::\n",
            &schema,
        );
        let result = compile_documents(&schema, &[second, first]);
        let duplicate = result
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == "identity.duplicate_display_id")
            .unwrap();
        assert_eq!(duplicate.related().len(), 2);
        assert_eq!(duplicate.related()[0].span().path(), "a.mara.md");
        let unresolved = result
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == "reference.unresolved")
            .unwrap();
        assert_eq!(unresolved.primary().unwrap().path(), "a.mara.md");
        let ambiguous = result
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == "reference.ambiguous")
            .unwrap();
        assert_eq!(ambiguous.primary().unwrap().path(), "b.mara.md");
        assert_eq!(ambiguous.related().len(), 2);
    }
}
