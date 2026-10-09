//! Named factual SELECT execution. Hosts supply sources; policies do not gate facts.
mod signature;
pub use crate::factual_graph::{Edge, Endpoint, Occurrence, Support};
use crate::factual_graph::{PreparedGraph, hash, prepare};
use crate::{FieldFilter, ItemFilters, ItemSource, SourceEngine};
use oxigraph::{
    model::{GraphName, Literal, NamedNode, Quad, Term, Variable},
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};
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
    pub(super) fn kind(&self) -> ValueType {
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
    /// Extra projected item variables retained as contributing source support.
    #[serde(default)]
    pub source_columns: Vec<String>,
    #[serde(default, deserialize_with = "unique_map")]
    pub parameters: BTreeMap<String, Parameter>,
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
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub values: BTreeMap<String, Option<Value>>,
    pub contributors: Vec<Value>,
    pub support: Vec<Support>,
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

// @mara implements REQ-NAMED-QUERY-FACTS
// @mara implements DES-QUERY-INPUT-RESULT-CONTRACT
impl SourceEngine {
    pub fn query_execute(&self, request: Request) -> Result<Page, Error> {
        let filtered = !request.selection.ids.is_empty()
            || !request.selection.flavours.is_empty()
            || !request.selection.fields.is_empty()
            || !request.selection.paths.is_empty();
        if request.selection.all == filtered {
            return Err(Error::argument(
                "select explicit all or at least one ID/filter; all is exclusive",
            ));
        }
        if request.max_rows == 0 || !(1..=100).contains(&request.limit) {
            return Err(Error::argument(
                "max_rows must be positive; limit must be 1..=100",
            ));
        }
        if request.definition_files.is_empty() {
            return Err(Error::argument("supply an explicit definition set"));
        }
        let mut identity_request = request.clone();
        identity_request.cursor = None;
        let identity = hash(&(
            "named-query-3",
            env!("CARGO_PKG_VERSION"),
            self.source_fingerprint(),
            &identity_request,
        ));
        let mut page = Page {
            format_version: 3,
            definition: request.definition.clone(),
            definition_file: None,
            input_identity: identity,
            evaluation_complete: true,
            unavailable_reason: None,
            summary: Summary {
                selected_roots: 0,
                binding_count: Some(0),
                known_bindings: 0,
            },
            records: vec![],
            has_more: false,
            next_cursor: None,
        };
        let start = cursor_position(request.cursor.as_deref(), &page.input_identity)?;
        let mut definitions = BTreeMap::new();
        let mut files = BTreeSet::new();
        for file in &request.definition_files {
            let path = self
                .normalize_path(file)
                .map_err(|e| Error::argument(e.to_string()))?;
            if !files.insert(path.clone()) {
                return Err(Error::argument("duplicate normalized definition file"));
            }
            let Some(text) = self.files().get(&path) else {
                return unavailable(page, format!("missing definition source: {path}"), start);
            };
            let set: DefinitionSet =
                serde_json::from_str(text).map_err(|e| Error::definition(&path, e.to_string()))?;
            if set.format_version != 3 {
                return Err(Error::definition(
                    &path,
                    "expected named-definition format 3",
                ));
            }
            for definition in set.definitions {
                if definition.id.trim().is_empty() {
                    return Err(Error::definition(&path, "empty definition identity"));
                }
                if definitions
                    .insert(definition.id.clone(), (path.clone(), definition))
                    .is_some()
                {
                    return Err(Error::definition(
                        &path,
                        "duplicate definition identity across loaded set",
                    ));
                }
            }
        }
        let (file, definition) = definitions
            .get(&request.definition)
            .ok_or_else(|| Error::definition("", "unknown exact definition identity"))?;
        page.definition_file = Some(file.clone());
        let (_, schema) = match self.strict_project() {
            Ok(v) => v,
            Err(e) => return unavailable(page, format!("schema input unavailable: {e}"), start),
        };
        // Bind every loaded definition before enumerating roots, even for an empty selection.
        let mut compiled = BTreeMap::new();
        for (id, (path, definition)) in &definitions {
            compiled.insert(id, signature::compile(path, definition, &schema)?);
        }
        let (project, _, corpus) = match self.corpus() {
            Ok(v) => v,
            Err(e) => return unavailable(page, format!("source input unavailable: {e}"), start),
        };
        let parameters = parameters(file, definition, &request.parameters, &corpus)?;
        let diagnostics = crate::validate_corpus(&corpus, &schema);
        if diagnostics.iter().any(|d| {
            !d.is_missing_mid()
                && !matches!(
                    d.code(),
                    crate::DiagnosticCode::CodeMissing
                        | crate::DiagnosticCode::CodeAmbiguous
                        | crate::DiagnosticCode::CodeUnsupported
                        | crate::DiagnosticCode::EvaluationUnavailable
                )
        }) {
            return unavailable(page, "graph source prerequisites are invalid".into(), start);
        }
        let graph = prepare(&project, &corpus, &schema, self.source_fingerprint())
            .map_err(|e| Error::definition(file, e))?;
        for (id, (path, _)) in &definitions {
            let query = &mut compiled
                .get_mut(id)
                .expect("compiled loaded definition")
                .query;
            signature::bind_items(query, &schema, &graph)
                .map_err(|e| Error::definition(path, e))?;
            signature::validate_item_constants(query, &graph)
                .map_err(|e| Error::definition(path, e))?
        }
        let mut query = compiled
            .remove(&request.definition)
            .expect("compiled selected definition")
            .query;
        signature::add_parameter_projection(&mut query, definition.parameters.keys());
        let filters = ItemFilters::new(
            request.selection.flavours.clone(),
            request
                .selection
                .fields
                .iter()
                .map(|f| FieldFilter::new(&f.key, &f.value))
                .collect(),
            vec![],
            request.selection.paths.iter().map(Into::into).collect(),
            None,
        )
        .with_ids(request.selection.ids.clone());
        let roots = crate::filtered_items(&corpus, &schema, &filters)
            .map_err(|e| Error::argument(e.to_string()))?;
        let mut selected = BTreeMap::new();
        for item in roots {
            if !definition.root_flavours.iter().any(|f| f == item.flavour()) {
                return Err(Error::argument(format!(
                    "root {} does not satisfy definition root signature",
                    item.id()
                )));
            }
            let root = Endpoint::item(item);
            selected.insert(root.iri(), (root, ItemSource::from(item.source())));
        }
        if definition.required_fact_sources.contains(&FactSource::Code) {
            page.evaluation_complete = false;
            page.unavailable_reason =
                Some("required code facts were not supplied to this named-query snapshot".into());
        }
        let store = Store::new().map_err(|e| Error::definition(file, e.to_string()))?;
        for triple in &graph.triples {
            store
                .insert(&Quad::new(
                    triple.subject.clone(),
                    triple.predicate.clone(),
                    triple.object.clone(),
                    GraphName::DefaultGraph,
                ))
                .map_err(|e| Error::definition(file, e.to_string()))?;
        }
        let mut records = vec![];
        let mut rows = 0u32;
        page.summary.selected_roots = selected.len();
        for (iri, (root, source)) in selected {
            let invocation = parameters
                .iter()
                .map(|(name, value)| (Variable::new(name).unwrap(), value.clone()))
                .chain(std::iter::once((
                    Variable::new("root").unwrap(),
                    NamedNode::new(&iri).unwrap().into(),
                )))
                .collect::<BTreeMap<_, _>>();
            let mut tuples: BTreeMap<String, Binding> = BTreeMap::new();
            if page.evaluation_complete {
                // Substitution-only inputs must remain visible to BIND/BOUND.
                let evaluator = SparqlEvaluator::new().without_optimizations();
                #[cfg(not(target_family = "wasm"))]
                let evaluator = evaluator.without_default_http_service_handler();
                let mut prepared = evaluator.for_query(query.clone());
                for (name, value) in &invocation {
                    prepared = prepared.substitute_variable(name.clone(), value.clone());
                }
                let solutions = match prepared.on_store(&store).execute() {
                    Ok(QueryResults::Solutions(s)) => s,
                    Ok(_) => unreachable!("compiled SELECT"),
                    Err(e) => {
                        page.evaluation_complete = false;
                        page.unavailable_reason =
                            Some(format!("backend evaluation unavailable: {e}"));
                        records.push(Record::Root {
                            root,
                            source,
                            binding_count: None,
                        });
                        continue;
                    }
                };
                for solution in solutions {
                    let solution = match solution {
                        Ok(s) => s,
                        Err(e) => {
                            page.evaluation_complete = false;
                            page.unavailable_reason =
                                Some(format!("backend evaluation unavailable: {e}"));
                            break;
                        }
                    };
                    if rows == request.max_rows {
                        page.evaluation_complete = false;
                        page.unavailable_reason = Some("observed solution-row budget reached; no exact count or negative conclusion".into());
                        break;
                    }
                    rows += 1;
                    if solution.get("root") != Some(&NamedNode::new(&iri).unwrap().into()) {
                        return Err(Error::definition(
                            file,
                            "SELECT changed its explicitly bound root",
                        ));
                    }
                    let mut values = BTreeMap::new();
                    for (name, column) in &definition.columns {
                        let value = solution
                            .get(name.as_str())
                            .map(|t| typed(t, &graph))
                            .transpose()
                            .map_err(|e| Error::definition(file, e))?;
                        if let Some(value) = &value {
                            if value.kind() != column.r#type {
                                return Err(Error::definition(
                                    file,
                                    format!("column {name} returned incompatible type"),
                                ));
                            }
                            if let Value::Item {
                                endpoint: Endpoint::Item { id, .. },
                                ..
                            } = value
                            {
                                let item = crate::resolve_item(&corpus, id).expect("graph item");
                                if !column.flavours.is_empty()
                                    && !column.flavours.iter().any(|f| f == item.flavour())
                                {
                                    return Err(Error::definition(
                                        file,
                                        format!("column {name} returned incompatible flavour"),
                                    ));
                                }
                            }
                        } else if !column.optional {
                            return Err(Error::definition(
                                file,
                                format!(
                                    "required column {name} is absent; declare optional bindings"
                                ),
                            ));
                        }
                        values.insert(name.clone(), value);
                    }
                    let mut contributors = BTreeMap::new();
                    let mut support = BTreeMap::new();
                    for name in &definition.support_columns {
                        if let Some(term) = solution.get(name.as_str()) {
                            let Term::NamedNode(id) = term else {
                                return Err(Error::definition(
                                    file,
                                    "support column must bind a canonical edge",
                                ));
                            };
                            let edge = graph.support.get(id.as_str()).ok_or_else(|| {
                                Error::definition(file, "unknown canonical edge support")
                            })?;
                            support.insert(id.as_str().to_owned(), edge.clone());
                        }
                    }
                    for name in &definition.source_columns {
                        if let Some(term) = solution.get(name.as_str()) {
                            let value =
                                typed(term, &graph).map_err(|e| Error::definition(file, e))?;
                            if !matches!(value, Value::Item { .. }) {
                                return Err(Error::definition(
                                    file,
                                    "source column must bind a canonical item",
                                ));
                            }
                            contributors.insert(hash(&value), value);
                        }
                    }
                    let key = serde_json::to_string(&values).expect("typed tuple");
                    let entry = tuples.entry(key).or_insert_with(|| Binding {
                        values,
                        contributors: vec![],
                        support: vec![],
                    });
                    for c in &entry.contributors {
                        contributors.insert(hash(c), c.clone());
                    }
                    for s in &entry.support {
                        support.insert(format!("urn:mara:query:edge:{}", hash(&s.edge)), s.clone());
                    }
                    entry.contributors = contributors.into_values().collect();
                    entry.support = support.into_values().collect();
                }
            }
            let count = tuples.len();
            page.summary.known_bindings += count;
            records.push(Record::Root {
                root: root.clone(),
                source,
                binding_count: Some(count),
            });
            records.extend(tuples.into_values().map(|binding| Record::Binding {
                root: root.clone(),
                binding,
            }));
        }
        if !page.evaluation_complete {
            page.summary.binding_count = None;
            for record in &mut records {
                if let Record::Root { binding_count, .. } = record {
                    *binding_count = None;
                }
            }
        } else {
            page.summary.binding_count = Some(page.summary.known_bindings);
        }
        paginate(
            page,
            records,
            start,
            request.limit,
            request.cursor.is_some(),
        )
    }
}
fn parameters(
    file: &str,
    definition: &Definition,
    supplied: &[BindingParameter],
    corpus: &crate::Corpus,
) -> Result<BTreeMap<String, Term>, Error> {
    let mut values = BTreeMap::new();
    for p in supplied {
        let declaration = definition
            .parameters
            .get(&p.name)
            .ok_or_else(|| Error::argument(format!("undeclared parameter {}", p.name)))?;
        if declaration.r#type != p.value.kind() {
            return Err(Error::argument(format!(
                "wrong type for parameter {}",
                p.name
            )));
        }
        if values
            .insert(p.name.clone(), p.value.term(corpus)?)
            .is_some()
        {
            return Err(Error::argument(format!("duplicate parameter {}", p.name)));
        }
    }
    for (name, p) in &definition.parameters {
        if !values.contains_key(name) {
            if let Some(value) = &p.default {
                values.insert(
                    name.clone(),
                    value
                        .term(corpus)
                        .map_err(|e| Error::definition(file, e.message))?,
                );
            } else if p.required {
                return Err(Error::argument(format!(
                    "missing required parameter {name}"
                )));
            }
        }
    }
    Ok(values)
}
fn typed(term: &Term, graph: &PreparedGraph) -> Result<Value, String> {
    use oxrdf::vocab::{rdf, xsd};
    Ok(match term {
        Term::NamedNode(n) => {
            if let Some((endpoint, source)) = graph.items.get(n.as_str()) {
                Value::Item {
                    endpoint: endpoint.clone(),
                    source: source.clone(),
                }
            } else if let Some(s) = graph.support.get(n.as_str()) {
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
