//! Named factual SELECT execution. Hosts supply sources; policies do not gate facts.
mod composition;
mod facts;
pub(crate) mod native;
mod signature;
pub use crate::factual_graph::{Edge, Endpoint, Occurrence, Support};
use crate::factual_graph::{PreparedGraph, hash, prepare};
use crate::{ItemSource, SourceEngine};
pub use native::NativeCodeInputs;
use oxigraph::model::{Literal, NamedNode, Term};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Item,
    Code,
    External,
    Iri,
    Text,
    Integer,
    Number,
    Boolean,
    Edge,
}

/// Fact sources a named definition explicitly requires from its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    Documents,
    Code,
    External,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub r#type: ValueType,
    #[serde(default)]
    pub flavours: Vec<String>,
    #[serde(default)]
    pub optional: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[schemars(transform = schemars::transform::RecursiveTransform(parameter_guidance))]
pub enum ParameterValue {
    Text(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    Item(String),
    Iri(String),
}
fn parameter_guidance(schema: &mut schemars::Schema) {
    if let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (key, description) in [
            ("type", "Exact parameter type; no implicit coercion."),
            (
                "value",
                "Typed JSON value; item references and IRIs are exact.",
            ),
        ] {
            if let Some(property) = properties
                .get_mut(key)
                .and_then(serde_json::Value::as_object_mut)
            {
                property.insert(
                    "description".into(),
                    serde_json::Value::String(description.into()),
                );
            }
        }
    }
}
impl ParameterValue {
    /// Typed result value for facts, mappings and derived projections.
    pub(crate) fn typed(&self, corpus: &crate::Corpus) -> Result<Value, Error> {
        Ok(match self {
            Self::Text(value) => Value::Text {
                value: value.clone(),
                language: None,
            },
            Self::Integer(value) => Value::Integer { value: *value },
            Self::Number(value) if value.is_finite() => Value::Number {
                value: if *value == 0.0 {
                    "0".into()
                } else {
                    value.to_string()
                },
            },
            Self::Number(_) => return Err(Error::argument("finite numbers only")),
            Self::Boolean(value) => Value::Boolean { value: *value },
            Self::Item(id) => {
                let item = crate::resolve_item(corpus, id)
                    .map_err(|error| Error::argument(error.to_string()))?;
                Value::Item {
                    endpoint: Endpoint::item(item),
                    source: ItemSource::from(item.source()),
                }
            }
            Self::Iri(value) => {
                NamedNode::new(value).map_err(|_| Error::argument("invalid IRI value"))?;
                Value::Iri {
                    value: value.clone(),
                }
            }
        })
    }

