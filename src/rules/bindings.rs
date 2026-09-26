use super::{FIELD, FLAVOUR, REL, RULE, SH, XSD};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub(super) fn field_names(s: &Value) -> BTreeSet<String> {
    s["flavours"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|f| {
            f["fields"]
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys().cloned())
        })
        .collect()
}

// Fixed SHACL vocabulary, schema-generated value contexts; no input context or I/O.
pub(super) fn context(s: &Value) -> Value {
    let mut paths = json!({"@vocab":null,"field":{"@id":FIELD,"@prefix":true},"schema":{"@id":REL,"@prefix":true},"inversePath":{"@id":format!("{SH}inversePath"),"@type":"@vocab"}});
    let fields = field_names(s);
    let relations = s["relations"].as_object().unwrap();
    for name in &fields {
        if !relations.contains_key(name)
            && !["inversePath", "field", "schema"].contains(&name.as_str())
        {
            paths[name] = json!(format!("{FIELD}{name}"));
        }
    }
    for name in relations.keys() {
        if !fields.contains(name) && !["inversePath", "field", "schema"].contains(&name.as_str()) {
            paths[name] = json!(format!("{REL}{name}"));
        }
    }
    let mut flavours = json!({"@vocab":null});
    for name in s["flavours"].as_object().unwrap().keys() {
        flavours[name] = json!(format!("{FLAVOUR}{name}"));
    }
    let types = json!({"@vocab":null,"string":format!("{XSD}string"),"integer":format!("{XSD}integer"),"double":format!("{XSD}double"),"boolean":format!("{XSD}boolean")});
    let mut literals = types.clone();
    literals["value"] = json!("@value");
    literals["datatype"] = json!("@type");
    let mut c = json!({"@version":1.1,"@vocab":SH,"id":"@id","type":"@type","rule":{"@id":RULE,"@prefix":true},
        "targetClass":{"@type":"@vocab","@context":flavours},"class":{"@type":"@vocab","@context":flavours},
        "path":{"@type":"@vocab","@context":paths},"datatype":{"@type":"@vocab","@context":types},
        "severity":{"@type":"@vocab"},"whenShape":{"@id":"urn:mara:rules:1:whenShape","@type":"@id"},
        "paths":{"@id":"urn:mara:rules:1:paths"},
        "hasValue":{"@context":literals},"in":{"@container":"@list","@context":literals}});
    for k in ["property", "node", "qualifiedValueShape", "not"] {
        c[k] = json!({"@type":"@id"});
    }
    for k in ["and", "or"] {
        c[k] = json!({"@type":"@id","@container":"@list"});
    }
    c
}

pub(super) fn resolve_path(v: &str, s: &Value) -> Result<String, String> {
    let fields = field_names(s);
    let relations = s["relations"].as_object().unwrap();
    if let Some(n) = v.strip_prefix("field:") {
        if fields.contains(n) {
            return Ok(format!("{FIELD}{n}"));
        }
    } else if let Some(n) = v.strip_prefix("schema:") {
        if relations.contains_key(n) {
            return Ok(format!("{REL}{n}"));
        }
    } else {
        match (fields.contains(v), relations.contains_key(v)) {
            (true, false) if v != "inversePath" => return Ok(format!("{FIELD}{v}")),
            (false, true) if v != "inversePath" => return Ok(format!("{REL}{v}")),
            (true, true) => {
                return Err(format!("ambiguous path {v}: qualify field: or schema:"));
            }
            _ => {}
        }
    }
    Err(format!("unknown or unsupported path {v}"))
}
