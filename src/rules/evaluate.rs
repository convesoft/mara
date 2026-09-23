use super::*;
use crate::{
    Corpus, Diagnostic, DiagnosticItem, DiagnosticObligation, FieldType, Item, ValidationResult,
};
use rudof_iri::IriS;
use rudof_rdf::{rdf_core::term::Object, rdf_impl::OxigraphInMemory};
use shacl::validator::{
    RecursionSemantics,
    bounded::{self, Control},
    engine::{NativeEngine, Validate},
    nodes::FocusNodes,
    report::ValidationOutcome,
};

impl Rules {
    pub fn evaluate(
        &self,
        corpus: &Corpus,
        schema: &Schema,
        prerequisites: &[Diagnostic],
        result: &mut ValidationResult,
        work: &mut WorkBudget,
    ) {
        let Some(ir) = &self.ir else {
            return;
        };
        if self.roots.is_empty() || work.exhausted {
            return;
        }
        let mut control = Control {
            used: work.used,
            limit: work.limit,
            patterns: self.patterns.clone(),
            ..Default::default()
        };
        let mut graph = Vec::new();
        let mut identity: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
        for item in corpus.items() {
            identity.entry(item.id()).or_default().push(item);
            if let Some(mid) = item.mid() {
                identity.entry(mid).or_default().push(item);
            }
        }
        for item in corpus.items() {
            if !work.charge(1) {
                return;
            }
            let Some(mid) = item.mid() else {
                continue;
            };
            let iri = format!("urn:mara:mid:{mid}");
            let invalid = !corpus.is_complete()
                || !item.validation_source_is_complete()
                || prerequisites.iter().any(|d| contains(item, d));
            if invalid {
                control.unavailable.insert(iri.clone());
            }
            let mut node = json!({"@id":iri,"@type":format!("{FLAVOUR}{}",item.flavour())});
            if !invalid && let Some(flavour) = schema.flavours.get(item.flavour()) {
                for field in item.metadata() {
                    if let Some(def) = flavour.fields.get(field.key()) {
                        if !work.charge(1 + field.value().len()) {
                            return;
                        }
                        match literal(field.value(), def.field_type) {
                            Some(value) => {
                                *control
                                    .path_work
                                    .entry((iri.clone(), format!("{FIELD}{}", field.key()), false))
                                    .or_default() += 1 + field.value().len();
                                push(&mut node, &format!("{FIELD}{}", field.key()), value)
                            }
                            None => {
                                control.unavailable.insert(iri.clone());
                            }
                        }
                    }
                }
            }
            graph.push(node);
        }
        let mut edges = BTreeSet::new();
        for item in corpus.items() {
            for relation in item.relations() {
                if !work.charge(1 + relation.target().len() + relation.name().len()) {
                    return;
                }
                let target = identity
                    .get(relation.target())
                    .and_then(|v| if v.len() == 1 { Some(v[0]) } else { None });
                let edge = target
                    .and_then(|t| crate::RelationEdge::new(schema, item, relation.name(), t).ok());
                match edge {
                    Some(edge) => {
                        let crate::RelationEndpoint::Item { mid: a, .. } = &edge.source;
                        let crate::RelationEndpoint::Item { mid: b, .. } = &edge.target;
                        edges.insert((a.clone(), edge.relation.clone(), b.clone()));
                        if edge.symmetric {
                            edges.insert((b.clone(), edge.relation, a.clone()));
                        }
                    }
                    None => {
                        if let Some((name, _, _)) = schema.resolve_relation(relation.name()) {
                            control.invalid_paths.insert(format!("{REL}{name}"));
                        }
                    }
                }
            }
        }
        for (a, relation, b) in &edges {
            *control
                .path_work
                .entry((
                    format!("urn:mara:mid:{a}"),
                    format!("{REL}{relation}"),
                    false,
                ))
                .or_default() += 1 + "urn:mara:mid:".len() + b.len();
            *control
                .path_work
                .entry((
                    format!("urn:mara:mid:{b}"),
                    format!("{REL}{relation}"),
                    true,
                ))
                .or_default() += 1 + "urn:mara:mid:".len() + a.len();
            graph.push(json!({"@id":format!("urn:mara:mid:{a}"),format!("{REL}{relation}"):[{"@id":format!("urn:mara:mid:{b}")}]}));
        }
        let data = match OxigraphInMemory::from_str(
            &json!({"@graph":graph}).to_string(),
            &RDFFormat::JsonLd,
            None,
            &ReaderMode::Strict,
        ) {
            Ok(data) => data,
            Err(_) => {
                result.evaluation_complete = false;
                result.diagnostics.push(ValidationDiagnostic::new(
                    DiagnosticCode::EvaluationUnavailable,
                    Severity::Error,
                    ValidationScope::Project,
                    DiagnosticLocation::default(),
                    "could not project typed item data for rule evaluation",
                ));
                return;
            }
        };
        let mut order = Vec::new();
        for (id, shape) in &self.shapes {
            order.push((
                shape.source.path.clone(),
                shape.source.start_byte,
                id.clone(),
                String::new(),
            ));
            for (key, location) in &shape.locations {
                let component = component(key);
                if !component.is_empty() {
                    order.push((
                        location.path.clone(),
                        location.start_byte,
                        id.clone(),
                        component,
                    ));
                }
            }
        }
        order.sort();
        for (rank, (_, _, id, component)) in order.into_iter().enumerate() {
            control.order.insert((id, component), rank);
        }
        control.used = work.used;
        let selected = result.target.id.clone();
        for item in corpus.items().filter(|i| {
            selected
                .as_deref()
                .is_none_or(|id| i.id() == id || i.mid() == Some(id))
        }) {
            for root in &self.roots {
                if !work.charge(1) {
                    return;
                }
                let shape = &self.shapes[root];
                if !strings(&shape.value["targetClass"])
                    .iter()
                    .any(|f| f == item.flavour())
                {
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
                    continue;
                }
                let Some(mid) = item.mid() else {
                    self.unavailable(result, item, root, "selected item has no valid MID");
                    continue;
                };
                let focus =
                    Object::iri(IriS::new(&format!("urn:mara:mid:{mid}")).expect("validated MID"));
                control.used = work.used;
                control.counts.clear();
                let ((condition, outcome), returned) = bounded::run(control, || {
                    let mut engine = NativeEngine::new(RecursionSemantics::default());
                    let run = |id: &str, engine: &mut NativeEngine| {
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
                    let condition = shape.value["whenShape"]
                        .as_str()
                        .map(|id| run(id, &mut engine));
                    let applies = condition
                        .as_ref()
                        .is_none_or(|c| c.as_ref().is_ok_and(ValidationOutcome::conforms));
                    bounded::reset_trace();
                    let outcome = if applies {
                        Some(run(root, &mut engine))
                    } else {
                        None
                    };
                    (condition, outcome)
                });
                control = returned;
                work.used = control.used;
                work.exhausted = control.exhausted;
                if work.exhausted {
                    return;
                }
                if condition.as_ref().is_some_and(Result::is_err)
                    || outcome.as_ref().is_some_and(Result::is_err)
                {
                    self.unavailable(
                        result,
                        item,
                        root,
                        "rule prerequisite is invalid or native SHACL evaluation failed",
                    );
                    continue;
                }
                if let Some(Ok(outcome)) = outcome
                    && !outcome.conforms()
                {
                    let mut failures = outcome.violations().iter().collect::<Vec<_>>();
                    failures.sort_by_key(|f| {
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
                        )
                    });
                    let (failure, mut context) = first_leaf(failures[0], &control, Vec::new());
                    for entry in &mut context {
                        let Some(parent) =
                            entry["shape"].as_str().and_then(|id| self.shapes.get(id))
                        else {
                            continue;
                        };
                        let path = &parent.value["path"];
                        let Some(raw) = path.as_str().or_else(|| path["inversePath"].as_str())
                        else {
                            continue;
                        };
                        let Ok(resolved) =
                            bindings::resolve_path(raw, &serde_json::to_value(schema).unwrap())
                        else {
                            continue;
                        };
                        let Some(name) = resolved.strip_prefix(REL) else {
                            continue;
                        };
                        let definition = &schema.relations[name];
                        let direction = if definition.symmetric {
                            "symmetric"
                        } else if path.is_object() {
                            "incoming"
                        } else {
                            "outgoing"
                        };
                        let endpoint = |key: &str| {
                            entry[key]
                                .as_str()
                                .and_then(|s| s.strip_prefix("urn:mara:mid:"))
                                .and_then(|mid| identity.get(mid))
                                .and_then(|items| items.first().copied())
                        };
                        let edge = endpoint("focus").zip(endpoint("value")).and_then(|(a, b)| {
                            if path.is_object() {
                                crate::RelationEdge::new(schema, b, name, a).ok()
                            } else {
                                crate::RelationEdge::new(schema, a, name, b).ok()
                            }
                        });
                        entry["relation"] = json!(name);
                        entry["direction"] = json!(direction);
                        entry["label"] = json!(if direction == "incoming" {
                            definition.inverse.as_deref().unwrap_or(name)
                        } else {
                            name
                        });
                        entry["edge"] = json!(edge);
                    }
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
                        format!("rule {root} failed: {key}"),
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
                    let counts = control
                        .counts
                        .get(&(id.clone(), failure.focus_node().to_string()));
                    let (selected, qualifying) = if let Some((s, q)) = counts {
                        (Some(*s), Some(*q))
                    } else {
                        (None, None)
                    };
                    let mut details = json!({"kind":kind(key),"path":obligation.value["path"],"parameter":obligation.value[key],"selected_count":selected,"qualifying_count":qualifying,"focus":failure.focus_node().to_string(),"value":failure.value().map(ToString::to_string),"context":context});
                    if let Some(path) = obligation.value.get("path") {
                        let raw = path
                            .as_str()
                            .or_else(|| path["inversePath"].as_str())
                            .unwrap_or("");
                        if let Ok(resolved) =
                            bindings::resolve_path(raw, &serde_json::to_value(schema).unwrap())
                            && let Some(name) = resolved.strip_prefix(REL)
                        {
                            let r = &schema.relations[name];
                            let direction = if r.symmetric {
                                "symmetric"
                            } else if path.is_object() {
                                "incoming"
                            } else {
                                "outgoing"
                            };
                            details["relation"] = json!(name);
                            details["direction"] = json!(direction);
                            details["label"] = json!(if direction == "incoming" {
                                r.inverse.as_deref().unwrap_or(name)
                            } else {
                                name
                            });
                        }
                    }
                    diagnostic.details = Some(details);
                    result.diagnostics.push(diagnostic);
                }
            }
        }
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
fn contains(item: &Item, d: &Diagnostic) -> bool {
    d.source().path() == item.source().path()
        && d.source().span().start_byte() >= item.source().span().start_byte()
        && d.source().span().start_byte() < item.source().span().end_byte()
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

fn first_leaf(
    failure: &shacl::validator::report::ValidationResult,
    control: &Control,
    mut context: Vec<Value>,
) -> (shacl::validator::report::ValidationResult, Vec<Value>) {
    let key = (
        failure
            .source()
            .map(ToString::to_string)
            .unwrap_or_default(),
        failure.focus_node().to_string(),
        failure.constraint_component().to_string(),
    );
    if let Some(children) = control.explanations.get(&key).filter(|c| !c.is_empty()) {
        context.push(json!({"shape":key.0,"focus":key.1,"component":key.2,"path":failure.path().map(ToString::to_string),"value":failure.value().map(ToString::to_string)}));
        let child = children
            .iter()
            .min_by_key(|c| {
                let key = (
                    c.source().map(ToString::to_string).unwrap_or_default(),
                    c.focus_node().to_string(),
                    c.constraint_component().to_string(),
                );
                control
                    .events
                    .iter()
                    .position(|k| k == &key)
                    .unwrap_or(usize::MAX)
            })
            .unwrap();
        return first_leaf(child, control, context);
    }
    (failure.clone(), context)
}