    pub(crate) fn kind(&self) -> ValueType {
        match self {
            Self::Text(_) => ValueType::Text,
            Self::Integer(_) => ValueType::Integer,
            Self::Number(_) => ValueType::Number,
            Self::Boolean(_) => ValueType::Boolean,
            Self::Item(_) => ValueType::Item,
            Self::Iri(_) => ValueType::Iri,
        }
    }
    fn term(&self, corpus: &crate::Corpus) -> Result<Term, Error> {
        use oxrdf::vocab::xsd;
        Ok(match self {
            Self::Text(s) => Literal::new_simple_literal(s).into(),
            Self::Integer(n) => Literal::new_typed_literal(n.to_string(), xsd::INTEGER).into(),
            Self::Number(n) if n.is_finite() => Literal::new_typed_literal(
                if *n == 0.0 { "0".into() } else { n.to_string() },
                xsd::DOUBLE,
            )
            .into(),
            Self::Number(_) => return Err(Error::argument("parameters require finite numbers")),
            Self::Boolean(b) => Literal::new_typed_literal(b.to_string(), xsd::BOOLEAN).into(),
            Self::Item(id) => NamedNode::new(
                Endpoint::item(
                    crate::resolve_item(corpus, id).map_err(|e| Error::argument(e.to_string()))?,
                )
                .iri(),
            )
            .expect("item IRI")
            .into(),
            Self::Iri(iri) => NamedNode::new(iri)
                .map_err(|_| Error::argument("invalid IRI parameter"))?
                .into(),
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub r#type: ValueType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<ParameterValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub select: String,
    /// Required host inputs are declarations, never inferred from query variables.
    pub required_fact_sources: Vec<FactSource>,
    /// Signature only, not a root selector. Caller selection remains explicit.
    pub root_flavours: Vec<String>,
    #[serde(deserialize_with = "unique_map")]
    pub columns: BTreeMap<String, Column>,
    #[serde(default)]
    pub support_columns: Vec<String>,
    /// Projected dependency fact provenance; excluded from tuple identity like edge support.
    #[serde(default)]
    pub facts_columns: Vec<String>,
    /// Extra projected item variables retained as contributing source support.
    #[serde(default)]
    pub source_columns: Vec<String>,
    #[serde(default, deserialize_with = "unique_map")]
    pub parameters: BTreeMap<String, Parameter>,
    /// Explicit child queries materialized as relations for this standard SELECT.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Host-supplied fact snapshots overlaid onto returned tuple rows (definition format 1).
    #[serde(default)]
    pub external_facts: Vec<ExternalFact>,
    /// Caller/project-supplied typed lookup mappings applied to tuple rows (definition format 1).
    #[serde(default)]
    pub lookups: Vec<Lookup>,
    /// Definition-selected precedence aggregation over tuple rows (definition format 1).
    #[serde(default)]
    pub aggregate: Option<Aggregate>,
}

/// One declared external fact input overlaid onto returned rows by declared key columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalFact {
    /// Request snapshot binding name; a declared input that is not supplied is unavailable.
    pub input: String,
    /// Tuple columns (in order) that identify one supplied record.
    pub keys: Vec<String>,
    /// Record field name to output tuple column.
    #[serde(deserialize_with = "unique_map")]
    pub fields: BTreeMap<String, String>,
    /// Output column to typed value used only for a covered key with no record.
    #[serde(deserialize_with = "unique_map")]
    pub missing: BTreeMap<String, ParameterValue>,
}

/// A typed lookup from declared key columns to one output column.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lookup {
    /// Request-supplied mapping name; a declared mapping that is not supplied is unavailable.
    pub mapping: String,
    /// Tuple columns supplying the lookup key, in order.
    pub keys: Vec<String>,
    /// Declared output column replaced by the mapped value.
    pub output: String,
}

/// Definition-selected precedence aggregation. No built-in state or risk ordering exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregate {
    /// Tuple columns grouped by exact typed value.
    #[serde(default)]
    pub group_by: Vec<String>,
    /// Column whose typed value is ranked by `precedence`.
    pub rank: String,
    /// Ordered typed values, highest precedence first.
    pub precedence: Vec<ParameterValue>,
    /// Member columns retained in the selected aggregate binding.
    #[serde(default)]
    pub retain: Vec<String>,
    /// Integer output column carrying the number of grouped members.
    pub count: String,
    /// Declared rank value for a selected root with no grouped members.
    pub empty: ParameterValue,
}

