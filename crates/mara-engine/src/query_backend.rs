//! Executable backend proof, deliberately narrower than the complete query API.
//! Standard SPARQL supplies facts. SHACL sees only those projected facts.
use crate::{ItemSource, RelationEdge, RelationEndpoint, RelationOccurrence, SourceEngine};
use oxigraph::{
    io::{RdfFormat, RdfParser},
    model::{Graph, Literal, NamedNode, Term, Triple, Variable},
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};
use oxrdf::graph::{CanonicalizationAlgorithm, CanonicalizationHashAlgorithm};
use rudof_iri::IriS;
use rudof_rdf::{
    rdf_core::{RDFFormat, term::Object},
    rdf_impl::{OxigraphInMemory, ReaderMode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shacl::{
    ir::IRSchema,
    validator::{engine::Validate, nodes::FocusNodes},
};
use spargebra::{
    Query, SparqlParser,
    algebra::{Expression, GraphPattern},
    term::{GroundTerm, TermPattern},
};
use std::collections::{BTreeMap, BTreeSet};
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

const Q: &str = "urn:mara:query:";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub format_version: u32,
    pub id: String,
    pub select: String,
    /// Projected tuple columns; ?root is the invocation key, not a tuple column.
    pub columns: Vec<String>,
    /// Projected canonical edge identifiers, excluded from tuple distinctness.
    pub support_columns: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub definition_file: String,
    pub roots: Vec<String>,
    /// Budget counts observed solution rows, including supporting duplicates.
    pub max_rows: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRequest {
    pub query: Request,
    pub shape_file: String,
    pub shape: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypedValue {
    Item {
        endpoint: RelationEndpoint,
        source: ItemSource,
    },
    Iri {
        value: String,
    },
    Literal {
        value: String,
        datatype: String,
        language: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Support {
    pub edge: RelationEdge,
    pub occurrences: Vec<RelationOccurrence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub values: BTreeMap<String, Option<TypedValue>>,
    pub support: Vec<Support>,
}
struct SupportedTuple {
    values: BTreeMap<String, Option<TypedValue>>,
    edges: BTreeMap<String, Support>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootFacts {
    pub root: RelationEndpoint,
    pub source: ItemSource,
    pub bindings: Vec<Binding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facts {
    pub format_version: u32,
    pub definition: String,
    pub definition_file: String,
    pub input_identity: String,
    pub evaluation_complete: bool,
    pub unavailable_reason: Option<String>,
    pub roots: Vec<RootFacts>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheck {
    pub root: RelationEndpoint,
    pub state: String,
    pub shape_file: String,
    pub shape: String,
    pub components: Vec<String>,
    pub counts: Vec<Count>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Count {
    pub shape: String,
    pub component: String,
    pub total: usize,
    pub qualifying: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    pub format_version: u32,
    pub evaluation_complete: bool,
    pub facts: Facts,
    pub checks: Vec<PolicyCheck>,
}

// @mara implements DES-QUERY-POLICY-ENGINE
impl SourceEngine {
    /// Runs the definition from the explicitly supplied snapshot, without policy.
    pub fn query_backend_execute(&self, request: Request) -> Result<Facts, String> {
        let text = self
            .files()
            .get(&request.definition_file)
            .ok_or("missing query definition source")?;
        let definition: Definition =
            serde_json::from_str(text).map_err(|e| format!("{}: {e}", request.definition_file))?;
        if definition.format_version != 1 || definition.id.is_empty() || request.max_rows == 0 {
            return Err("invalid definition version/identity or zero row budget".into());
        }
        let mut names = BTreeSet::new();
        for name in definition.columns.iter().chain(&definition.support_columns) {
            if name == "root" || !crate::rules::valid_parameter_name(name) || !names.insert(name) {
                return Err("invalid or repeated projection column".into());
            }
        }
        if definition.columns.is_empty() {
            return Err("a query needs tuple columns".into());
        }
        let query = SparqlParser::new()
            .parse_query(&definition.select)
            .map_err(|e| e.to_string())?;
        let Query::Select {
            dataset: None,
            pattern,
            ..
        } = &query
        else {
            return Err("proof profile requires SELECT over the supplied default graph".into());
        };
        profile(pattern)?;
        let mut projected = BTreeSet::new();
        pattern.on_in_scope_variable(|variable| {
            projected.insert(variable.as_str());
        });
        if !projected.contains("root") || !names.iter().all(|v| projected.contains(v.as_str())) {
            return Err("SELECT must project ?root and every declared column".into());
        }
        let (project, schema, corpus) = self.corpus().map_err(|e| e.to_string())?;
        // Source conformance is independent of enabled documentation invariants.
        let validation = crate::validate_corpus(&corpus, &schema);
        if !validation.is_empty() {
            return Err("unavailable: graph source prerequisites are invalid".into());
        }
        let graph =
            crate::factual_graph::prepare(&project, &corpus, &schema, self.source_fingerprint())?;
        let mut items = BTreeMap::new();
        for item in corpus.items() {
            let mid = item.mid().ok_or("unavailable: query proof requires MIDs")?;
            let iri = format!("urn:mara:mid:{mid}");
            items.insert(
                iri,
                TypedValue::Item {
                    endpoint: RelationEndpoint::Item {
                        id: item.id().into(),
                        mid: mid.into(),
                    },
                    source: item.source().into(),
                },
            );
        }
        let mut support = BTreeMap::new();
        for record in corpus.relation_graph(&schema).edges() {
            let edge = &record.edge;
            let iri = format!(
                "{Q}edge:{}",
                hash(&serde_json::to_vec(edge).expect("edge serializes"))
            );

            let mut occurrences = crate::relations::occurrences(&project, &corpus, &schema, edge)
                .map_err(|e| e.to_string())?;
            for occurrence in &mut occurrences {
                // Existing relation-get cursors use a host-width-dependent hash.
                // Query occurrence identity is portable and independent of paths.
                occurrence.reference = format!(
                    "query-occ-1-{}",
                    hash(
                        &serde_json::to_vec(&(
                            self.source_fingerprint(),
                            &occurrence.source,
                            &occurrence.author,
                            &occurrence.kind,
                            &occurrence.relation,
                            &occurrence.target,
                        ))
                        .expect("occurrence identity serializes")
                    )
                );
            }
            support.insert(
                iri,
                Support {
                    edge: edge.clone(),
                    occurrences,
                },
            );
        }
        let store = Store::new().map_err(|e| e.to_string())?;
        for triple in graph.triples {
            store
                .insert(&oxigraph::model::Quad::new(
                    triple.subject,
                    triple.predicate,
                    triple.object,
                    oxigraph::model::GraphName::DefaultGraph,
                ))
                .map_err(|e| e.to_string())?;
        }
        let mut selected = BTreeMap::new();
        for reference in &request.roots {
            let item = crate::resolve_item(&corpus, reference).map_err(|e| e.to_string())?;
            let iri = format!("urn:mara:mid:{}", item.mid().ok_or("missing root MID")?);
            selected.insert(
                iri,
                (
                    RelationEndpoint::Item {
                        id: item.id().into(),
                        mid: item.mid().unwrap().into(),
                    },
                    ItemSource::from(item.source()),
                ),
            );
        }
        let mut facts = Facts {
            format_version: 1,
            definition: definition.id,
            definition_file: request.definition_file.clone(),
            input_identity: hash(
                &serde_json::to_vec(&(self.source_fingerprint(), &request))
                    .expect("invocation serializes"),
            ),
            evaluation_complete: true,
            unavailable_reason: None,
            roots: vec![],
        };
        let mut rows = 0;
        for (iri, (root, source)) in selected {
            let mut tuples: BTreeMap<String, SupportedTuple> = BTreeMap::new();
            if facts.evaluation_complete {
                let evaluator = SparqlEvaluator::new();
                // The pinned native dependency closure enables HTTP. WASM has
                // no HTTP handler; SERVICE is rejected by the profile on both.
                #[cfg(not(target_family = "wasm"))]
                let evaluator = evaluator.without_default_http_service_handler();
                let prepared = evaluator.for_query(query.clone()).substitute_variable(
                    Variable::new("root").unwrap(),
                    NamedNode::new(&iri).unwrap(),
                );
                let QueryResults::Solutions(solutions) = prepared
                    .on_store(&store)
                    .execute()
                    .map_err(|e| e.to_string())?
                else {
                    unreachable!("validated SELECT")
                };
                for solution in solutions {
                    let solution = solution.map_err(|e| e.to_string())?;
                    if rows == request.max_rows {
                        facts.evaluation_complete = false;
                        facts.unavailable_reason = Some("observed solution-row budget reached; no exact count or negative conclusion".into());
                        break;
                    }
                    rows += 1;
                    if solution.get("root") != Some(&NamedNode::new(&iri).unwrap().into()) {
                        return Err("query changed its explicitly bound root".into());
                    }
                    let values: BTreeMap<_, _> = definition
                        .columns
                        .iter()
                        .map(|name| {
                            Ok((
                                name.clone(),
                                solution
                                    .get(name.as_str())
                                    .map(|term| typed(term, &items))
                                    .transpose()?,
                            ))
                        })
                        .collect::<Result<_, String>>()?;
                    let mut edges = BTreeMap::new();
                    for name in &definition.support_columns {
                        if let Some(term) = solution.get(name.as_str()) {
                            let Term::NamedNode(edge_id) = term else {
                                return Err("support must be a canonical edge IRI".into());
                            };
                            let record = support
                                .get(edge_id.as_str())
                                .ok_or("query returned unknown edge support")?;
                            edges.insert(edge_id.to_string(), record.clone());
                        }
                    }
                    // Unmatched OPTIONAL-only rows have neither values nor support.
                    // Real supported matches retain absent projected columns.
                    if values.values().all(Option::is_none) && edges.is_empty() {
                        continue;
                    }
                    let key = serde_json::to_string(&values).expect("values serialize");
                    let entry = tuples.entry(key).or_insert_with(|| SupportedTuple {
                        values,
                        edges: BTreeMap::new(),
                    });
                    entry.edges.extend(edges);
                }
            }
            facts.roots.push(RootFacts {
                root,
                source,
                bindings: tuples
                    .into_values()
                    .map(|tuple| Binding {
                        values: tuple.values,
                        support: tuple.edges.into_values().collect(),
                    })
                    .collect(),
            });
        }
        Ok(facts)
    }

    /// Evaluates a supplied SHACL shape over the same definition's projected result.
    /// The shape cannot traverse the source corpus: only root/row/column RDF exists.
    pub fn query_backend_policy(&self, request: PolicyRequest) -> Result<Evaluation, String> {
        let shapes = self
            .files()
            .get(&request.shape_file)
            .ok_or("missing policy source")?;
        let mut graph = Graph::new();
        for quad in RdfParser::from_format(RdfFormat::Turtle).for_slice(shapes.as_bytes()) {
            let quad = quad.map_err(|e| e.to_string())?;
            match &quad.object {
                Term::Literal(literal) => literal_profile(literal)?,
                Term::Triple(_) => {
                    return Err("RDF-star policy terms are outside the proof profile".into());
                }
                _ => {}
            }
            graph.insert(&Triple::new(quad.subject, quad.predicate, quad.object));
        }
        // Parser entropy and host word width must not affect anonymous constraints.
        graph.canonicalize(CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
        });
        let ir = IRSchema::from_str(
            &graph.to_string(),
            &RDFFormat::NTriples,
            None,
            &ReaderMode::Strict,
        )
        .map_err(|e| e.to_string())?;
        let shape = ir
            .get_shape(&Object::iri(
                IriS::new(&request.shape).map_err(|e| e.to_string())?,
            ))
            .ok_or("unknown policy shape")?;
        let facts = self.query_backend_execute(request.query)?;
        let mut checks = vec![];
        let mut complete = facts.evaluation_complete;
        for root in &facts.roots {
            let focus = crate::rules::relation_iri(&root.root);
            let mut check = PolicyCheck {
                root: root.root.clone(),
                state: "unavailable".into(),
                shape_file: request.shape_file.clone(),
                shape: request.shape.clone(),
                components: vec![],
                counts: vec![],
            };
            if facts.evaluation_complete {
                let mut graph = vec![json!({"@id":focus})];
                for (index, binding) in root.bindings.iter().enumerate() {
                    let row_id = format!("_:query_row_{index}");
                    graph.push(json!({"@id":focus,format!("{Q}match"):{"@id":row_id}}));
                    let mut row = json!({"@id":row_id});
                    for (column, value) in &binding.values {
                        if let Some(value) = value {
                            row[format!("{Q}column:{column}")] = json_ld(value);
                        }
                    }
                    graph.push(row);
                }
                let data = OxigraphInMemory::from_str(
                    &json!({"@graph":graph}).to_string(),
                    &RDFFormat::JsonLd,
                    None,
                    &ReaderMode::Strict,
                )
                .map_err(|e| e.to_string())?;
                let mut engine = crate::rules::engine::RuleEngine::new();
                let outcome = shape.validate(
                    &data,
                    &mut engine,
                    Some(&FocusNodes::single(
                        Object::iri(IriS::new(&focus).unwrap()).into(),
                    )),
                    None,
                    &ir,
                );
                match outcome {
                    Ok(outcome) if !engine.failed.get() => {
                        check.state = if outcome.conforms() {
                            "passed"
                        } else {
                            "failed"
                        }
                        .into();
                        check.components = outcome
                            .violations()
                            .iter()
                            .map(|v| v.constraint_component().to_string())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect();
                        check.counts = engine
                            .counts
                            .into_iter()
                            .map(|((shape, _, component), (total, qualifying))| Count {
                                shape,
                                component,
                                total,
                                qualifying,
                            })
                            .collect();
                    }
                    _ => {
                        complete = false;
                    }
                }
            }
            checks.push(check);
        }
        Ok(Evaluation {
            format_version: 1,
            evaluation_complete: complete,
            facts,
            checks,
        })
    }
}

fn typed(term: &Term, items: &BTreeMap<String, TypedValue>) -> Result<TypedValue, String> {
    match term {
        Term::NamedNode(n) => Ok(items.get(n.as_str()).cloned().unwrap_or(TypedValue::Iri {
            value: n.as_str().into(),
        })),
        Term::Literal(l) => {
            literal_profile(l)?;
            Ok(TypedValue::Literal {
                value: l.value().into(),
                datatype: l.datatype().as_str().into(),
                language: l.language().map(str::to_owned),
            })
        }
        _ => Err("blank node output is outside the proof profile".into()),
    }
}
fn json_ld(value: &TypedValue) -> Value {
    match value {
        TypedValue::Item { endpoint, .. } => json!({"@id":crate::rules::relation_iri(endpoint)}),
        TypedValue::Iri { value } => json!({"@id":value}),
        TypedValue::Literal {
            value,
            datatype,
            language,
        } => match language {
            Some(language) => json!({"@value":value,"@language":language}),
            None => json!({"@value":value,"@type":datatype}),
        },
    }
}

/// A bounded standard-language subset, not a new expression interpreter.
/// SPARQL '=' coerces numeric types; this proof admits only sameTerm equality.
pub(crate) fn profile(pattern: &GraphPattern) -> Result<(), String> {
    match pattern {
        GraphPattern::Bgp { patterns } => {
            for triple in patterns { term_profile(&triple.subject)?; term_profile(&triple.object)?; }
            Ok(())
        }
        GraphPattern::Path { subject, object, .. } => { term_profile(subject)?; term_profile(object) }
        GraphPattern::Values { bindings, .. } => {
            for term in bindings.iter().flatten().flatten() {
                match term {
                    GroundTerm::Literal(literal) => literal_profile(literal)?,
                    GroundTerm::Triple(_) => return Err("RDF-star query terms are outside the proof profile".into()),
                    _ => {}
                }
            }
            Ok(())
        }
        GraphPattern::Join { left, right } | GraphPattern::Union { left, right } | GraphPattern::Minus { left, right } => { profile(left)?; profile(right) },
        GraphPattern::LeftJoin { left, right, expression } => { profile(left)?; profile(right)?; if let Some(e) = expression { expression_profile(e)?; } Ok(()) },
        GraphPattern::Filter { expr, inner } => { expression_profile(expr)?; profile(inner) },
        GraphPattern::Extend { expression, inner, .. } => { expression_profile(expression)?; profile(inner) },
        GraphPattern::Project { inner, .. } | GraphPattern::Distinct { inner } => profile(inner),
        _ => Err("query outside the bounded SELECT proof profile (no SERVICE, dataset, slicing, nondeterminism or aggregate shortcut)".into()),
    }
}
pub(crate) fn literal_profile(literal: &Literal) -> Result<(), String> {
    if literal.direction().is_some() {
        return Err("directional literals are outside the RDF-1.1 proof result profile".into());
    }
    // Oxigraph internally normalizes known numeric datatypes. Admit only
    // primitive spellings that survive that normalization unchanged.
    let value = literal.value();
    let valid = match literal.datatype() {
        oxrdf::vocab::xsd::STRING | oxrdf::vocab::rdf::LANG_STRING => true,
        oxrdf::vocab::xsd::BOOLEAN => matches!(value, "true" | "false"),
        oxrdf::vocab::xsd::INTEGER => value
            .parse::<i64>()
            .is_ok_and(|number| number.to_string() == value),
        oxrdf::vocab::xsd::DOUBLE => value.parse::<f64>().is_ok_and(|number| {
            number.is_finite()
                && !(number == 0.0 && number.is_sign_negative())
                && number.to_string() == value
        }),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("literal outside the canonical primitive proof profile".into())
    }
}
fn term_profile(term: &TermPattern) -> Result<(), String> {
    match term {
        TermPattern::Literal(literal) => literal_profile(literal),
        TermPattern::Triple(_) => Err("RDF-star query terms are outside the proof profile".into()),
        _ => Ok(()),
    }
}
fn expression_profile(expression: &Expression) -> Result<(), String> {
    match expression {
        Expression::Literal(literal) => literal_profile(literal),
        Expression::NamedNode(_) | Expression::Variable(_) | Expression::Bound(_) => Ok(()),
        Expression::SameTerm(a, b) | Expression::And(a, b) | Expression::Or(a, b) => {
            expression_profile(a)?;
            expression_profile(b)
        }
        Expression::Not(e) => expression_profile(e),
        Expression::Exists(p) => profile(p),
        _ => Err("expression outside the exact typed proof profile".into()),
    }
}
