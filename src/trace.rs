//! Read-only, bounded trace matrices over the same native rule outcomes as validation.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    Corpus, DiagnosticCode, DiagnosticLocation, FieldFilter, Item, ItemFilters, Project,
    RelationEdge, RelationEndpoint, Schema, Severity, ValidationDiagnostic, ValidationError,
    ValidationResult, ValidationScope, ValidationSummary, ValidationTarget, ValidationTargetKind,
    query,
    rules::{RuleObservation, Rules},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceSelection {
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub flavours: Vec<String>,
    #[serde(default)]
    pub fields: Vec<TraceField>,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceField {
    /// Exact schema-declared custom field key; excludes title, MID and relation names.
    pub key: String,
    /// Scalar text to match exactly, including an empty value.
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceCheck {
    /// Nonempty project-relative YAML source paths, loaded only for this request.
    pub files: Vec<PathBuf>,
    /// Expanded IRI of one named targetless node shape in the supplied files.
    pub shape: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceMatrixParams {
    #[serde(flatten)]
    pub selection: TraceSelection,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub check: Option<TraceCheck>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub render: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TraceMatrixSummary {
    pub evaluation: Value,
    pub selected: usize,
    pub not_applicable: usize,
    pub passed: usize,
    pub failed: usize,
    pub unavailable: usize,
    pub counts_exact: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TraceRecord {
    Result {
        root: Value,
        evaluation: Value,
        state: String,
    },
    Check {
        reference: String,
        root: Value,
        evaluation: Value,
        obligation: Value,
        context: Vec<Value>,
        parent: Option<String>,
        state: String,
        condition: Value,
        counts: Value,
        every: Option<bool>,
        inspection: Option<Value>,
        reported: Option<Value>,
    },
    Edge {
        check: String,
        edge: Value,
        label: String,
        direction: String,
        endpoint: Value,
        outside_selection: bool,
        qualification: Option<String>,
        every: Option<String>,
        occurrence_count: usize,
        inspection: Value,
    },
    Issue {
        diagnostic: Value,
    },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TraceMatrixResult {
    pub format_version: u8,
    pub kind: String,
    pub selection: TraceSelection,
    pub evaluation_complete: bool,
    pub summaries: Vec<TraceMatrixSummary>,
    pub records: Vec<TraceRecord>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

pub(crate) fn matrix(
    project: &Project,
    schema: &Schema,
    params: &TraceMatrixParams,
) -> Result<TraceMatrixResult, ValidationError> {
    let mut normalized = params.selection.clone();
    normalized.ids.sort();
    normalized.ids.dedup();
    normalized.flavours.sort();
    normalized.flavours.dedup();
    normalized
        .fields
        .sort_by(|a, b| (&a.key, &a.value).cmp(&(&b.key, &b.value)));
    normalized
        .fields
        .dedup_by(|a, b| a.key == b.key && a.value == b.value);
    normalized.paths = query::normalized_paths(&normalized.paths)
        .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
    normalized.paths.sort();
    normalized.paths.dedup();
    let selection = &normalized;
    if selection.all
        == (!selection.ids.is_empty()
            || !selection.flavours.is_empty()
            || !selection.fields.is_empty()
            || !selection.paths.is_empty())
    {
        return Err(ValidationError::invalid_argument(
            "select at least one id, flavour, field or path, or use all:true alone",
        ));
    }
    if params.rules.is_empty() == params.check.is_none() {
        return Err(ValidationError::invalid_argument(
            "supply nonempty rules or one request check, never both",
        ));
    }
    let limit = params.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(ValidationError::invalid_argument(
            "limit must be 1 through 100",
        ));
    }
    if params.render.as_deref().is_some_and(|r| r != "markdown") {
        return Err(ValidationError::invalid_argument("render must be markdown"));
    }
    if params.cursor.as_deref() == Some("") {
        return Err(ValidationError::invalid_argument(
            "cursor must not be empty",
        ));
    }
    let (corpus, mut prerequisites) = crate::load_corpus_for_validation(project, schema)
        .map_err(|e| ValidationError::new("io_error", e.to_string()))?;
    prerequisites.extend(crate::validate_corpus(&corpus, schema));
    let filters = ItemFilters::new(
        selection.flavours.clone(),
        selection
            .fields
            .iter()
            .map(|f| FieldFilter::new(&f.key, &f.value))
            .collect(),
        vec![],
        selection.paths.clone(),
        None,
    )
    .with_search_options(selection.ids.clone(), false);
    let selected = query::filtered_items(&corpus, schema, &filters, None)
        .map_err(|e| ValidationError::invalid_argument(e.to_string()))?;
    let selected_mids = selected
        .iter()
        .filter_map(|item| item.mid().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut rules = if let Some(check) = &params.check {
        if check.files.is_empty() {
            return Err(ValidationError::invalid_argument(
                "check files must be nonempty",
            ));
        }
        if check.shape.is_empty() {
            return Err(ValidationError::invalid_argument("check shape is required"));
        }
        Rules::load_files(project, schema, check.files.clone())
    } else {
        Rules::load(project, schema)
    };
    let (kind, roots) = if let Some(check) = &params.check {
        let flavours = selected
            .iter()
            .map(|i| i.flavour().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let root = if rules.diagnostics.is_empty() {
            Some(
                rules
                    .request_check(&check.shape, schema, &flavours)
                    .map_err(ValidationError::invalid_argument)?,
            )
        } else {
            None
        };
        ("check", root.into_iter().collect::<Vec<_>>())
    } else {
        let mut ids = params.rules.clone();
        ids.sort();
        ids.dedup();
        for id in &ids {
            if !rules.root_ids().contains(id) {
                return Err(ValidationError::invalid_argument(format!(
                    "unknown enabled rule IRI '{id}'"
                )));
            }
        }
        ("rule", ids)
    };
    let evaluations = roots
        .iter()
        .map(|root| json!({"kind":kind,"shape":root}))
        .collect::<Vec<_>>();
    let descriptors = corpus
        .discovery()
        .nodes()
        .filter_map(|node| {
            let crate::DiscoveryNodeKind::Item(item) = node.kind() else {
                return None;
            };
            Some((
                item.mid().unwrap_or(item.id()).to_owned(),
                serde_json::to_value(node.summary()).unwrap(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let edges = collect_edges(&corpus, schema)?;
    let snapshot = snapshot_id(project, schema, &corpus, &rules, &prerequisites);
    let mut validation = empty_validation(project);
    let observations = if rules.diagnostics.is_empty() {
        rules.observe(
            &corpus,
            schema,
            &prerequisites,
            &mut validation,
            &roots,
            Some(&selected_mids),
        )
    } else {
        validation.evaluation_complete = false;
        Vec::new()
    };
    let observed = observations
        .into_iter()
        .map(|o| ((o.mid.clone(), o.root.clone()), o))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    for issue in rules
        .diagnostics
        .iter()
        .cloned()
        .chain(prerequisites.iter().map(ValidationDiagnostic::from_source))
    {
        records.push(record(json!({"kind":"issue","diagnostic":issue})));
    }
    if (!corpus.is_complete()
        || corpus
            .items()
            .any(|item| !item.validation_source_is_complete()))
        && !validation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::EvaluationUnavailable)
    {
        records.push(record(
            json!({"kind":"issue","diagnostic":unavailable_diagnostic(
            "matrix evaluation requires a complete, valid corpus") }),
        ));
    }
    let mut summaries = evaluations
        .iter()
        .map(|evaluation| TraceMatrixSummary {
            evaluation: evaluation.clone(),
            selected: selected.len(),
            not_applicable: 0,
            passed: 0,
            failed: 0,
            unavailable: 0,
            counts_exact: true,
        })
        .collect::<Vec<_>>();
    for item in &selected {
        let mid = item.mid().unwrap_or(item.id());
        for (index, root) in roots.iter().enumerate() {
            let observation = observed.get(&(mid.to_owned(), root.clone()));
            let state = observation.map_or("unavailable", |o| o.state);
            match state {
                "not_applicable" => summaries[index].not_applicable += 1,
                "passed" => summaries[index].passed += 1,
                "failed" => summaries[index].failed += 1,
                _ => {
                    summaries[index].unavailable += 1;
                    summaries[index].counts_exact = false;
                }
            }
            let root_descriptor = &descriptors[mid];
            let evaluation = &evaluations[index];
            records.push(record(json!({"kind":"result","root":root_descriptor,
                "evaluation":evaluation,"state":state})));
            if let Some(observation) = observation
                && matches!(state, "passed" | "failed")
            {
                let mut emitted = BTreeSet::new();
                let context = ExplainContext {
                    rules: &rules,
                    schema,
                    corpus: &corpus,
                    descriptors: &descriptors,
                    selected_mids: &selected_mids,
                    edges: &edges,
                    observation,
                    root: root_descriptor,
                    evaluation,
                    snapshot: &snapshot,
                };
                let mut output = ExplainRecords {
                    seen: &mut emitted,
                    records: &mut records,
                };
                explain(&context, root, item, None, &[], &mut output, 0);
            }
        }
    }
    for diagnostic in &validation.diagnostics {
        if diagnostic.code == DiagnosticCode::EvaluationUnavailable {
            records.push(record(json!({"kind":"issue","diagnostic":diagnostic})));
        }
    }
    let evaluation_complete = records
        .iter()
        .all(|r| !matches!(r, TraceRecord::Issue { .. }))
        && summaries.iter().all(|s| s.unavailable == 0);
    if !evaluation_complete {
        for summary in &mut summaries {
            summary.counts_exact = false;
        }
    }
    let mut result = TraceMatrixResult {
        format_version: 1,
        kind: "matrix".into(),
        selection: selection.clone(),
        evaluation_complete,
        summaries,
        records,
        has_more: false,
        next_cursor: None,
        markdown: None,
    };
    page(&snapshot, params, limit, &mut result)?;
    Ok(result)
}

fn empty_validation(project: &Project) -> ValidationResult {
    ValidationResult {
        format_version: 1,
        project: project.root().to_owned(),
        target: ValidationTarget {
            kind: ValidationTargetKind::Project,
            id: None,
        },
        valid: false,
        evaluation_complete: true,
        diagnostics: vec![],
        summary: ValidationSummary {
            errors: 0,
            warnings: 0,
            counts_exact: true,
        },
        selection: None,
        has_more: false,
        next_cursor: None,
        path: None,
        flavours: None,
        relations: None,
    }
}

fn unavailable_diagnostic(message: &str) -> ValidationDiagnostic {
    ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        ValidationScope::Project,
        DiagnosticLocation::default(),
        message,
    )
}

#[derive(Clone)]
struct EdgeEntry {
    edge: RelationEdge,
    occurrences: usize,
}

fn collect_edges(corpus: &Corpus, schema: &Schema) -> Result<Vec<EdgeEntry>, ValidationError> {
    let mut unique = BTreeMap::<String, EdgeEntry>::new();
    for author in corpus.items() {
        for relation in author.relations() {
            let edge = if let Some(address) = crate::external::address(relation.target()) {
                RelationEdge::external(schema, author, relation.name(), address)
                    .map_err(|e| ValidationError::invalid_argument(e.to_string()))
            } else {
                query::resolve_item(corpus, relation.target())
                    .map_err(|e| ValidationError::invalid_argument(e.to_string()))
                    .and_then(|target| {
                        RelationEdge::new(schema, author, relation.name(), target)
                            .map_err(|e| ValidationError::invalid_argument(e.to_string()))
                    })
            };
            let Ok(edge) = edge else { continue };
            let key = serde_json::to_string(&edge).expect("edge serializes");
            unique
                .entry(key)
                .and_modify(|e| e.occurrences += 1)
                .or_insert(EdgeEntry {
                    edge,
                    occurrences: 1,
                });
        }
    }
    Ok(unique.into_values().collect())
}

struct ExplainContext<'a> {
    rules: &'a Rules,
    schema: &'a Schema,
    corpus: &'a Corpus,
    descriptors: &'a BTreeMap<String, Value>,
    selected_mids: &'a BTreeSet<String>,
    edges: &'a [EdgeEntry],
    observation: &'a RuleObservation,
    root: &'a Value,
    evaluation: &'a Value,
    snapshot: &'a str,
}

struct ExplainRecords<'a> {
    seen: &'a mut BTreeSet<String>,
    records: &'a mut Vec<TraceRecord>,
}

fn explain(
    ctx: &ExplainContext<'_>,
    shape_id: &str,
    focus: &Item,
    parent: Option<&str>,
    context: &[Value],
    output: &mut ExplainRecords<'_>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let Some((shape, source)) = ctx.rules.shape(shape_id) else {
        return;
    };
    let observation = ctx.observation;
    let reference = check_reference(ctx.snapshot, observation, shape_id, focus, context);
    if !output.seen.insert(reference.clone()) {
        return;
    }
    let state = if parent.is_none() {
        observation.state
    } else {
        observation
            .states
            .get(&(shape_id.to_owned(), focus.mid().unwrap().to_owned()))
            .map_or(
                "unavailable",
                |passed| if *passed { "passed" } else { "failed" },
            )
    };
    let path = shape.get("path").filter(|p| !p.is_null());
    let (relation, direction, label) = path
        .and_then(|path| relation_path(path, ctx.schema))
        .map_or((None, None, None), |(a, b, c)| (Some(a), Some(b), Some(c)));
    let matching = relation
        .as_ref()
        .map(|name| {
            matching_edges(
                ctx.edges,
                name,
                direction.as_deref().unwrap(),
                focus.mid().unwrap(),
            )
        })
        .unwrap_or_default();
    let qualifier = shape["qualifiedValueShape"].as_str();
    let every_shape = shape["node"].as_str();
    let native_count = observation
        .counts
        .iter()
        .find_map(|((id, focus_id, component), count)| {
            let supported = if qualifier.is_some() {
                component.ends_with("#QualifiedMinCountConstraintComponent")
                    || component.ends_with("#QualifiedMaxCountConstraintComponent")
            } else {
                component.ends_with("#MinCountConstraintComponent")
                    || component.ends_with("#MaxCountConstraintComponent")
            };
            (id == shape_id
                && focus_id == &format!("urn:mara:mid:{}", focus.mid().unwrap())
                && supported)
                .then_some(count)
        });
    let selected_count = native_count
        .map(|count| count.0)
        .or_else(|| relation.as_ref().map(|_| matching.len()));
    let qualifying = qualifier.and_then(|q| {
        native_count.and_then(|count| count.1).or_else(|| {
            relation.as_ref()?;
            matching
                .iter()
                .map(|(_, endpoint)| {
                    observation
                        .states
                        .get(&(q.to_owned(), endpoint_state_key(endpoint)))
                        .map(|value| usize::from(*value))
                })
                .sum::<Option<usize>>()
        })
    });
    let qualifying = if qualifier.is_none()
        && (shape.get("minCount").is_some() || shape.get("maxCount").is_some())
    {
        selected_count
    } else {
        qualifying
    };
    let count = json!({"selected":selected_count,"qualifying":qualifying,
        "minimum":shape["qualifiedMinCount"].as_u64().or_else(|| shape["minCount"].as_u64()),
        "maximum":shape["qualifiedMaxCount"].as_u64().or_else(|| shape["maxCount"].as_u64())});
    let condition = json!({"components":shape.as_object().unwrap().iter()
        .filter(|(key,_)| matches!(key.as_str(),"property"|"class"|"datatype"|"hasValue"|
            "pattern"|"minCount"|"maxCount"|"qualifiedValueShape"|
            "qualifiedMinCount"|"qualifiedMaxCount"|"node"|"in"|"and"|"or"|"not"))
        .map(|(key,value)|(key.clone(),value.clone())).collect::<BTreeMap<_,_>>(),
        "path":path});
    let components = shape_components(shape);
    let every = every_shape.filter(|_| relation.is_some()).and_then(|q| {
        matching
            .iter()
            .map(|(_, end)| {
                observation
                    .states
                    .get(&(q.to_owned(), endpoint_state_key(end)))
                    .copied()
            })
            .collect::<Option<Vec<_>>>()
            .map(|values| values.iter().all(|v| *v))
    });
    let inspection = path.and_then(|path| {
        if relation.is_some() {
            return None;
        }
        let field = path
            .as_str()?
            .strip_prefix("field:")
            .unwrap_or(path.as_str()?);
        let mut values = focus.metadata().iter().filter(|entry| entry.key() == field);
        let first = values.next();
        let value_count = usize::from(first.is_some()) + values.count();
        let (value, value_truncated) = first.map_or((None, false), |entry| {
            let excerpt = entry.value().chars().take(256).collect::<String>();
            let truncated = entry.value().chars().count() > 256;
            (Some(excerpt), truncated)
        });
        Some(
            json!({"field":field,"item":focus.mid(),"item_id":focus.id(),"value":value,
            "value_truncated":value_truncated,"value_count":value_count,
            "source":first.map(|entry|DiagnosticLocation::source(entry.source()))}),
        )
    });
    output.records.push(record(
        json!({"kind":"check","reference":reference,"root":ctx.root,
        "evaluation":ctx.evaluation,"obligation":{"shape":shape_id,"component":components,"source":source},
        "context":context,"parent":parent,"state":state,"condition":condition,
        "counts":count,"every":every,"inspection":inspection,
        "reported":observation.diagnostic.as_ref().filter(|d|
            d.obligation.as_ref().is_some_and(|o|o.shape==shape_id))}),
    ));
    let local_shape = relation.is_none();
    if let (Some(name), Some(direction), Some(label)) = (relation, direction, label) {
        for (entry, endpoint) in &matching {
            let qualification = qualifier.and_then(|q| {
                observation
                    .states
                    .get(&(q.to_owned(), endpoint_state_key(endpoint)))
                    .copied()
            });
            let every_state = every_shape.and_then(|q| {
                observation
                    .states
                    .get(&(q.to_owned(), endpoint_state_key(endpoint)))
                    .copied()
            });
            let descriptor = endpoint_descriptor(endpoint, ctx.descriptors);
            let outside_selection = match endpoint {
                RelationEndpoint::Item { mid, .. } => !ctx.selected_mids.contains(mid),
                RelationEndpoint::External { .. } => true,
                RelationEndpoint::Code { .. } => true,
            };
            output.records.push(record(
                json!({"kind":"edge","check":reference,"edge":entry.edge,
                "label":label,"direction":direction,"endpoint":descriptor,
                "outside_selection":outside_selection,
                "qualification":qualification.map(|v|if v{"passed"}else{"failed"}),
                "every":every_state.map(|v|if v{"passed"}else{"failed"}),
                "occurrence_count":entry.occurrences,
                "inspection":{"source":entry.edge.source.id(),"relation":entry.edge.relation,
                    "target":match &entry.edge.target {
                        RelationEndpoint::Item{id,..}=>id.clone(),
                        RelationEndpoint::External{address}=>format!("external:{address}"),
                        RelationEndpoint::Code{reference}=>reference.clone(),
                    }}}),
            ));
            if let RelationEndpoint::Item { mid, .. } = endpoint
                && let Some(next) = ctx.corpus.items().find(|item| item.mid() == Some(mid))
            {
                let mut next_context = context.to_vec();
                next_context.push(json!(entry.edge));
                for next_shape in [qualifier, every_shape].into_iter().flatten() {
                    if observation
                        .states
                        .contains_key(&(next_shape.to_owned(), mid.clone()))
                    {
                        explain(
                            ctx,
                            next_shape,
                            next,
                            Some(&reference),
                            &next_context,
                            output,
                            depth + 1,
                        );
                    }
                }
            }
        }
        let _ = name;
    }
    for child in shape["property"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if observation
            .states
            .contains_key(&(child.to_owned(), focus.mid().unwrap().to_owned()))
        {
            explain(ctx, child, focus, Some(&reference), context, output, depth);
        }
    }
    if local_shape {
        let nested = shape["node"]
            .as_str()
            .into_iter()
            .chain(["and", "or"].into_iter().flat_map(|key| {
                shape[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
            }))
            .chain(shape["not"].as_str());
        for child in nested {
            if observation
                .states
                .contains_key(&(child.to_owned(), focus.mid().unwrap().to_owned()))
            {
                explain(ctx, child, focus, Some(&reference), context, output, depth);
            }
        }
    }
}

fn shape_components(shape: &Value) -> Vec<String> {
    let mut components = shape
        .as_object()
        .unwrap()
        .keys()
        .filter_map(|key| {
            let name = match key.as_str() {
                "property" => "Property",
                "class" => "Class",
                "datatype" => "Datatype",
                "hasValue" => "HasValue",
                "pattern" => "Pattern",
                "minCount" => "MinCount",
                "maxCount" => "MaxCount",
                "qualifiedMinCount" => "QualifiedMinCount",
                "qualifiedMaxCount" => "QualifiedMaxCount",
                "node" => "Node",
                "in" => "In",
                "and" => "And",
                "or" => "Or",
                "not" => "Not",
                _ => return None,
            };
            Some(format!(
                "http://www.w3.org/ns/shacl#{name}ConstraintComponent"
            ))
        })
        .collect::<Vec<_>>();
    components.sort();
    components
}

fn check_reference(
    snapshot: &str,
    observation: &RuleObservation,
    shape: &str,
    focus: &Item,
    context: &[Value],
) -> String {
    let bytes = serde_json::to_vec(&(
        snapshot,
        observation.root.as_str(),
        shape,
        focus.mid(),
        context,
    ))
    .unwrap();
    format!("check:{:x}", Sha256::digest(bytes))
}

fn relation_path(path: &Value, schema: &Schema) -> Option<(String, String, String)> {
    let (raw, incoming) = if let Some(raw) = path.as_str() {
        (raw, false)
    } else {
        (path["inversePath"].as_str()?, true)
    };
    let raw = raw.strip_prefix("schema:").unwrap_or(raw);
    let (canonical, definition, inverse_alias) = schema.resolve_relation(raw)?;
    let incoming = incoming ^ inverse_alias;
    let direction = if definition.symmetric {
        "symmetric"
    } else if incoming {
        "incoming"
    } else {
        "outgoing"
    };
    let label = if incoming {
        definition.inverse.as_deref().unwrap_or(canonical)
    } else {
        canonical
    };
    Some((canonical.to_owned(), direction.into(), label.into()))
}

fn matching_edges<'a>(
    edges: &'a [EdgeEntry],
    name: &str,
    direction: &str,
    mid: &str,
) -> Vec<(&'a EdgeEntry, &'a RelationEndpoint)> {
    let mut found = Vec::new();
    for entry in edges.iter().filter(|e| e.edge.relation == name) {
        let edge = &entry.edge;
        let source = matches!(&edge.source,RelationEndpoint::Item{mid:id,..} if id==mid);
        let target = matches!(&edge.target,RelationEndpoint::Item{mid:id,..} if id==mid);
        let endpoint = if direction == "symmetric" && edge.symmetric {
            if source {
                Some(&edge.target)
            } else if target {
                Some(&edge.source)
            } else {
                None
            }
        } else if direction == "outgoing" && !edge.symmetric && source {
            Some(&edge.target)
        } else if direction == "incoming" && !edge.symmetric && target {
            Some(&edge.source)
        } else {
            None
        };
        if let Some(endpoint) = endpoint {
            found.push((entry, endpoint));
        }
    }
    found.sort_by_key(|(_, end)| serde_json::to_string(end).unwrap());
    found
}

fn endpoint_descriptor(
    endpoint: &RelationEndpoint,
    descriptors: &BTreeMap<String, Value>,
) -> Value {
    match endpoint {
        RelationEndpoint::Item { mid, .. } => descriptors[mid].clone(),
        RelationEndpoint::External { address } => json!({"kind":"external","address":address}),
        RelationEndpoint::Code { reference } => json!({"kind":"code","reference":reference}),
    }
}

fn endpoint_state_key(endpoint: &RelationEndpoint) -> String {
    match endpoint {
        RelationEndpoint::Item { mid, .. } => mid.clone(),
        RelationEndpoint::External { address } => format!("external:{address}"),
        RelationEndpoint::Code { reference } => reference.clone(),
    }
}

fn page(
    snapshot: &str,
    params: &TraceMatrixParams,
    limit: usize,
    result: &mut TraceMatrixResult,
) -> Result<(), ValidationError> {
    let mut hash = Sha256::new();
    hash.update(b"trace-matrix-1-adapter-1");
    hash.update(snapshot);
    hash.update(
        serde_json::to_vec(&(
            &params.selection,
            &params.rules,
            &params.check,
            limit,
            &params.render,
        ))
        .expect("request serializes"),
    );
    let fingerprint = format!("{:x}", hash.finalize());
    let start = match &params.cursor {
        None => 0,
        Some(cursor) => {
            let (prefix, position) = cursor.rsplit_once(':').ok_or_else(stale_cursor)?;
            if prefix != format!("1:{fingerprint}") {
                return Err(stale_cursor());
            }
            position.parse::<usize>().map_err(|_| stale_cursor())?
        }
    };
    let all = std::mem::take(&mut result.records);
    if params.cursor.is_some() && (start == 0 || start >= all.len()) {
        return Err(stale_cursor());
    }
    for record in all.iter().skip(start).take(limit) {
        result.records.push(record.clone());
        result.has_more = start + result.records.len() < all.len();
        result.next_cursor = result
            .has_more
            .then(|| format!("1:{fingerprint}:{}", start + result.records.len()));
        if params.render.as_deref() == Some("markdown") {
            result.markdown = Some(markdown(result));
        }
        if serde_json::to_vec(result).unwrap().len() > 65_536 {
            result.records.pop();
            if result.records.is_empty() {
                return Err(ValidationError::new(
                    "output_limit",
                    "a matrix record exceeds the 65536-byte page budget; shorten its source identity or location",
                ));
            }
            result.has_more = true;
            result.next_cursor = Some(format!("1:{fingerprint}:{}", start + result.records.len()));
            if params.render.as_deref() == Some("markdown") {
                result.markdown = Some(markdown(result));
            }
            break;
        }
    }
    if params.render.as_deref() == Some("markdown") {
        result.markdown = Some(markdown(result));
    }
    if serde_json::to_vec(result).unwrap().len() > 65_536 {
        return Err(ValidationError::new(
            "output_limit",
            "matrix envelope exceeds the 65536-byte page budget; narrow the selection or shorten source identities",
        ));
    }
    Ok(())
}

fn snapshot_id(
    project: &Project,
    schema: &Schema,
    corpus: &Corpus,
    rules: &Rules,
    prerequisites: &[crate::Diagnostic],
) -> String {
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(schema).expect("schema serializes"));
    for path in [
        project.root().join(crate::PROJECT_FILE),
        project.schema_path().to_owned(),
    ]
    .into_iter()
    .chain(rules.files.iter().cloned())
    {
        hash.update(
            path.strip_prefix(project.root())
                .unwrap_or(&path)
                .as_os_str()
                .as_encoded_bytes(),
        );
        if let Ok(bytes) = fs::read(&path) {
            hash.update(bytes)
        }
    }
    for document in corpus.documents() {
        hash.update(document.path().as_os_str().as_encoded_bytes());
        hash.update(document.source().as_bytes());
    }
    let retained = corpus
        .documents()
        .iter()
        .map(|document| document.path())
        .collect::<BTreeSet<_>>();
    let mut excluded = BTreeSet::new();
    for diagnostic in prerequisites {
        hash.update(
            serde_json::to_vec(&ValidationDiagnostic::from_source(diagnostic))
                .expect("diagnostic serializes"),
        );
        let path = diagnostic.source().path();
        if !retained.contains(path) {
            excluded.insert(path);
        }
    }
    for path in excluded {
        hash.update(b"excluded-source");
        hash.update(path.as_os_str().as_encoded_bytes());
        if let Ok(bytes) = fs::read(project.root().join(path)) {
            hash.update(bytes);
        }
    }
    format!("{:x}", hash.finalize())
}

fn stale_cursor() -> ValidationError {
    ValidationError::new(
        "stale_cursor",
        "invalid or stale matrix cursor; restart without a cursor",
    )
}

pub fn markdown(result: &TraceMatrixResult) -> String {
    let mut out = String::from(
        "# Traceability matrix\n\nCanonical links are relative to the project root. Save this page there to follow them.\n\n",
    );
    out.push_str(&format!(
        "Selection: `{}`. Evaluation complete: **{}**. More records: **{}**.\n\n",
        serde_json::to_string(&result.selection).unwrap(),
        result.evaluation_complete,
        result.has_more
    ));
    for summary in &result.summaries {
        out.push_str(&format!("- `{}`: selected {}, applicable {}, passed {}, failed {}, not applicable {}, unavailable {}; counts exact {}.\n",
            summary.evaluation["shape"].as_str().unwrap_or("?"),summary.selected,
            summary.passed+summary.failed,summary.passed,summary.failed,summary.not_applicable,
            summary.unavailable,summary.counts_exact));
    }
    out.push_str("\n| Kind | State | Source or endpoint | Detail |\n|---|---|---|---|\n");
    for typed in &result.records {
        let record = serde_json::to_value(typed).expect("trace record serializes");
        match record["kind"].as_str().unwrap_or("") {
            "result" => {
                let root = &record["root"];
                let id = root["id"].as_str().unwrap_or("?");
                let path = root["source"]["path"].as_str().unwrap_or("");
                let line = root["source"]["start_line"].as_u64().unwrap_or(0);
                out.push_str(&format!(
                    "\n## [{id}](<{}>) (line {line}) — {} — `{}`\n\n| Kind | State | Source or endpoint | Detail |\n|---|---|---|---|\n",
                    md_target(path),
                    record["state"].as_str().unwrap_or("?"),
                    record["evaluation"]["shape"].as_str().unwrap_or("?")
                ));
            }
            "check" => {
                let shape = record["obligation"]["shape"].as_str().unwrap_or("?");
                let source = &record["obligation"]["source"];
                let path = source["path"].as_str().unwrap_or("");
                let line = source["line"].as_u64().unwrap_or(0);
                let inspected = &record["inspection"];
                let detail = if inspected.is_object() {
                    let value = if inspected["value"].is_null() {
                        "missing".to_owned()
                    } else {
                        format!(
                            "{}{}",
                            md_cell(&inspected["value"].to_string()),
                            if inspected["value_truncated"] == true {
                                "…"
                            } else {
                                ""
                            }
                        )
                    };
                    let source = &inspected["source"];
                    let location = if source.is_object() {
                        format!(
                            " at [{}](<{}>) (line {})",
                            md_cell(inspected["item_id"].as_str().unwrap_or("item")),
                            md_target(source["path"].as_str().unwrap_or("")),
                            source["line"].as_u64().unwrap_or(0)
                        )
                    } else {
                        String::new()
                    };
                    let count = inspected["value_count"].as_u64().unwrap_or(0);
                    format!(
                        "; {} = {}{} ({})",
                        md_cell(inspected["field"].as_str().unwrap_or("?")),
                        value,
                        location,
                        match count {
                            0 => "0 values".to_owned(),
                            1 => "1 value".to_owned(),
                            _ => format!("first of {count} values"),
                        }
                    )
                } else {
                    String::new()
                };
                out.push_str(&format!("| Check | {} | [{shape}](<{}>) (line {line}) | selected {}, qualifying {}, min {}, max {}{detail} |\n",
                    record["state"].as_str().unwrap_or("?"),md_target(path),
                    show(&record["counts"]["selected"]),show(&record["counts"]["qualifying"]),
                    show(&record["counts"]["minimum"]),show(&record["counts"]["maximum"])));
            }
            "edge" => {
                let end = &record["endpoint"];
                let dest = if end["kind"] == "external" {
                    let address = end["address"].as_str().unwrap_or("");
                    format!("external [{address}](<{address}>) (terminal)")
                } else {
                    let id = end["id"].as_str().unwrap_or("?");
                    let path = end["source"]["path"].as_str().unwrap_or("");
                    format!("[{id}](<{}>)", md_target(path))
                };
                out.push_str(&format!(
                    "| Edge | {} | {dest}{} | qualification {}, every {}; assertions {} |\n",
                    record["label"].as_str().unwrap_or("?"),
                    if record["outside_selection"] == true {
                        " (outside root selection)"
                    } else {
                        ""
                    },
                    show(&record["qualification"]),
                    show(&record["every"]),
                    show(&record["occurrence_count"])
                ));
            }
            "issue" => out.push_str(&format!(
                "| Issue | unavailable | | {} |\n",
                record["diagnostic"]["message"]
                    .as_str()
                    .unwrap_or("evaluation unavailable")
            )),
            _ => (),
        }
    }
    if result.has_more {
        out.push_str(&format!(
            "\nContinued: use next_cursor `{}` with unchanged options.\n",
            result.next_cursor.as_deref().unwrap_or("")
        ));
    }
    out
}
fn show(value: &Value) -> String {
    if value.is_null() {
        "unknown".into()
    } else {
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
    }
}
fn md_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
}
fn md_target(path: &str) -> String {
    path.replace('%', "%25")
        .replace(' ', "%20")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('#', "%23")
        .replace('?', "%3F")
}

fn record(value: Value) -> TraceRecord {
    serde_json::from_value(value).expect("matrix record matches its public shape")
}
