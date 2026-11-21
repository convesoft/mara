//! `schema list` and `schema get` (REQ-SCHEMA-INSPECTION and DES-SCHEMA-MODEL in
//! `docs/project.mara.md`).

use crate::diagnostic::Diagnostic;
use crate::document::label_name;
use crate::hash::{Hash, KeyHasher};
use crate::json::Object;
use crate::paging::PAGE_BYTES;
use crate::schema::{FieldKind, RelationDecl, Schema, TypeDecl};

use super::page::{self, Envelope, strings};
use super::{Page, Paging, READ_VERSION, ReadError, TOO_LARGE, result_key};

/// The schema a read runs on, with the key of its layer ([`schema_layer`]) and its path.
pub struct SchemaLayer<'s> {
    pub schema: &'s Schema,
    pub key: Hash,
    /// Relative to the project root.
    pub path: &'s str,
}

/// The key of the schema layer: the configuration and schema files as read, `None` for one
/// that could not be read. A schema read's cursor goes stale only when one of them changes.
pub fn schema_layer(config_hash: Option<Hash>, schema_hash: Option<Hash>) -> Hash {
    let mut key = KeyHasher::new("mara schema layer", READ_VERSION);
    key.optional(config_hash.as_ref());
    key.optional(schema_hash.as_ref());
    key.finish()
}

/// A declared type or relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Declaration<'s> {
    Type {
        name: &'s str,
        declaration: &'s TypeDecl,
        /// The `Status` values, one list for every type.
        statuses: &'s [String],
    },
    Relation {
        name: &'s str,
        declaration: &'s RelationDecl,
    },
}

impl Declaration<'_> {
    pub fn name(&self) -> &str {
        match self {
            Declaration::Type { name, .. } | Declaration::Relation { name, .. } => name,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Declaration::Type { .. } => "type",
            Declaration::Relation { .. } => "relation",
        }
    }

    /// What `schema list` says: kind, name, ID prefix or inverse name, and description.
    fn summary_json(&self) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.string("kind", self.kind());
        object.string("name", self.name());
        match self {
            Declaration::Type { declaration, .. } => {
                object.optional_string("id_prefix", declaration.id_prefix.as_deref());
                object.string("description", &declaration.description);
            }
            Declaration::Relation { declaration, .. } => {
                object.optional_string("inverse", declaration.inverse.as_deref());
                object.string("description", &declaration.description);
            }
        }
        object.end();
        out
    }

    /// The whole declaration, as loaded.
    fn json(&self) -> String {
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.string("kind", self.kind());
        object.string("name", self.name());
        match self {
            Declaration::Type {
                declaration,
                statuses,
                ..
            } => {
                let fields: Vec<String> = declaration
                    .fields
                    .iter()
                    .map(|(name, field)| {
                        let mut out = String::new();
                        let mut object = Object::new(&mut out);
                        object.string("name", name);
                        match &field.kind {
                            FieldKind::Text => {
                                object.string("kind", "text");
                                object.key("values").push_str("null");
                            }
                            FieldKind::Enum(values) => {
                                object.string("kind", "enum");
                                strings(&mut object, "values", values.iter().map(String::as_str));
                            }
                        }
                        object.bool("required", field.required);
                        object.bool("repeatable", field.repeatable);
                        object.end();
                        out
                    })
                    .collect();
                object.optional_string("id_prefix", declaration.id_prefix.as_deref());
                object.string("description", &declaration.description);
                strings(&mut object, "statuses", statuses.iter().map(String::as_str));
                object.array("fields", fields.iter().map(String::as_str));
            }
            Declaration::Relation { declaration, .. } => {
                let mut targets = String::new();
                let mut by_source = Object::new(&mut targets);
                for (source, types) in &declaration.targets {
                    strings(&mut by_source, source, types.iter().map(String::as_str));
                }
                by_source.end();
                object.optional_string("inverse", declaration.inverse.as_deref());
                object.string("description", &declaration.description);
                object.key("targets").push_str(&targets);
                object.bool("same_type", declaration.same_type);
                object.bool("acyclic", declaration.acyclic);
            }
        }
        object.end();
        out
    }
}

/// Every declared type, then every declared relation, each by name.
fn declarations(schema: &Schema) -> impl Iterator<Item = Declaration<'_>> {
    let types = schema
        .types
        .iter()
        .map(|(name, declaration)| Declaration::Type {
            name,
            declaration,
            statuses: &schema.statuses,
        });
    let relations = schema
        .relations
        .iter()
        .map(|(name, declaration)| Declaration::Relation { name, declaration });
    types.chain(relations)
}

// @mara implements REQ-SCHEMA-INSPECTION
/// `schema list`: every declared type and relation, types first, each by name.
pub fn schema_list<'s>(
    layer: &SchemaLayer<'s>,
    paging: &Paging<'_>,
) -> Result<Page<Declaration<'s>>, ReadError> {
    let all: Vec<Declaration<'s>> = declarations(layer.schema).collect();
    let key = result_key(layer.key, "schema list", [], &[]);
    pages(layer, key, paging, all, Declaration::summary_json)
}

// @mara implements REQ-SCHEMA-INSPECTION
/// `schema get`: the type and the relation that `name` names, as loaded. A relation is named by
/// its forward or inverse name; names match as labels do (DES-METADATA-TABLE).
pub fn schema_get<'s>(
    layer: &SchemaLayer<'s>,
    name: &str,
    paging: &Paging<'_>,
) -> Result<Page<Declaration<'s>>, ReadError> {
    let wanted = label_name(name);
    let found: Vec<Declaration<'s>> = declarations(layer.schema)
        .filter(|declaration| match declaration {
            Declaration::Type { name, .. } => *name == wanted,
            Declaration::Relation { name, declaration } => {
                *name == wanted || declaration.inverse.as_deref() == Some(wanted.as_str())
            }
        })
        .collect();
    if found.is_empty() {
        return Err(ReadError::UnknownDeclaration(format!(
            "the schema declares no type or relation named `{name}`"
        )));
    }
    let key = result_key(layer.key, "schema get", [wanted.as_str()], &[]);
    pages(layer, key, paging, found, Declaration::json)
}

fn pages<'s>(
    layer: &SchemaLayer<'s>,
    key: Hash,
    paging: &Paging<'_>,
    all: Vec<Declaration<'s>>,
    json: fn(&Declaration<'s>) -> String,
) -> Result<Page<Declaration<'s>>, ReadError> {
    let envelope = Envelope {
        head: None,
        records: "declarations",
        diagnostics: &[],
        kept: 0,
    };
    page::list(
        key,
        paging,
        all.len(),
        &envelope,
        |index| (all[index], json(&all[index])),
        |declaration| {
            let message = format!(
                "the {} `{}` cannot fit a page of {PAGE_BYTES} bytes",
                declaration.kind(),
                declaration.name()
            );
            ReadError::TooLarge(Box::new(Diagnostic::error(
                TOO_LARGE, layer.path, 1, message,
            )))
        },
    )
}