/// One host-supplied typed fact snapshot with an explicit complete-coverage claim.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactSnapshot {
    /// Declared input name matched against the definition's external fact input.
    pub input: String,
    /// Opaque host label; full content is bound into the request input identity.
    pub identity: String,
    /// Key maps for which this snapshot claims complete coverage.
    #[serde(default)]
    pub coverage: Vec<BTreeMap<String, ParameterValue>>,
    /// Typed records supplied by the host.
    #[serde(default)]
    pub records: Vec<FactRecord>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactRecord {
    /// Declared key columns to opaque typed values.
    #[serde(deserialize_with = "unique_map")]
    pub keys: BTreeMap<String, ParameterValue>,
    /// Declared record field names to typed values.
    #[serde(deserialize_with = "unique_map")]
    pub fields: BTreeMap<String, ParameterValue>,
}
/// One caller/project-supplied typed lookup mapping bound into input identity.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypedMapping {
    /// Request binding name matched against a definition's declared lookup mapping.
    pub mapping: String,
    /// Typed key-to-value entries; duplicate typed keys are rejected.
    pub entries: Vec<MappingEntry>,
    /// Explicit fallback used only for an absent key; no implicit zero/safe value exists.
    #[serde(default)]
    pub fallback: Option<ParameterValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MappingEntry {
    /// Declared lookup key columns to exact typed values.
    #[serde(deserialize_with = "unique_map")]
    pub keys: BTreeMap<String, ParameterValue>,
    /// Typed value returned for this exact key.
    pub value: ParameterValue,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    /// Local relation alias used by `urn:mara:query:input:<name>` predicates.
    pub name: String,
    /// Exact identity of another definition in the explicitly loaded set.
    pub definition: String,
    /// The only supported child root binding is the caller-selected `root`.
    pub root: String,
    /// Child parameter name to parent parameter name; no implicit inheritance.
    #[serde(default, deserialize_with = "unique_map")]
    pub parameters: BTreeMap<String, String>,
    /// Explicitly exposed child binding/support names consumed by parent SPARQL.
    pub projections: DependencyProjection,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyProjection {
    /// Child tuple column name to exposed relation-column alias.
    #[serde(default, deserialize_with = "unique_map")]
    pub columns: BTreeMap<String, String>,
    /// Optional relation-column alias for explicitly projected child contributors.
    #[serde(default)]
    pub contributors: Option<String>,
    /// Optional relation-column alias for explicitly projected child edge support.
    #[serde(default)]
    pub support: Option<String>,
    /// Optional relation-column alias for explicitly projected child fact provenance.
    #[serde(default)]
    pub facts: Option<String>,
}
fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a map with unique names")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((name, value)) = map.next_entry::<String, T>()? {
                if values.insert(name.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate declaration {name}"
                    )));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionSet {
    pub format_version: u32,
    pub definitions: Vec<Definition>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldSelection {
    /// Exact schema-declared field key.
    pub key: String,
    /// Exact field text; empty text remains an explicit filter value.
    pub value: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Explicit all roots; exclusive of every ID/filter category.
    #[serde(default)]
    pub all: bool,
    /// Exact item IDs/MIDs; aliases select one root, unknown references fail.
    #[serde(default)]
    pub ids: Vec<String>,
    /// Exact schema flavours, OR within this category.
    #[serde(default)]
    pub flavours: Vec<String>,
    /// Exact field filters, OR within one key and AND across keys/categories.
    #[serde(default)]
    pub fields: Vec<FieldSelection>,
    /// Exact project-relative document paths or directory subtrees, OR within this category.
    #[serde(default)]
    pub paths: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindingParameter {
    /// Exact declared parameter name; duplicates are invalid.
    pub name: String,
    /// Explicit typed parameter value; no coercion or command resolution.
    pub value: ParameterValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub definition_files: Vec<String>,
    pub definition: String,
    pub selection: Selection,
    #[serde(default)]
    pub parameters: Vec<BindingParameter>,
    pub max_rows: u32,
    pub limit: usize,
    #[serde(default)]
    pub cursor: Option<String>,
    /// Explicit host-supplied snapshots; never loaded from the network or persisted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fact_snapshots: Vec<FactSnapshot>,
    /// Explicit caller/project policy mappings bound into the input identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mappings: Vec<TypedMapping>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    Item {
        endpoint: Endpoint,
        source: ItemSource,
    },
    Code {
        reference: String,
    },
    External {
        address: String,
    },
    Iri {
        value: String,
    },
    Text {
        value: String,
        language: Option<String>,
    },
    Integer {
        value: i64,
    },
    Number {
        value: String,
    },
    Boolean {
        value: bool,
    },
    Edge {
        edge: Edge,
    },
}
impl Value {
    fn kind(&self) -> ValueType {
        match self {
            Self::Item { .. } => ValueType::Item,
            Self::Code { .. } => ValueType::Code,
            Self::External { .. } => ValueType::External,
            Self::Iri { .. } => ValueType::Iri,
            Self::Text { .. } => ValueType::Text,
            Self::Integer { .. } => ValueType::Integer,
            Self::Number { .. } => ValueType::Number,
            Self::Boolean { .. } => ValueType::Boolean,
            Self::Edge { .. } => ValueType::Edge,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Binding {
    pub values: BTreeMap<String, Option<Value>>,
    pub contributors: Vec<Value>,
    pub support: Vec<Support>,
    /// External snapshot/record provenance for overlaid facts; absent for graph-only results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<FactSupport>,
}
/// Provenance for one overlaid external fact key. Content identity is bound into input identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactSupport {
    /// Declared external fact input name.
    pub input: String,
    /// Opaque snapshot label supplied by the host.
    pub identity: String,
    /// Declared key columns to typed values used for the lookup.
    pub key: BTreeMap<String, Value>,
    /// Record index within the snapshot when a record was present; absent for covered-absent.
    pub record: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    Root {
        root: Endpoint,
        source: ItemSource,
        binding_count: Option<usize>,
    },
    Binding {
        root: Endpoint,
        binding: Binding,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub selected_roots: usize,
    pub binding_count: Option<usize>,
    pub known_bindings: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub format_version: u32,
    pub definition: String,
    pub definition_file: Option<String>,
    pub input_identity: String,
    pub evaluation_complete: bool,
    pub unavailable_reason: Option<String>,
    pub summary: Summary,
    pub records: Vec<Record>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
}
impl Error {
    pub fn argument(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_argument".into(),
            message: message.into(),
            source: None,
        }
    }
    pub(super) fn definition(file: &str, message: impl Into<String>) -> Self {
        Self {
            code: "invalid_definition".into(),
            message: message.into(),
            source: Some(file.into()),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}

pub(crate) fn graph_source_prerequisites_valid(diagnostics: &[crate::Diagnostic]) -> bool {
    diagnostics.iter().all(|diagnostic| {
        diagnostic.is_missing_mid()
            || matches!(
                diagnostic.code(),
                crate::DiagnosticCode::CodeMissing
                    | crate::DiagnosticCode::CodeAmbiguous
                    | crate::DiagnosticCode::CodeUnsupported
                    | crate::DiagnosticCode::EvaluationUnavailable
            )
    })
}

// @mara implements REQ-NAMED-QUERY-FACTS
// @mara implements DES-QUERY-INPUT-RESULT-CONTRACT
pub(crate) struct QueryInputPreflight {
    pub(crate) definitions: BTreeMap<String, Definition>,
    unavailable: BTreeMap<String, Option<String>>,
    native_code: Option<std::sync::Arc<native::NativeCodeLoader>>,
}
impl QueryInputPreflight {
    /// Required unavailable inputs for a definition and its declared dependencies.
    pub(crate) fn unavailable_for(&self, id: &str) -> Option<String> {
        fn visit(
            id: &str,
            preflight: &QueryInputPreflight,
            seen: &mut BTreeSet<String>,
        ) -> Option<String> {
            if !seen.insert(id.to_owned()) {
                return None;
            }
            let definition = preflight.definitions.get(id)?;
            if definition.required_fact_sources.contains(&FactSource::Code)
                && let Some(reason) = native::code_unavailable(preflight.native_code.as_deref(), id)
            {
                return Some(reason);
            }
            if let Some(reason) = preflight
                .unavailable
                .get(id)
                .and_then(|reason| reason.clone())
            {
                return Some(reason);
            }
            for dependency in &definition.dependencies {
                if let Some(reason) = visit(&dependency.definition, preflight, seen) {
                    return Some(reason);
                }
            }
            None
        }
        visit(id, self, &mut BTreeSet::new())
    }
}

impl SourceEngine {
    fn load_query_definitions(
        &self,
        files: &[String],
    ) -> Result<BTreeMap<String, composition::LoadedDefinition>, Error> {
        if files.is_empty() {
            return Err(Error::argument("supply an explicit definition set"));
        }
        let (_, schema) = self.strict_project().map_err(|e| Error {
            code: "input_unavailable".into(),
            message: e.to_string(),
            source: None,
        })?;
        let mut definitions = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for file in files {
            let path = self
                .normalize_path(file)
                .map_err(|e| Error::argument(e.to_string()))?;
            if !seen.insert(path.clone()) {
                return Err(Error::argument("duplicate normalized definition file"));
            }
            let text = self.files().get(&path).ok_or_else(|| Error {
                code: "input_unavailable".into(),
                message: format!("missing definition source: {path}"),
                source: Some(path.clone()),
            })?;
            let set: DefinitionSet =
                serde_json::from_str(text).map_err(|e| Error::definition(&path, e.to_string()))?;
            if set.format_version != 1 {
                return Err(Error::definition(
                    &path,
                    "expected named-definition format_version 1",
                ));
            }
            for definition in set.definitions {
                if definition.id.trim().is_empty()
                    || definitions
                        .insert(
                            definition.id.clone(),
                            composition::LoadedDefinition {
                                file: path.clone(),
                                definition,
                            },
                        )
                        .is_some()
                {
                    return Err(Error::definition(
                        &path,
                        "empty or duplicate definition identity across loaded set",
                    ));
                }
            }
        }
        composition::validate_loaded_dependency_graph(&definitions)?;
        for (id, loaded) in &definitions {
            signature::compile(&loaded.file, &loaded.definition, &schema).map_err(|error| {
                Error::definition(&loaded.file, format!("{id}: {}", error.message))
            })?;
        }
        Ok(definitions)
    }

    pub(crate) fn validated_query_definitions(
        &self,
        files: &[String],
    ) -> Result<BTreeMap<String, Definition>, Error> {
        Ok(self
            .load_query_definitions(files)?
            .into_iter()
            .map(|(id, loaded)| (id, loaded.definition))
            .collect())
    }

    /// Validate supplied facts/mappings with the definition-format-1 preparer used by query
    /// execution, independent of selected roots or dependency reachability.
    pub(crate) fn preflight_query_inputs(
        &self,
        files: &[String],
        fact_snapshots: &[FactSnapshot],
        mappings: &[TypedMapping],
    ) -> Result<QueryInputPreflight, Error> {
        let loaded = self.load_query_definitions(files)?;
        let definitions = loaded
            .iter()
            .map(|(id, value)| (id.clone(), value.definition.clone()))
            .collect::<BTreeMap<_, _>>();
        let (_, _, corpus) = self.corpus().map_err(|error| Error {
            code: "input_unavailable".into(),
            message: error.to_string(),
            source: None,
        })?;
        let declared_inputs = definitions
            .values()
            .flat_map(|definition| {
                definition
                    .external_facts
                    .iter()
                    .map(|fact| fact.input.clone())
            })
            .collect::<BTreeSet<_>>();
        let declared_mappings = definitions
            .values()
            .flat_map(|definition| {
                definition
                    .lookups
                    .iter()
                    .map(|lookup| lookup.mapping.clone())
            })
            .collect::<BTreeSet<_>>();
        let request = Request {
            definition_files: files.to_vec(),
            definition: String::new(),
            selection: Selection::default(),
            parameters: vec![],
            max_rows: 1,
            limit: 1,
            cursor: None,
            fact_snapshots: fact_snapshots.to_vec(),
            mappings: mappings.to_vec(),
        };
        let inputs = facts::inputs(&request, &declared_inputs, &declared_mappings)?;
        let mut unavailable = BTreeMap::new();
        for (id, loaded) in &loaded {
            let prepared = facts::prepare(&loaded.definition, &loaded.file, &corpus, &inputs)?;
            unavailable.insert(id.clone(), prepared.unavailable().map(str::to_owned));
        }
        Ok(QueryInputPreflight {
            definitions,
            unavailable,
            native_code: self.native_query_code.clone(),
        })
    }

    pub fn query_execute(&self, request: Request) -> Result<Page, Error> {
        composition::execute(self, request)
    }
}
fn typed(
    term: &Term,
    graph: &PreparedGraph,
    derived_support: &BTreeMap<String, Support>,
) -> Result<Value, String> {
    use oxrdf::vocab::{rdf, xsd};
    Ok(match term {
        Term::NamedNode(n) => {
            if let Some((endpoint, source)) = graph.items.get(n.as_str()) {
                Value::Item {
                    endpoint: endpoint.clone(),
                    source: source.clone(),
                }
            } else if let Some(s) = graph
                .support
                .get(n.as_str())
                .or_else(|| derived_support.get(n.as_str()))
            {
                Value::Edge {
                    edge: s.edge.clone(),
                }
            } else {
                match graph.endpoints.get(n.as_str()) {
                    Some(Endpoint::External { address }) => Value::External {
                        address: address.clone(),
                    },
                    Some(Endpoint::Code { reference }) => Value::Code {
                        reference: reference.clone(),
                    },
                    _ => Value::Iri {
                        value: n.as_str().into(),
                    },
                }
            }
        }
        Term::Literal(l) => {
            crate::query_backend::literal_profile(l)?;
            match l.datatype() {
                xsd::STRING | rdf::LANG_STRING => Value::Text {
                    value: l.value().into(),
                    language: l.language().map(str::to_owned),
                },
                xsd::INTEGER => Value::Integer {
                    value: l.value().parse().map_err(|_| "invalid integer result")?,
                },
                xsd::DOUBLE => Value::Number {
                    value: l.value().into(),
                },
                xsd::BOOLEAN => Value::Boolean {
                    value: l.value() == "true",
                },
                _ => return Err("unsupported result datatype".into()),
            }
        }
        _ => return Err("blank/RDF-star result is not a typed Mara binding".into()),
    })
}
fn unavailable(mut page: Page, reason: String, start: usize) -> Result<Page, Error> {
    if start != 0 {
        return Err(Error::argument("input unavailable; restart continuation"));
    }
    page.evaluation_complete = false;
    page.unavailable_reason = Some(reason);
    page.summary.binding_count = None;
    enforce_page_budget(&page)?;
    Ok(page)
}
fn cursor_position(cursor: Option<&str>, identity: &str) -> Result<usize, Error> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let mut parts = cursor.split(':');
    if parts.next() != Some("3") || parts.next() != Some(identity) {
        return Err(Error::argument("invalid or stale query cursor; restart"));
    }
    let position = parts
        .next()
        .filter(|p| p.len() == 16)
        .and_then(|p| usize::from_str_radix(p, 16).ok())
        .ok_or_else(|| Error::argument("invalid query cursor position"))?;
    if position == 0 || parts.next().is_some() {
        return Err(Error::argument("invalid query cursor position"));
    }
    Ok(position)
}
fn enforce_page_budget(page: &Page) -> Result<(), Error> {
    if serde_json::to_vec(page).expect("query page").len() > 65_536 {
        return Err(Error::argument(
            "query page exceeds the 65536-byte delivery budget",
        ));
    }
    Ok(())
}
fn paginate(
    mut page: Page,
    records: Vec<Record>,
    start: usize,
    limit: usize,
    continuation: bool,
) -> Result<Page, Error> {
    if continuation && start >= records.len() {
        return Err(Error::argument("query cursor position outside result"));
    }
    for record in records.iter().skip(start).take(limit) {
        page.records.push(record.clone());
        let next = start + page.records.len();
        page.has_more = next < records.len();
        page.next_cursor = page
            .has_more
            .then(|| format!("3:{}:{next:016x}", page.input_identity));
        if serde_json::to_vec(&page).expect("query page").len() > 65_536 {
            page.records.pop();
            if page.records.is_empty() {
                return Err(Error::argument(
                    "one query record exceeds the 65536-byte delivery budget",
                ));
            }
            let next = start + page.records.len();
            page.has_more = true;
            page.next_cursor = Some(format!("3:{}:{next:016x}", page.input_identity));
            break;
        }
    }
    enforce_page_budget(&page)?;
    Ok(page)
}
