//! Project-owned current-state policy. YAML is converted with generated JSON-LD
//! bindings and evaluated by the pinned native SHACL engine.
mod bindings;
mod engine;
mod evaluate;
mod yaml;
use crate::{
    DiagnosticCode, DiagnosticLocation, Project, Schema, Severity, ValidationDiagnostic,
    ValidationScope,
};
use rudof_rdf::{rdf_core::RDFFormat, rdf_impl::ReaderMode};
use serde_json::{Value, json};
use shacl::ir::IRSchema;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};
const SH: &str = "http://www.w3.org/ns/shacl#";
const FIELD: &str = "urn:mara:field:";
const REL: &str = "urn:mara:relation:";
const FLAVOUR: &str = "urn:mara:flavour:";
const RULE: &str = "urn:mara:rule:";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

#[derive(Clone)]
struct Shape {
    value: Value,
    source: DiagnosticLocation,
    locations: BTreeMap<String, DiagnosticLocation>,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ValueKind {
    Nodes,
    RelationEndpoints,
    Literals(Vec<&'static str>),
}
pub(crate) struct Rules {
    shapes: BTreeMap<String, Shape>,
    roots: Vec<String>,
    pub files: Vec<PathBuf>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    ir: Option<IRSchema>,
    source_fingerprints: BTreeMap<PathBuf, String>,
}
impl Rules {
    pub fn load(project: &Project, schema: &Schema) -> Self {
        let mut rules = Self {
            shapes: BTreeMap::new(),
            roots: vec![],
            files: vec![],
            diagnostics: vec![],
            ir: None,
            source_fingerprints: BTreeMap::new(),
        };
        let schema_value = serde_json::to_value(schema).expect("schema serializes");
        let mut seen = BTreeSet::new();
        let mut files = project.rule_files.clone();
        files.sort();
        for path in files {
            rules.files.push(project.root().join(&path));
            let location = DiagnosticLocation {
                path: Some(path.clone()),
                ..Default::default()
            };
            if !crate::is_project_relative(&path)
                || !matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("yaml" | "yml")
                )
            {
                rules.invalid(
                    location,
                    "rule source must be a project-relative .yaml or .yml file",
                );
                continue;
            }
            let full = project.root().join(&path);
            let canonical = match fs::canonicalize(&full) {
                Ok(p) if p.starts_with(project.root()) && p.is_file() => p,
                _ => {
                    rules.invalid(
                        location,
                        "rule source must be an existing regular file inside the project",
                    );
                    continue;
                }
            };
            if !seen.insert(canonical) {
                rules.invalid(location, "duplicate rule source path");
                continue;
            }
            let source = match fs::read_to_string(&full) {
                Ok(s) => s,
                Err(_) => {
                    rules.invalid(location, "cannot read UTF-8 rule source");
                    continue;
                }
            };
            {
                use sha2::{Digest, Sha256};
                rules.source_fingerprints.insert(
                    path.clone(),
                    format!("{:x}", Sha256::digest(source.as_bytes())),
                );
            }
            let (value, spans) = match yaml::parse(&source, &path) {
                Ok(v) => v,
                Err(e) => {
                    rules.invalid(location, e);
                    continue;
                }
            };
            match value {
                Value::Array(values) => {
                    for (i, v) in values.into_iter().enumerate() {
                        rules.add(v, &format!("/{i}"), &spans, &schema_value, true);
                    }
                }
                Value::Object(_) => {
                    rules.add(value, "", &spans, &schema_value, true);
                }
                _ => rules.invalid(location, "expected a shape mapping or sequence of shapes"),
            }
        }
        for shape in rules.shapes.values_mut() {
            let property = shape.value.get("path").is_some();
            shape
                .value
                .as_object_mut()
                .unwrap()
                .entry("type")
                .or_insert_with(|| {
                    json!(if property {
                        "PropertyShape"
                    } else {
                        "NodeShape"
                    })
                });
        }
        rules.roots = rules
            .shapes
            .iter()
            .filter(|(_, s)| s.value.get("targetClass").is_some())
            .map(|(id, _)| id.clone())
            .collect();
        {
            for (id, shape) in rules.shapes.clone() {
                if let Err((key, message)) = rules.check(&id, &shape, &schema_value) {
                    rules.invalid(shape.location(&key), message);
                }
                if let Some(pattern) = shape.value["pattern"].as_str()
                    && shacl::ir::components::Pattern::new(pattern.into(), None).is_err()
                {
                    rules.invalid(
                        shape.location("pattern"),
                        "invalid or oversized regular expression",
                    );
                }
            }
            // Validate every reusable definition too, including unused cycles.
            for id in rules.shapes.keys().cloned().collect::<Vec<_>>() {
                if let Err(message) = rules.depth(&id, &mut Vec::new(), 0, &schema_value) {
                    rules.invalid(rules.shapes[&id].source.clone(), message);
                }
            }
            for root in rules.roots.clone() {
                let flavours = strings(&rules.shapes[&root].value["targetClass"]);
                if let Err((loc, message)) = rules.compatible(
                    &root,
                    &flavours,
                    &ValueKind::Nodes,
                    schema,
                    &mut BTreeSet::new(),
                ) {
                    rules.invalid(loc, message);
                }
            }
        }
        if rules.diagnostics.is_empty() && !rules.shapes.is_empty() {
            let mut graph = Vec::new();
            for shape in rules.shapes.values() {
                let mut v = shape.value.clone();
                for key in ["targetClass", "whenShape", "paths", "severity"] {
                    v.as_object_mut().unwrap().remove(key);
                }
                if let Some(path) = v.get_mut("path") {
                    if let Some(name) = path.as_str() {
                        *path = json!(
                            bindings::resolve_path(name, &schema_value).expect("validated path")
                        );
                    } else if let Some(name) = path["inversePath"].as_str() {
                        path["inversePath"] = json!(
                            bindings::resolve_path(name, &schema_value).expect("validated path")
                        );
                    }
                }
                graph.push(v);
            }
            let input = json!({"@context":bindings::context(&schema_value), "@graph":graph});
            match IRSchema::from_str(
                &input.to_string(),
                &RDFFormat::JsonLd,
                None,
                &ReaderMode::Strict,
            ) {
                Ok(ir) => rules.ir = Some(ir),
                Err(_) => rules.invalid(
                    rules.shapes.values().next().unwrap().source.clone(),
                    "SHACL shape compilation failed; check parameter combinations and patterns",
                ),
            }
        }
        rules
    }
    fn invalid(&mut self, location: DiagnosticLocation, message: impl Into<String>) {
        let rule = self
            .roots
            .iter()
            .find(|id| {
                let source = &self.shapes[*id].source;
                source.path == location.path
                    && source
                        .pointer
                        .as_ref()
                        .zip(location.pointer.as_ref())
                        .is_some_and(|(parent, child)| {
                            parent == child || child.starts_with(&format!("{parent}/"))
                        })
            })
            .cloned();
        let mut diagnostic = ValidationDiagnostic::new(
            DiagnosticCode::RuleInvalid,
            Severity::Error,
            ValidationScope::Schema,
            location,
            message,
        );
        diagnostic.rule = rule;
        self.diagnostics.push(diagnostic);
    }
    fn add(
        &mut self,
        mut v: Value,
        pointer: &str,
        spans: &BTreeMap<String, DiagnosticLocation>,
        schema: &Value,
        top: bool,
    ) -> Option<String> {
        let loc = spans.get(pointer).cloned().unwrap_or_default();
        if let Some(s) = v.as_str() {
            if top {
                self.invalid(loc, "top-level shapes must be mappings with an id");
                return None;
            }
            return expand_id(s).ok();
        }
        let Some(map) = v.as_object_mut() else {
            self.invalid(loc, "expected shape mapping or named reference");
            return None;
        };
        let id = match map.get("id") {
            Some(Value::String(s)) => match expand_id(s) {
                Ok(s) => s,
                Err(e) => {
                    self.invalid(loc, e);
                    return None;
                }
            },
            Some(_) => {
                self.invalid(loc, "shape id must be a string");
                return None;
            }
            None if top => {
                self.invalid(loc, "top-level shapes require an id");
                return None;
            }
            None => {
                use sha2::{Digest, Sha256};
                format!(
                    "urn:mara:anonymous:{:x}",
                    Sha256::digest(
                        format!(
                            "{}#{pointer}#{}",
                            loc.path.as_ref().unwrap().display(),
                            self.source_fingerprints[loc.path.as_ref().unwrap()]
                        )
                        .as_bytes()
                    )
                )
            }
        };
        let mut locations = BTreeMap::new();
        for key in map.keys() {
            let p = format!("{pointer}/{}", yaml::escape(key));
            locations.insert(
                key.clone(),
                spans.get(&p).cloned().unwrap_or_else(|| loc.clone()),
            );
        }
        for key in ["property", "and", "or"] {
            if let Some(Value::Array(values)) = map.get_mut(key) {
                for (i, value) in values.iter_mut().enumerate() {
                    let p = format!("{pointer}/{key}/{i}");
                    if let Some(id) = self.add(value.clone(), &p, spans, schema, false) {
                        *value = json!(id);
                    }
                }
            }
        }
        for key in ["node", "not", "qualifiedValueShape"] {
            if let Some(value) = map.get_mut(key)
                && let Some(id) = self.add(
                    value.clone(),
                    &format!("{pointer}/{key}"),
                    spans,
                    schema,
                    false,
                )
            {
                *value = json!(id);
            }
        }
        if let Some(Value::String(s)) = map.get_mut("whenShape")
            && let Ok(expanded) = expand_id(s)
        {
            *s = expanded;
        }
        map.insert("id".into(), json!(id));
        let _ = schema;
        if let Some(previous) = self.shapes.get_mut(&id) {
            for (key, value) in map.iter() {
                match previous.value.get(key) {
                    Some(old) if old == value => (),
                    Some(old) if ["property", "class", "targetClass"].contains(&key.as_str()) => {
                        let mut values =
                            old.as_array().cloned().unwrap_or_else(|| vec![old.clone()]);
                        for v in value
                            .as_array()
                            .cloned()
                            .unwrap_or_else(|| vec![value.clone()])
                        {
                            if !values.contains(&v) {
                                values.push(v);
                            }
                        }
                        previous.value[key] = json!(values);
                    }
                    Some(_) => {
                        let at = locations.get(key).cloned().unwrap_or_else(|| loc.clone());
                        self.invalid(
                            at,
                            format!("conflicting single-valued parameter {key} on {id}"),
                        );
                        return None;
                    }
                    None => {
                        previous.value[key] = value.clone();
                        previous.locations.insert(
                            key.clone(),
                            locations.get(key).cloned().unwrap_or_else(|| loc.clone()),
                        );
                    }
                }
            }
        } else {
            self.shapes.insert(
                id.clone(),
                Shape {
                    value: v,
                    source: loc,
                    locations,
                },
            );
        }
        Some(id)
    }
    fn check(&self, _id: &str, shape: &Shape, schema: &Value) -> Result<(), (String, String)> {
        let m = shape.value.as_object().unwrap();
        let root = m.contains_key("targetClass");
        let property = m.contains_key("path");
        for (key, v) in m {
            let error = |s: &str| (key.clone(), s.to_owned());
            match key.as_str() {
                "id" => (),
                "type"
                    if v.as_str()
                        == Some(if property {
                            "PropertyShape"
                        } else {
                            "NodeShape"
                        }) => {}
                "type" => return Err(error("shape type and path disagree")),
                "path" => {
                    let (p, inverse) = if let Some(p) = v.as_str() {
                        (p, false)
                    } else if v.as_object().is_some_and(|o| o.len() == 1) {
                        (
                            v["inversePath"]
                                .as_str()
                                .ok_or_else(|| error("expected inversePath relation"))?,
                            true,
                        )
                    } else {
                        return Err(error("unsupported property path"));
                    };
                    let resolved =
                        bindings::resolve_path(p, schema).map_err(|e| (key.clone(), e))?;
                    if inverse && !resolved.starts_with(REL) {
                        return Err(error("inversePath requires a canonical relation"));
                    }
                }
                "targetClass" | "class" => {
                    if key == "targetClass" && property {
                        return Err(error("enabled roots must be NodeShape"));
                    }
                    let values = strings(v);
                    if values.len() != v.as_array().map_or(1, Vec::len)
                        || values.is_empty()
                        || values.iter().any(|s| schema["flavours"].get(s).is_none())
                    {
                        return Err(error("expected declared flavour name(s)"));
                    }
                }
                "whenShape" | "node" | "not" | "qualifiedValueShape" => {
                    let id = v
                        .as_str()
                        .ok_or_else(|| error("expected a shape reference"))?;
                    let referenced = self
                        .shapes
                        .get(id)
                        .ok_or_else(|| error("unknown named shape"))?;
                    if key == "qualifiedValueShape" && !property {
                        return Err(error("qualifiedValueShape requires a property path"));
                    }
                    if referenced.value.get("targetClass").is_some()
                        || referenced.value.get("paths").is_some()
                        || referenced.value.get("whenShape").is_some()
                    {
                        return Err(error(
                            "referenced shapes must have no root selection metadata",
                        ));
                    }
                    if key == "whenShape"
                        && (!root
                            || referenced.value.get("targetClass").is_some()
                            || referenced.value.get("path").is_some())
                    {
                        return Err(error(
                            "whenShape requires a targetless NodeShape on an enabled root",
                        ));
                    }
                }
                "property" | "and" | "or" => {
                    let list = v
                        .as_array()
                        .ok_or_else(|| error("expected a shape sequence"))?;
                    for id in list {
                        let shape = id
                            .as_str()
                            .and_then(|id| self.shapes.get(id))
                            .ok_or_else(|| error("unknown shape reference"))?;
                        if shape.value.get("targetClass").is_some()
                            || shape.value.get("paths").is_some()
                            || shape.value.get("whenShape").is_some()
                        {
                            return Err(error(
                                "referenced shapes must have no root selection metadata",
                            ));
                        }
                        if key == "property" && shape.value.get("path").is_none() {
                            return Err(error("property requires PropertyShape references"));
                        }
                    }
                }
                "paths" if root => {
                    if strings(v).len() != v.as_array().map_or(1, Vec::len)
                        || crate::query::normalized_paths(
                            &strings(v).iter().map(PathBuf::from).collect::<Vec<_>>(),
                        )
                        .is_err()
                    {
                        return Err(error("invalid root paths"));
                    }
                }
                "severity" if root && matches!(v.as_str(), Some("Warning" | "Violation")) => (),
                "minCount" | "maxCount" | "qualifiedMinCount" | "qualifiedMaxCount"
                    if property && v.as_u64().is_some_and(|n| n <= isize::MAX as u64) => {}
                "datatype"
                    if matches!(
                        v.as_str(),
                        Some("string" | "integer" | "double" | "boolean")
                    ) => {}
                "name" | "description" | "message" | "pattern" if v.is_string() => (),
                "hasValue" => check_literal(v).map_err(|e| (key.clone(), e))?,
                "in" => {
                    for literal in v
                        .as_array()
                        .ok_or_else(|| error("expected literal sequence"))?
                    {
                        check_literal(literal).map_err(|e| (key.clone(), e))?;
                    }
                }
                _ => return Err(error("unsupported rule key, location or parameter type")),
            }
        }
        if (m.contains_key("qualifiedMinCount") || m.contains_key("qualifiedMaxCount"))
            && !m.contains_key("qualifiedValueShape")
        {
            return Err((
                "qualifiedValueShape".into(),
                "qualified counts require a value shape".into(),
            ));
        }
        Ok(())
    }
    fn depth(
        &self,
        id: &str,
        stack: &mut Vec<String>,
        hops: usize,
        schema: &Value,
    ) -> Result<(), String> {
        if stack.len() >= 32 || stack.iter().any(|s| s == id) {
            return Err("recursive shape reference or depth above 32".into());
        }
        let Some(shape) = self.shapes.get(id) else {
            return Ok(());
        };
        let relation = shape.value.get("path").is_some_and(|v| {
            v.is_object()
                || v.as_str().is_some_and(|s| {
                    bindings::resolve_path(s, schema).is_ok_and(|p| p.starts_with(REL))
                })
        });
        let hops = hops + usize::from(relation);
        if hops > 8 {
            return Err("relationship depth exceeds eight".into());
        }
        stack.push(id.into());
        for child in references(&shape.value) {
            self.depth(child, stack, hops, schema)?;
        }
        stack.pop();
        Ok(())
    }
    fn compatible(
        &self,
        id: &str,
        flavours: &[String],
        value_kind: &ValueKind,
        schema: &Schema,
        visited: &mut BTreeSet<(String, Vec<String>, ValueKind)>,
    ) -> Result<(), (DiagnosticLocation, String)> {
        if !visited.insert((id.into(), flavours.to_vec(), value_kind.clone())) {
            return Ok(());
        }
        let Some(s) = self.shapes.get(id) else {
            return Ok(());
        };
        let classes = strings(&s.value["class"]);
        // A property shape constrains the values selected by its path, not
        // the incoming focus nodes whose flavours determine path compatibility.
        let context = if classes.is_empty() || s.value.get("path").is_some() {
            flavours.to_vec()
        } else {
            flavours
                .iter()
                .filter(|f| classes.contains(f))
                .cloned()
                .collect()
        };
        let mut children = context.clone();
        let mut child_kind = value_kind.clone();
        if let Some(path) = s.value.get("path") {
            if matches!(value_kind, ValueKind::Literals(_)) {
                return Err((
                    s.location("path"),
                    "field and relation paths require item nodes, not literal field values".into(),
                ));
            }
            let name = path
                .as_str()
                .or_else(|| path["inversePath"].as_str())
                .unwrap_or("");
            let sv = serde_json::to_value(schema).unwrap();
            if let Ok(resolved) = bindings::resolve_path(name, &sv) {
                if let Some(field) = resolved.strip_prefix(FIELD) {
                    let defs = context
                        .iter()
                        .filter_map(|f| schema.flavours.get(f).and_then(|f| f.fields.get(field)))
                        .collect::<Vec<_>>();
                    if !context.is_empty() && defs.len() != context.len() {
                        return Err((
                            s.location("path"),
                            format!("field {field} is not declared for every selected flavour"),
                        ));
                    }
                    let mut datatypes: Vec<_> =
                        defs.iter().map(|d| datatype_name(d.field_type)).collect();
                    datatypes.sort_unstable();
                    datatypes.dedup();
                    child_kind = ValueKind::Literals(datatypes);
                    children.clear();
                } else if let Some(relation) = resolved.strip_prefix(REL) {
                    child_kind = ValueKind::RelationEndpoints;
                    let r = &schema.relations[relation];
                    let (from, to) = if path.is_object() {
                        (&r.target, &r.source)
                    } else {
                        (&r.source, &r.target)
                    };
                    if context.iter().any(|f| !from.contains(f)) {
                        return Err((
                            s.location("path"),
                            "relation path is incompatible with the selected flavour".into(),
                        ));
                    }
                    children = to
                        .iter()
                        .filter(|f| !r.same_flavour || context.contains(f))
                        .cloned()
                        .collect();
                }
            }
            if !classes.is_empty() {
                children.retain(|f| classes.contains(f));
            }
        }
        if !classes.is_empty() && matches!(child_kind, ValueKind::Literals(_)) {
            return Err((
                s.location("class"),
                "flavour class constraints require item nodes, not literal field values".into(),
            ));
        }
        if !classes.is_empty()
            && matches!(child_kind, ValueKind::RelationEndpoints)
            && children.is_empty()
        {
            return Err((
                s.location("class"),
                "flavour class is incompatible with the relation endpoint flavours".into(),
            ));
        }
        if matches!(child_kind, ValueKind::Nodes | ValueKind::RelationEndpoints) {
            for key in ["hasValue", "in"] {
                if s.value.get(key).is_some() {
                    return Err((
                        s.location(key),
                        format!("{key} constraints require literal field values"),
                    ));
                }
            }
        }
        if let Some(datatype) = s.value["datatype"].as_str() {
            let error = match &child_kind {
                ValueKind::Nodes | ValueKind::RelationEndpoints => {
                    Some("datatype constraints require literal field values")
                }
                ValueKind::Literals(types) if types.iter().any(|t| datatype != *t) => {
                    Some("field datatype is incompatible with its schema declaration")
                }
                ValueKind::Literals(_) => None,
            };
            if let Some(message) = error {
                return Err((s.location("datatype"), message.into()));
            }
        }
        for key in [
            "whenShape",
            "node",
            "not",
            "qualifiedValueShape",
            "and",
            "or",
            "property",
        ] {
            for child in strings(&s.value[key]) {
                self.compatible(
                    &child,
                    if key == "whenShape" {
                        &context
                    } else {
                        &children
                    },
                    if key == "whenShape" {
                        value_kind
                    } else {
                        &child_kind
                    },
                    schema,
                    visited,
                )?;
            }
        }
        Ok(())
    }
}
impl Shape {
    fn location(&self, key: &str) -> DiagnosticLocation {
        self.locations
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.source.clone())
    }
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| v.as_str().map(|s| vec![s.into()]).unwrap_or_default())
}
fn references(v: &Value) -> Vec<&str> {
    [
        "whenShape",
        "node",
        "not",
        "qualifiedValueShape",
        "property",
        "and",
        "or",
    ]
    .into_iter()
    .flat_map(|k| {
        v[k].as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_else(|| v[k].as_str().into_iter().collect::<Vec<_>>())
    })
    .collect()
}
fn expand_id(id: &str) -> Result<String, String> {
    let expanded = if let Some(name) = id.strip_prefix("rule:") {
        if name.is_empty() {
            return Err("rule identity needs a name".into());
        }
        format!("{RULE}{name}")
    } else {
        id.to_owned()
    };
    rudof_iri::IriS::new(&expanded)
        .map(|_| expanded)
        .map_err(|_| "shape identity must be rule:name or an absolute IRI".into())
}

fn check_literal(v: &Value) -> Result<(), String> {
    if let Some(m) = v.as_object() {
        if m.len() != 2 || !m.contains_key("value") || !m.contains_key("datatype") {
            return Err("typed literal requires exactly value and datatype".into());
        }
        let valid = match m["datatype"].as_str() {
            Some("string") => m["value"].is_string(),
            Some("integer") => m["value"].is_i64() || m["value"].is_u64(),
            Some("double") => m["value"].as_f64().is_some_and(f64::is_finite),
            Some("boolean") => m["value"].is_boolean(),
            _ => false,
        };
        if !valid {
            return Err("invalid typed literal".into());
        }
    } else if v.is_null() || v.is_array() {
        return Err("expected a scalar literal".into());
    }
    Ok(())
}
fn datatype_name(t: crate::FieldType) -> &'static str {
    match t {
        crate::FieldType::String | crate::FieldType::Enum => "string",
        crate::FieldType::Integer => "integer",
        crate::FieldType::Number => "double",
        crate::FieldType::Boolean => "boolean",
    }
}
