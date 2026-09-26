use super::engine::RuleEngine;
use super::*;
use crate::{
    Corpus, Diagnostic, DiagnosticItem, DiagnosticObligation, FieldType, Item, ValidationResult,
};
use rudof_iri::IriS;
use rudof_rdf::{rdf_core::term::Object, rdf_impl::OxigraphInMemory};
use shacl::validator::{engine::Validate, nodes::FocusNodes};

impl Rules {
    pub fn evaluate(
        &self,
        corpus: &Corpus,
        schema: &Schema,
        prerequisites: &[Diagnostic],
        result: &mut ValidationResult,
    ) {
        self.observe(corpus, schema, prerequisites, result, &self.roots, None);
    }

    pub(crate) fn observe(
        &self,
        corpus: &Corpus,
        schema: &Schema,
        prerequisites: &[Diagnostic],
        result: &mut ValidationResult,
        roots: &[String],
        selected_mids: Option<&BTreeSet<String>>,
    ) -> Vec<RuleObservation> {
        let mut observations = Vec::new();
        let Some(ir) = &self.ir else {
            return observations;
        };
        if roots.is_empty() {
            return observations;
        }
        // Policy uses the complete graph, including for item validation. Reject
        // invalid prerequisites before projecting them into apparent absence.
        if !prerequisites.is_empty()
            || !corpus.is_complete()
            || corpus.items().any(|i| !i.validation_source_is_complete())
        {
            project_unavailable(
                result,
                "rule evaluation skipped: fix the corpus source, identity, field or reference diagnostics first",
            );
            return observations;
        }
        let mut graph = Vec::new();
        let mut identity = BTreeMap::new();
        for item in corpus.items() {
            identity.insert(item.id(), item);
            if let Some(mid) = item.mid() {
                identity.insert(mid, item);
            }
        }
        for item in corpus.items() {
            let Some(mid) = item.mid() else {
                project_unavailable(result, "rule evaluation requires valid item identities");
                return observations;
            };
            let mut node = json!({"@id":format!("urn:mara:mid:{mid}"),"@type":format!("{FLAVOUR}{}",item.flavour())});
            if let Some(flavour) = schema.flavours.get(item.flavour()) {
                for field in item.metadata() {
                    if let Some(def) = flavour.fields.get(field.key()) {
                        let Some(value) = literal(field.value(), def.field_type) else {
                            project_unavailable(
                                result,
                                "rule evaluation requires representable typed field values",
                            );
                            return observations;
                        };
                        push(&mut node, &format!("{FIELD}{}", field.key()), value);
                    }
                }
            }
            graph.push(node);
        }
        let mut edges = BTreeSet::new();
        for item in corpus.items() {
            for relation in item.relations() {
                let edge = if let Some(address) = crate::external::address(relation.target()) {
                    crate::RelationEdge::external(schema, item, relation.name(), address).ok()
                } else {
                    identity.get(relation.target()).and_then(|target| {
                        crate::RelationEdge::new(schema, item, relation.name(), target).ok()
                    })
                };
                let Some(edge) = edge else {
                    project_unavailable(
                        result,
                        "rule evaluation requires resolved, valid relationships",
                    );
                    return observations;
                };
                let crate::RelationEndpoint::Item { mid: a, .. } = &edge.source else {
                    unreachable!()
                };
                let b = match &edge.target {
                    crate::RelationEndpoint::Item { mid, .. } => format!("urn:mara:mid:{mid}"),
                    crate::RelationEndpoint::External { address } => format!(
                        "urn:mara:external:{}",
                        url::form_urlencoded::byte_serialize(address.as_bytes())
                            .collect::<String>()
                    ),
                    crate::RelationEndpoint::Code { reference } => {
                        format!("urn:mara:code:{reference}")
                    }
                };
                let a = format!("urn:mara:mid:{a}");
                edges.insert((a.clone(), edge.relation.clone(), b.clone()));
                if edge.symmetric {
                    edges.insert((b, edge.relation, a));
                }
            }
        }
        for (a, relation, b) in &edges {
            graph.push(json!({"@id":a,format!("{REL}{relation}"):[{"@id":b}]}));
        }
        let data = match OxigraphInMemory::from_str(
            &json!({"@graph":graph}).to_string(),
            &RDFFormat::JsonLd,
            None,
            &ReaderMode::Strict,
        ) {
            Ok(data) => data,
            Err(_) => {
                project_unavailable(
                    result,
                    "could not project typed item data for rule evaluation",
                );
                return observations;
            }
        };
        let selected = result.target.id.clone();
        for item in corpus.items().filter(|i| {
            selected
                .as_deref()
                .is_none_or(|id| i.id() == id || i.mid() == Some(id))
                && selected_mids.is_none_or(|mids| i.mid().is_some_and(|mid| mids.contains(mid)))
        }) {
            for root in roots {
                let shape = &self.shapes[root];
                let mid = item.mid().expect("validated identity").to_owned();
                let record = |state, diagnostic, counts, states| RuleObservation {
                    mid: mid.clone(),
                    root: root.clone(),
                    state,
                    diagnostic,
                    counts,
                    states,
                };
                if !shape.value["targetClass"].is_null()
                    && !strings(&shape.value["targetClass"])
                        .iter()
                        .any(|f| f == item.flavour())
                {
                    observations.push(record(
                        "not_applicable",
                        None,
                        BTreeMap::new(),
                        BTreeMap::new(),
                    ));
                    continue;
                }
                let paths = crate::query::normalized_paths(
                    &strings(&shape.value["paths"])
                        .iter()
                        .map(PathBuf::from)
                        .collect::<Vec<_>>(),
                )
                .expect("validated scope paths");
                if !paths.is_empty() && !paths.iter().any(|p| item.source().path().starts_with(p)) {
                    observations.push(record(
                        "not_applicable",
                        None,
                        BTreeMap::new(),
                        BTreeMap::new(),
                    ));
                    continue;
                }
                let focus =
                    Object::iri(IriS::new(&format!("urn:mara:mid:{mid}")).expect("validated MID"));
                let run = |id: &str, engine: &mut RuleEngine| {
                    let s = ir
                        .get_shape(&Object::iri(
                            IriS::new(id).expect("validated shape identity"),
                        ))
                        .expect("compiled shape");
                    s.validate(
                        &data,
                        engine,
                        Some(&FocusNodes::single(focus.clone().into())),
                        None,
                        ir,
                    )
                };
                if let Some(condition) = shape.value["whenShape"].as_str() {
                    let mut engine = RuleEngine::new();
                    match run(condition, &mut engine) {
                        Ok(outcome) if !engine.failed.get() => {
                            if !outcome.conforms() {
                                observations.push(record(
                                    "not_applicable",
                                    None,
                                    BTreeMap::new(),
                                    BTreeMap::new(),
                                ));
                                continue;
                            }
                        }
                        _ => {
                            self.unavailable(
                                result,
                                item,
                                root,
                                "native SHACL applicability evaluation failed",
                            );
                            observations.push(record(
                                "unavailable",
                                None,
                                BTreeMap::new(),
                                BTreeMap::new(),
                            ));
                            continue;
                        }
                    }
                }
                let mut engine = RuleEngine::new();
                let outcome = match run(root, &mut engine) {
                    Ok(outcome) if !engine.failed.get() => outcome,
                    _ => {
                        self.unavailable(
                            result,
                            item,
                            root,
                            "native SHACL obligation evaluation failed",
                        );
                        observations.push(record(
                            "unavailable",
                            None,
                            BTreeMap::new(),
                            BTreeMap::new(),
                        ));
                        continue;
                    }
                };
                if outcome.conforms() {
                    let states = cached_states(&engine, ir, &self.shapes, corpus);
                    observations.push(record("passed", None, engine.counts, states));
                    continue;
                }
                // Select a stable reported violation, without claiming an
                // exhaustive explanation of the engine's internal evaluation.
                let failure = outcome
                    .violations()
                    .iter()
                    .min_by_key(|f| {
                        let id = f.source().map(ToString::to_string).unwrap_or_default();
                        let component = f.constraint_component().to_string();
                        let loc = self
                            .shapes
                            .get(&id)
                            .map(|s| s.location(component_key(&component)));
                        (
                            loc.as_ref().and_then(|l| l.path.clone()),
                            loc.as_ref().and_then(|l| l.start_byte),
                            id,
                            component,
                            f.focus_node().to_string(),
                            f.value().map(ToString::to_string),
                        )
                    })
                    .expect("nonconforming outcome has a violation");
                let id = failure
                    .source()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| root.clone());
                let component = failure.constraint_component().to_string();
                let key = component_key(&component);
                let obligation = self.shapes.get(&id).unwrap_or(shape);
                let mut diagnostic = ValidationDiagnostic::new(
                    DiagnosticCode::RuleFailed,
                    if shape.value["severity"] == "Warning" {
                        Severity::Warning
                    } else {
                        Severity::Error
                    },
                    ValidationScope::Item,
                    DiagnosticLocation::source(item.source()),
                    obligation.value["message"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("rule {root} failed: {key}")),
                );
                diagnostic.item = Some(DiagnosticItem {
                    id: item.id().into(),
                    mid: item.mid().map(str::to_owned),
                });
                diagnostic.rule = Some(root.clone());
                diagnostic.obligation = Some(DiagnosticObligation {
                    shape: id.clone(),
                    component: component.clone(),
                    source: obligation.location(key),
                });
                let counts = engine
                    .counts
                    .get(&(id, failure.focus_node().to_string(), component));
                let mut details = json!({"kind":kind(key),"path":obligation.value["path"],"parameter":obligation.value[key],"selected_count":counts.map(|c|c.0),"qualifying_count":counts.and_then(|c|c.1),"focus":failure.focus_node().to_string(),"value":failure.value().map(ToString::to_string)});
                if let Some(path) = obligation.value.get("path") {
                    let raw = path
                        .as_str()
                        .or_else(|| path["inversePath"].as_str())
                        .unwrap_or("");
                    if let Ok(resolved) =
                        bindings::resolve_path(raw, &serde_json::to_value(schema).unwrap())
                        && let Some(name) = resolved.strip_prefix(REL)
                    {
                        let relation = &schema.relations[name];
                        let direction = if relation.symmetric {
                            "symmetric"
                        } else if path.is_object() {
                            "incoming"
                        } else {
                            "outgoing"
                        };
                        details["relation"] = json!(name);
                        details["direction"] = json!(direction);
                        details["label"] = json!(if direction == "incoming" {
                            relation.inverse.as_deref().unwrap_or(name)
                        } else {
                            name
                        });
                    }
                }
                diagnostic.details = Some(details);
                let states = cached_states(&engine, ir, &self.shapes, corpus);
                observations.push(record(
                    "failed",
                    Some(diagnostic.clone()),
                    engine.counts,
                    states,
                ));
                result.diagnostics.push(diagnostic);
            }
        }
        observations
    }
    fn unavailable(&self, result: &mut ValidationResult, item: &Item, root: &str, message: &str) {
        result.evaluation_complete = false;
        let mut d = ValidationDiagnostic::new(
            DiagnosticCode::EvaluationUnavailable,
            Severity::Error,
            ValidationScope::Item,
            DiagnosticLocation::source(item.source()),
            message,
        );
        d.rule = Some(root.into());
        d.item = Some(DiagnosticItem {
            id: item.id().into(),
            mid: item.mid().map(str::to_owned),
        });
        result.diagnostics.push(d);
    }
}
fn cached_states(
    engine: &RuleEngine,
    ir: &shacl::ir::IRSchema,
    shapes: &BTreeMap<String, Shape>,
    corpus: &Corpus,
) -> BTreeMap<(String, String), bool> {
    use shacl::validator::engine::Engine;
    let mut states = BTreeMap::new();
    let external_addresses = corpus
        .items()
        .flat_map(|item| item.relations())
        .filter_map(|relation| crate::external::address(relation.target()))
        .collect::<BTreeSet<_>>();
    for id in shapes.keys() {
        let Ok(iri) = IriS::new(id) else { continue };
        let Some(idx) = ir.get_idx(&Object::iri(iri)) else {
            continue;
        };
        for item in corpus.items() {
            let Some(mid) = item.mid() else { continue };
            let Ok(focus) = IriS::new(&format!("urn:mara:mid:{mid}")) else {
                continue;
            };
            if let Some(outcome) = engine.get_cached_outcome(&Object::iri(focus), *idx) {
                states.insert((id.clone(), mid.to_owned()), outcome.conforms());
            }
        }
        for address in &external_addresses {
            let encoded =
                url::form_urlencoded::byte_serialize(address.as_bytes()).collect::<String>();
            let Ok(focus) = IriS::new(&format!("urn:mara:external:{encoded}")) else {
                continue;
            };
            if let Some(outcome) = engine.get_cached_outcome(&Object::iri(focus), *idx) {
                states.insert(
                    (id.clone(), format!("external:{address}")),
                    outcome.conforms(),
                );
            }
        }
    }
    states
}
fn project_unavailable(result: &mut ValidationResult, message: &str) {
    result.evaluation_complete = false;
    result.diagnostics.push(ValidationDiagnostic::new(
        DiagnosticCode::EvaluationUnavailable,
        Severity::Error,
        ValidationScope::Project,
        DiagnosticLocation::default(),
        message,
    ));
}
fn push(node: &mut Value, key: &str, value: Value) {
    node.as_object_mut()
        .unwrap()
        .entry(key)
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap()
        .push(value);
}
fn literal(raw: &str, t: FieldType) -> Option<Value> {
    match t {
        FieldType::String | FieldType::Enum => Some(json!(raw)),
        FieldType::Integer => raw.parse::<i64>().ok().map(|n| json!(n)),
        FieldType::Number => raw
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| json!({"@value":n,"@type":format!("{XSD}double")})),
        FieldType::Boolean => raw.parse::<bool>().ok().map(|b| json!(b)),
    }
}
fn component(key: &str) -> String {
    let name = match key {
        "class" => "Class",
        "datatype" => "Datatype",
        "hasValue" => "HasValue",
        "pattern" => "Pattern",
        "minCount" => "MinCount",
        "maxCount" => "MaxCount",
        "qualifiedMinCount" => "QualifiedMinCount",
        "qualifiedMaxCount" => "QualifiedMaxCount",
        "qualifiedValueShape" => "QualifiedValueShape",
        "node" => "Node",
        "and" => "And",
        "or" => "Or",
        "not" => "Not",
        "in" => "In",
        _ => return String::new(),
    };
    format!("{SH}{name}ConstraintComponent")
}
fn component_key(component: &str) -> &'static str {
    [
        "class",
        "datatype",
        "hasValue",
        "pattern",
        "minCount",
        "maxCount",
        "qualifiedMinCount",
        "qualifiedMaxCount",
        "node",
        "and",
        "or",
        "not",
        "in",
    ]
    .into_iter()
    .find(|key| self::component(key) == component)
    .unwrap_or("node")
}
fn kind(key: &str) -> &'static str {
    match key {
        "class" => "class",
        "datatype" => "datatype",
        "hasValue" => "has_value",
        "pattern" => "pattern",
        "minCount" | "qualifiedMinCount" => "minimum",
        "maxCount" | "qualifiedMaxCount" => "maximum",
        "node" => "every",
        "and" => "and",
        "or" => "or",
        "not" => "not",
        _ => "in",
    }
}
