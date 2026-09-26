//! JSON-compatible YAML with authored locations retained before JSON-LD conversion.
use crate::DiagnosticLocation;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use serde_saphyr::Spanned;
use std::{collections::BTreeMap, fmt, path::Path};

struct Node {
    value: Value,
    locations: BTreeMap<String, serde_saphyr::Location>,
}
impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Node;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON-compatible YAML with string keys")
            }
            fn visit_bool<E: de::Error>(self, x: bool) -> Result<Node, E> {
                Ok(node(Value::Bool(x)))
            }
            fn visit_i64<E: de::Error>(self, x: i64) -> Result<Node, E> {
                Ok(node(x.into()))
            }
            fn visit_u64<E: de::Error>(self, x: u64) -> Result<Node, E> {
                Ok(node(x.into()))
            }
            fn visit_f64<E: de::Error>(self, x: f64) -> Result<Node, E> {
                serde_json::Number::from_f64(x)
                    .map(|n| node(n.into()))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: de::Error>(self, x: &str) -> Result<Node, E> {
                Ok(node(x.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
                Ok(node(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
                let mut out = node(Value::Array(vec![]));
                while let Some(v) = seq.next_element::<Spanned<Node>>()? {
                    let p = format!("/{}", out.value.as_array().unwrap().len());
                    out.append(&p, &v);
                    out.value.as_array_mut().unwrap().push(v.value.value);
                }
                Ok(out)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
                let mut out = node(Value::Object(Map::new()));
                while let Some(key) = map.next_key::<String>()? {
                    let v = map.next_value::<Spanned<Node>>()?;
                    if out.value.get(&key).is_some() {
                        return Err(de::Error::custom("duplicate mapping key"));
                    }
                    out.append(&format!("/{}", escape(&key)), &v);
                    out.value
                        .as_object_mut()
                        .unwrap()
                        .insert(key, v.value.value);
                }
                Ok(out)
            }
        }
        d.deserialize_any(V)
    }
}
fn node(value: Value) -> Node {
    Node {
        value,
        locations: BTreeMap::new(),
    }
}
impl Node {
    fn append(&mut self, p: &str, child: &Spanned<Node>) {
        self.locations.insert(p.into(), child.referenced);
        for (suffix, location) in &child.value.locations {
            self.locations.insert(format!("{p}{suffix}"), *location);
        }
    }
}
pub(super) fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
pub(super) fn parse(
    source: &str,
    path: &Path,
) -> Result<(Value, BTreeMap<String, DiagnosticLocation>), String> {
    let mut options = serde_saphyr::Options::default();
    options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
    options.reject_unsupported_tags = true;
    let root: Spanned<Node> =
        serde_saphyr::from_str_with_options(source, options).map_err(|e| e.to_string())?;
    let mut locations = root.value.locations;
    locations.insert(String::new(), root.referenced);
    Ok((
        root.value.value,
        locations
            .into_iter()
            .map(|(pointer, loc)| {
                let span = loc.span();
                let start_byte = span.byte_offset().map(|x| x as usize);
                let end_byte = start_byte.zip(span.byte_len()).map(|(s, n)| s + n as usize);
                let location = DiagnosticLocation {
                    path: Some(path.into()),
                    line: (loc.line() > 0).then_some(loc.line() as usize),
                    start_byte,
                    end_byte,
                    pointer: Some(pointer.clone()),
                };
                (pointer, location)
            })
            .collect(),
    ))
}
