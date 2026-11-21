//! The project's schema: types, fields, statuses and relations (DES-SCHEMA-MODEL in
//! `docs/project.mara.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::diagnostic::Diagnostic;
use crate::toml_file::{Key, Table, TomlFile, Value};

/// The relation source that stands for code markers. No type has this name.
pub const CODE_SOURCE: &str = "code";

/// Names that Mara builds in as the `Type`, `ID` and `Status` keys.
const BUILT_IN_KEYS: [&str; 3] = ["type", "id", "status"];

/// The declarations of a schema. An invalid declaration is left out, and so is an invalid
/// part of one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schema {
    /// The `Status` values of every type, in declared order.
    pub statuses: Vec<String>,
    pub types: BTreeMap<String, TypeDecl>,
    /// By forward name.
    pub relations: BTreeMap<String, RelationDecl>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeDecl {
    pub id_prefix: Option<String>,
    pub description: String,
    pub fields: BTreeMap<String, FieldDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDecl {
    pub kind: FieldKind,
    pub required: bool,
    pub repeatable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    /// The allowed values, in declared order.
    Enum(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelationDecl {
    pub inverse: Option<String>,
    pub description: String,
    /// The allowed target types of each source type. The source `CODE_SOURCE` stands for code
    /// markers.
    pub targets: BTreeMap<String, Vec<String>>,
    /// The target has the type of the source.
    pub same_type: bool,
    /// No chain of the relation leads back to its start.
    pub acyclic: bool,
}

impl Schema {
    /// The relation `name` when an item of type `source` may have it: the type is declared and
    /// is a source of the relation (DES-SCHEMA-MODEL). A row of any other item that is keyed by
    /// `name` is no relation of that item (DES-SNAPSHOT).
    pub fn relation_from(&self, source: &str, name: &str) -> Option<&RelationDecl> {
        self.relations.get(name).filter(|relation| {
            self.types.contains_key(source) && relation.targets.contains_key(source)
        })
    }
}

// @mara implements REQ-SCHEMA-DEFINES-VOCABULARY
/// Reads the contents of the schema file at `path`, relative to the project root. Every
/// problem becomes a diagnostic. There is no schema when the file is not TOML or has another
/// format version.
pub fn parse_schema(path: &str, bytes: &[u8]) -> (Option<Schema>, Vec<Diagnostic>) {
    let (file, table) = TomlFile::parse(path, bytes);
    let mut reader = Reader { file };
    let schema = table.and_then(|table| reader.schema(&table));
    (schema, reader.file.finish())
}

struct Reader<'p> {
    file: TomlFile<'p>,
}

/// What name collisions leave out of the declarations that are otherwise valid.
#[derive(Default)]
struct Collisions {
    /// Relations whose inverse name is left out.
    inverses: BTreeSet<String>,
    /// Fields left out, by type and field name.
    fields: BTreeSet<(String, String)>,
}

/// A relation's forward or inverse name, where it is declared.
struct RelationName<'t> {
    name: &'t str,
    span: Range<usize>,
    relation: &'t str,
    inverse: bool,
}

impl RelationName<'_> {
    fn describe(&self) -> String {
        if self.inverse {
            format!(
                "inverse name `{}` of relation `{}`",
                self.name, self.relation
            )
        } else {
            format!("relation name `{}`", self.name)
        }
    }
}

impl Reader<'_> {
    fn schema(&mut self, root: &Table) -> Option<Schema> {
        const PLACE: &str = "the schema";
        if !self.file.format_version(root, 0..0, PLACE) {
            return None;
        }
        self.file.unknown_keys(
            root,
            &["format_version", "statuses", "types", "relations"],
            PLACE,
        );
        let collisions = self.collisions(root);
        let mut schema = Schema::default();
        if let Some(value) = self.file.required(root, "statuses", 0..0, PLACE) {
            schema.statuses = self.names("statuses", value);
        }
        if let Some(value) = root.get("types")
            && let Some(types) = self.file.table("types", value)
        {
            for (name, value) in types {
                let valid = self.declared_name(name, "type", &[CODE_SOURCE]);
                let name = name.get_ref().as_ref();
                if let Some(declaration) = self.type_decl(name, value)
                    && valid
                {
                    schema.types.insert(name.to_owned(), declaration);
                }
            }
        }
        if let Some(value) = root.get("relations")
            && let Some(relations) = self.file.table("relations", value)
        {
            for (name, value) in relations {
                let valid = self.declared_name(name, "relation", &BUILT_IN_KEYS);
                let name = name.get_ref().as_ref();
                if let Some(declaration) = self.relation_decl(name, value, &schema.types)
                    && valid
                {
                    schema.relations.insert(name.to_owned(), declaration);
                }
            }
        }
        for relation in &collisions.inverses {
            if let Some(declaration) = schema.relations.get_mut(relation) {
                declaration.inverse = None;
            }
        }
        for (type_name, field) in &collisions.fields {
            if let Some(declaration) = schema.types.get_mut(type_name) {
                declaration.fields.remove(field);
            }
        }
        Some(schema)
    }

    fn type_decl(&mut self, name: &str, value: &Value) -> Option<TypeDecl> {
        let table = self.file.table(name, value)?;
        let place = format!("type `{name}`");
        self.file
            .unknown_keys(table, &["id_prefix", "description", "fields"], &place);
        let mut declaration = TypeDecl::default();
        if let Some(value) = self.file.required(table, "id_prefix", value.span(), &place)
            && let Some(prefix) = self.file.string("id_prefix", value)
        {
            if is_id_prefix(prefix) {
                declaration.id_prefix = Some(prefix.to_owned());
            } else {
                self.file.error(
                    "invalid-value",
                    value.span(),
                    format!(
                        "`id_prefix` of {place} must be uppercase letters and digits in parts that each end with `-`, like `REQ-`: `{prefix}`"
                    ),
                );
            }
        }
        declaration.description = self.description(table, value.span(), &place);
        if let Some(value) = table.get("fields")
            && let Some(fields) = self.file.table("fields", value)
        {
            for (field, value) in fields {
                let valid = self.declared_name(field, "field", &BUILT_IN_KEYS);
                let field_name = field.get_ref().as_ref();
                if let Some(field_decl) = self.field_decl(name, field_name, value)
                    && valid
                {
                    declaration.fields.insert(field_name.to_owned(), field_decl);
                }
            }
        }
        Some(declaration)
    }

    fn field_decl(&mut self, type_name: &str, name: &str, value: &Value) -> Option<FieldDecl> {
        let table = self.file.table(name, value)?;
        let place = format!("field `{name}` of type `{type_name}`");
        self.file
            .unknown_keys(table, &["kind", "values", "required", "repeatable"], &place);
        let required = self.flag(table, "required");
        let repeatable = self.flag(table, "repeatable");
        let kind_value = self.file.required(table, "kind", value.span(), &place);
        let kind_name = kind_value.and_then(|kind| self.file.string("kind", kind));
        let values = table.get("values");
        let kind = match (kind_name, values) {
            (Some("text"), None) => Some(FieldKind::Text),
            (Some("text"), Some(values)) => {
                self.file.error(
                    "invalid-value",
                    values.span(),
                    format!("{place} has kind `text`, which takes no `values`"),
                );
                Some(FieldKind::Text)
            }
            (Some("enum"), None) => {
                self.file.error(
                    "missing-key",
                    value.span(),
                    format!("{place} has kind `enum` and no `values`"),
                );
                None
            }
            (Some("enum"), Some(values)) => {
                let values = self.names("values", values);
                (!values.is_empty()).then_some(FieldKind::Enum(values))
            }
            (other, values) => {
                if let (Some(other), Some(kind_value)) = (other, kind_value) {
                    self.file.error(
                        "invalid-value",
                        kind_value.span(),
                        format!("`kind` of {place} must be `text` or `enum`: `{other}`"),
                    );
                }
                if let Some(values) = values {
                    self.names("values", values);
                }
                None
            }
        };
        Some(FieldDecl {
            kind: kind?,
            required,
            repeatable,
        })
    }

    fn relation_decl(
        &mut self,
        name: &str,
        value: &Value,
        types: &BTreeMap<String, TypeDecl>,
    ) -> Option<RelationDecl> {
        let table = self.file.table(name, value)?;
        let place = format!("relation `{name}`");
        self.file.unknown_keys(
            table,
            &["inverse", "description", "targets", "same_type", "acyclic"],
            &place,
        );
        let mut declaration = RelationDecl {
            same_type: self.flag(table, "same_type"),
            acyclic: self.flag(table, "acyclic"),
            ..RelationDecl::default()
        };
        if let Some(value) = self.file.required(table, "inverse", value.span(), &place)
            && let Some(inverse) = self.file.string("inverse", value)
            && self.check_name(inverse, "inverse name", value.span(), &BUILT_IN_KEYS)
        {
            declaration.inverse = Some(inverse.to_owned());
        }
        declaration.description = self.description(table, value.span(), &place);
        let targets_value = self.file.required(table, "targets", value.span(), &place)?;
        let targets = self.file.table("targets", targets_value)?;
        if targets.is_empty() {
            self.file.error(
                "invalid-value",
                targets_value.span(),
                format!("`targets` of {place} names no source type"),
            );
        }
        for (source, value) in targets {
            let source_name = source.get_ref().as_ref();
            let mut source_valid = true;
            if source_name != CODE_SOURCE && !types.contains_key(source_name) {
                self.undeclared(source.span(), source_name, &place);
                source_valid = false;
            } else if declaration.same_type && source_name == CODE_SOURCE {
                self.file.error(
                    "invalid-value",
                    source.span(),
                    format!("{place} requires the same type, which code does not have"),
                );
                source_valid = false;
            }
            let Some(list) = self.file.array(source_name, value) else {
                continue;
            };
            if list.is_empty() {
                self.file.error(
                    "invalid-value",
                    value.span(),
                    format!("source `{source_name}` of {place} lists no target type"),
                );
            }
            let mut written = BTreeSet::new();
            let mut kept: Vec<String> = Vec::new();
            for target in list {
                let Some(target_name) = self.file.string(source_name, target) else {
                    continue;
                };
                let repeated = !written.insert(target_name);
                if repeated {
                    self.file.error(
                        "duplicate-name",
                        target.span(),
                        format!(
                            "{place} lists target `{target_name}` of source `{source_name}` twice"
                        ),
                    );
                }
                if !types.contains_key(target_name) {
                    self.undeclared(target.span(), target_name, &place);
                } else if source_valid && declaration.same_type && target_name != source_name {
                    self.file.error(
                        "invalid-value",
                        target.span(),
                        format!("{place} requires the same type, but source `{source_name}` lists target `{target_name}`"),
                    );
                } else if !repeated {
                    kept.push(target_name.to_owned());
                }
            }
            if source_valid && !kept.is_empty() {
                declaration.targets.insert(source_name.to_owned(), kept);
            }
        }
        Some(declaration)
    }

    /// Reports every collision between declared names, read from the tables as written before
    /// any declaration is validated, so that each is reported whether or not its declarations
    /// are kept. A repetition is reported at its later occurrence in the file. An inverse name
    /// that repeats a relation name or an earlier inverse name is left out, and so is a field that
    /// has the name of a relation its type is a source of.
    fn collisions(&mut self, root: &Table) -> Collisions {
        let empty = Table::new();
        let table = |key: &str| {
            root.get(key)
                .and_then(|value| value.get_ref().as_table())
                .unwrap_or(&empty)
        };
        let (relations, types) = (table("relations"), table("types"));
        let mut collisions = Collisions::default();

        let mut names: Vec<RelationName> = Vec::new();
        for (key, value) in relations {
            let relation = key.get_ref().as_ref();
            names.push(RelationName {
                name: relation,
                span: key.span(),
                relation,
                inverse: false,
            });
            if let Some(inverse) = value.get_ref().as_table().and_then(|t| t.get("inverse"))
                && let Some(name) = inverse.get_ref().as_str()
            {
                names.push(RelationName {
                    name,
                    span: inverse.span(),
                    relation,
                    inverse: true,
                });
            }
        }
        names.sort_by_key(|name| name.span.start);
        let forward: BTreeSet<&str> = relations.keys().map(|key| key.get_ref().as_ref()).collect();
        let mut first: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, name) in names.iter().enumerate() {
            let earlier = *first.entry(name.name).or_insert(index);
            if earlier != index {
                self.file.error(
                    "duplicate-name",
                    name.span.clone(),
                    format!("{} repeats {}", name.describe(), names[earlier].describe()),
                );
            }
        }
        for (index, name) in names.iter().enumerate() {
            if name.inverse && (forward.contains(name.name) || first[name.name] != index) {
                collisions.inverses.insert(name.relation.to_owned());
            }
        }

        for (type_key, type_value) in types {
            let type_name = type_key.get_ref().as_ref();
            let Some(fields) = type_value
                .get_ref()
                .as_table()
                .and_then(|t| t.get("fields"))
                .and_then(|fields| fields.get_ref().as_table())
            else {
                continue;
            };
            for field_key in fields.keys() {
                let field = field_key.get_ref().as_ref();
                let Some((relation_key, relation)) = relations.get_key_value(field) else {
                    continue;
                };
                let is_source = relation
                    .get_ref()
                    .as_table()
                    .and_then(|t| t.get("targets"))
                    .and_then(|targets| targets.get_ref().as_table())
                    .is_some_and(|targets| targets.contains_key(type_name));
                if !is_source {
                    continue;
                }
                let (span, message) = if field_key.span().start > relation_key.span().start {
                    (
                        field_key.span(),
                        format!(
                            "field `{field}` of type `{type_name}` repeats relation name `{field}`, which the type is a source of"
                        ),
                    )
                } else {
                    (
                        relation_key.span(),
                        format!(
                            "relation name `{field}` repeats field `{field}` of type `{type_name}`, a source of the relation"
                        ),
                    )
                };
                self.file.error("duplicate-name", span, message);
                collisions
                    .fields
                    .insert((type_name.to_owned(), field.to_owned()));
            }
        }
        collisions
    }

    /// Whether the name of a declaration is valid and not in `reserved`.
    fn declared_name(&mut self, key: &Key, what: &str, reserved: &[&str]) -> bool {
        self.check_name(key.get_ref(), &format!("{what} name"), key.span(), reserved)
    }

    fn check_name(
        &mut self,
        name: &str,
        what: &str,
        span: Range<usize>,
        reserved: &[&str],
    ) -> bool {
        if !is_name(name) {
            self.file.error(
                "invalid-value",
                span,
                format!(
                    "{what} `{name}` must be lowercase letters and digits, in parts joined by single `_` or `-`"
                ),
            );
            return false;
        }
        if reserved.contains(&name) {
            self.file.error(
                "reserved-name",
                span,
                format!("{what} `{name}` is reserved"),
            );
            return false;
        }
        true
    }

    /// The valid, distinct names of an array, in order.
    fn names(&mut self, key: &str, value: &Value) -> Vec<String> {
        let Some(items) = self.file.array(key, value) else {
            return Vec::new();
        };
        if items.is_empty() {
            self.file.error(
                "invalid-value",
                value.span(),
                format!("`{key}` must list at least one name"),
            );
        }
        let mut written = BTreeSet::new();
        let mut names: Vec<String> = Vec::new();
        for item in items {
            let Some(name) = self.file.string(key, item) else {
                continue;
            };
            let repeated = !written.insert(name);
            if repeated {
                self.file.error(
                    "duplicate-name",
                    item.span(),
                    format!("`{key}` lists `{name}` twice"),
                );
            }
            if self.check_name(name, &format!("`{key}` value"), item.span(), &[]) && !repeated {
                names.push(name.to_owned());
            }
        }
        names
    }

    fn description(&mut self, table: &Table, at: Range<usize>, place: &str) -> String {
        self.file
            .required(table, "description", at, place)
            .and_then(|value| self.file.text("description", value))
            .unwrap_or_default()
            .to_owned()
    }

    fn flag(&mut self, table: &Table, key: &str) -> bool {
        table
            .get(key)
            .and_then(|value| self.file.boolean(key, value))
            .unwrap_or(false)
    }

    fn undeclared(&mut self, span: Range<usize>, name: &str, place: &str) {
        let message = if name == CODE_SOURCE {
            format!("{place} lists `code` as a target; code can only be a source")
        } else {
            format!("{place} names type `{name}`, which the schema does not declare")
        };
        self.file.error("undeclared-type", span, message);
    }
}

/// Lowercase ASCII letters and digits in parts joined by single `_` or `-`.
pub(crate) fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.split(['_', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// An uppercase letter, then uppercase letters and digits in parts that each end with `-`:
/// the start of an ID matching `[A-Z][A-Z0-9]*(-[A-Z0-9]+)+` (DES-ITEM-IDS).
fn is_id_prefix(prefix: &str) -> bool {
    let Some(body) = prefix.strip_suffix('-') else {
        return false;
    };
    prefix.starts_with(|c: char| c.is_ascii_uppercase())
        && body.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = ".mara/schema.toml";

    fn parse(text: &str) -> (Option<Schema>, Vec<(usize, &'static str)>) {
        let (schema, diagnostics) = parse_schema(PATH, text.as_bytes());
        assert!(diagnostics.iter().all(|d| d.path == PATH));
        (
            schema,
            diagnostics
                .iter()
                .map(|d| (d.line, d.built_in_code()))
                .collect(),
        )
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// Every kind of declaration, a required field included.
    const EXAMPLE: &str = r#"format_version = 1
statuses = ["draft", "accepted", "retired"]

[types.risk]
id_prefix = "RISK-"
description = "An uncertain cause with an adverse consequence."

[types.risk.fields.likelihood]
kind = "enum"
values = ["rare", "unlikely", "possible", "likely", "almost-certain"]

[types.need]
id_prefix = "NEED-"
description = "A stakeholder capability."

[types.requirement]
id_prefix = "REQ-"
description = "One verifiable obligation."

[types.requirement.fields.rationale]
kind = "text"
required = true

[types.design]
id_prefix = "DES-"
description = "A durable realization."

[types.verification]
id_prefix = "VER-"
description = "How conformity is established."

[types.verification.fields.methods]
kind = "enum"
values = ["test", "analysis"]
repeatable = true

[relations.satisfies]
inverse = "satisfied_by"
description = "The source is intended to fulfil the target."
targets = { requirement = ["need"], design = ["requirement"] }

[relations.implements]
inverse = "implemented_by"
description = "Code realizes the target."
targets = { code = ["requirement", "design", "verification"] }

[relations.decomposes]
inverse = "decomposed_by"
description = "Child to parent."
targets = { requirement = ["requirement"], design = ["design"] }
same_type = true
acyclic = true
"#;

    #[test]
    fn reads_every_declaration() {
        let (schema, diagnostics) = parse(EXAMPLE);
        assert_eq!(diagnostics, []);
        let schema = schema.unwrap();
        assert_eq!(schema.statuses, strings(&["draft", "accepted", "retired"]));
        assert_eq!(
            schema.types.keys().collect::<Vec<_>>(),
            ["design", "need", "requirement", "risk", "verification"]
        );
        let risk = &schema.types["risk"];
        assert_eq!(risk.id_prefix.as_deref(), Some("RISK-"));
        assert_eq!(
            risk.description,
            "An uncertain cause with an adverse consequence."
        );
        assert_eq!(
            risk.fields["likelihood"],
            FieldDecl {
                kind: FieldKind::Enum(strings(&[
                    "rare",
                    "unlikely",
                    "possible",
                    "likely",
                    "almost-certain"
                ])),
                required: false,
                repeatable: false,
            }
        );
        assert_eq!(
            schema.types["requirement"].fields["rationale"],
            FieldDecl {
                kind: FieldKind::Text,
                required: true,
                repeatable: false
            }
        );
        assert!(schema.types["verification"].fields["methods"].repeatable);
        assert!(schema.types["need"].fields.is_empty());

        let satisfies = &schema.relations["satisfies"];
        assert_eq!(satisfies.inverse.as_deref(), Some("satisfied_by"));
        assert_eq!(
            satisfies.targets,
            BTreeMap::from([
                ("design".to_owned(), strings(&["requirement"])),
                ("requirement".to_owned(), strings(&["need"])),
            ])
        );
        assert!(!satisfies.same_type && !satisfies.acyclic);
        assert_eq!(
            schema.relations["implements"].targets[CODE_SOURCE],
            strings(&["requirement", "design", "verification"])
        );
        let decomposes = &schema.relations["decomposes"];
        assert!(decomposes.same_type && decomposes.acyclic);
    }

    #[test]
    fn a_schema_may_declare_no_type_or_relation() {
        let (schema, diagnostics) = parse("format_version = 1\nstatuses = [\"open\"]\n");
        assert_eq!(diagnostics, []);
        let schema = schema.unwrap();
        assert!(schema.types.is_empty() && schema.relations.is_empty());
    }

    #[test]
    fn leaves_out_only_what_is_invalid() {
        let text = r#"format_version = 1
statuses = ["draft", "Draft", "draft", 4]
colour = "blue"

[types.code]
id_prefix = "CODE-"
description = "Not allowed."

[types.Risk]
id_prefix = "R-"
description = "Not a name."

[types.risk]
id_prefix = "risk-"
description = ""
icon = "x"

[types.risk.fields.status]
kind = "text"

[types.risk.fields.likelihood]
kind = "enum"
values = ["rare", "rare", "very_"]

[types.risk.fields.owner]
kind = "text"
values = ["a"]

[types.risk.fields.score]
kind = "number"

[types.risk.fields.level]
kind = "enum"
values = []

[types.risk.fields.notes]
kind = "text"
repeatable = "yes"
"#;
        let (schema, diagnostics) = parse(text);
        assert_eq!(
            diagnostics,
            [
                (2, "invalid-value"),
                (2, "duplicate-name"),
                (2, "invalid-value"),
                (3, "unknown-key"),
                (5, "reserved-name"),
                (9, "invalid-value"),
                (14, "invalid-value"),
                (15, "invalid-value"),
                (16, "unknown-key"),
                (18, "reserved-name"),
                (23, "duplicate-name"),
                (23, "invalid-value"),
                (27, "invalid-value"),
                (30, "invalid-value"),
                (34, "invalid-value"),
                (38, "invalid-value"),
            ]
        );
        let schema = schema.unwrap();
        assert_eq!(schema.statuses, strings(&["draft"]));
        assert_eq!(schema.types.keys().collect::<Vec<_>>(), ["risk"]);
        let risk = &schema.types["risk"];
        assert_eq!(risk.id_prefix, None);
        assert_eq!(risk.description, "");
        assert_eq!(
            risk.fields.keys().collect::<Vec<_>>(),
            ["likelihood", "notes", "owner"]
        );
        assert_eq!(
            risk.fields["likelihood"].kind,
            FieldKind::Enum(strings(&["rare"]))
        );
        assert!(!risk.fields["notes"].repeatable);
    }

    #[test]
    fn checks_relation_endpoints_and_names() {
        let text = r#"format_version = 1
statuses = ["draft"]

[types.need]
id_prefix = "NEED-"
description = "A need."

[types.need.fields.satisfies]
kind = "text"

[types.requirement]
id_prefix = "REQ-"
description = "A requirement."

[relations.satisfies]
inverse = "satisfied_by"
description = "Fulfils."
targets = { requirement = ["need", "goal", "need", "code"], need = ["need"], goal = ["need"], test = [] }

[relations.refines]
inverse = "satisfies"
description = "Refines."
targets = { requirement = ["requirement"] }

[relations.derives]
inverse = "satisfied_by"
description = "Derives."
targets = { requirement = ["requirement"] }

[relations.decomposes]
inverse = "Decomposed by"
description = "Child to parent."
same_type = true
targets = { requirement = ["requirement", "need"], code = ["need"] }

[relations.id]
inverse = "id_of"
description = "Reserved."
targets = { need = ["need"] }

[relations.empty]
inverse = "empty_of"
description = "No source."
targets = {}

[relations.missing]
description = "Nothing else."
"#;
        let (schema, diagnostics) = parse(text);
        assert_eq!(
            diagnostics,
            [
                (15, "duplicate-name"),
                (18, "undeclared-type"),
                (18, "undeclared-type"),
                (18, "duplicate-name"),
                (18, "undeclared-type"),
                (18, "undeclared-type"),
                (18, "invalid-value"),
                (21, "duplicate-name"),
                (26, "duplicate-name"),
                (31, "invalid-value"),
                (34, "invalid-value"),
                (34, "invalid-value"),
                (36, "reserved-name"),
                (44, "invalid-value"),
                (46, "missing-key"),
                (46, "missing-key"),
            ]
        );
        let schema = schema.unwrap();
        assert!(schema.types["need"].fields.is_empty());
        let satisfies = &schema.relations["satisfies"];
        assert_eq!(
            satisfies.targets,
            BTreeMap::from([
                ("need".to_owned(), strings(&["need"])),
                ("requirement".to_owned(), strings(&["need"])),
            ])
        );
        assert_eq!(satisfies.inverse.as_deref(), Some("satisfied_by"));
        assert_eq!(schema.relations["refines"].inverse, None);
        assert_eq!(schema.relations["derives"].inverse, None);
        let decomposes = &schema.relations["decomposes"];
        assert_eq!(decomposes.inverse, None);
        assert_eq!(
            decomposes.targets,
            BTreeMap::from([("requirement".to_owned(), strings(&["requirement"]))])
        );
        assert!(!schema.relations.contains_key("id"));
        assert!(schema.relations["empty"].targets.is_empty());
        assert!(!schema.relations.contains_key("missing"));
    }

    #[test]
    fn one_problem_never_hides_another() {
        let text = r#"format_version = 1
statuses = ["open"]

[types.Risk]
id_prefix = "risk"

[types.code]
description = ""

[types.risk]
id_prefix = "RISK-"
description = "A risk."

[types.risk.fields.score]
kind = "number"
required = "yes"
repeatable = "yes"

[types.risk.fields.Status]
values = ["Low"]

[types.risk.fields.level]
kind = 3
values = "high"

[relations.Mitigates]
inverse = "Mitigated by"
description = ""
targets = { goal = ["need"], code = [], risk = ["risk"] }
same_type = true
"#;
        let (schema, diagnostics) = parse(text);
        assert_eq!(
            diagnostics,
            [
                (4, "invalid-value"),
                (4, "missing-key"),
                (5, "invalid-value"),
                (7, "reserved-name"),
                (7, "missing-key"),
                (8, "invalid-value"),
                (15, "invalid-value"),
                (16, "invalid-value"),
                (17, "invalid-value"),
                (19, "invalid-value"),
                (19, "missing-key"),
                (20, "invalid-value"),
                (23, "invalid-value"),
                (24, "invalid-value"),
                (26, "invalid-value"),
                (27, "invalid-value"),
                (28, "invalid-value"),
                (29, "invalid-value"),
                (29, "invalid-value"),
                (29, "undeclared-type"),
                (29, "undeclared-type"),
            ]
        );
        let schema = schema.unwrap();
        assert_eq!(schema.types.keys().collect::<Vec<_>>(), ["risk"]);
        assert!(schema.types["risk"].fields.is_empty());
        assert!(schema.relations.is_empty());
    }

    #[test]
    fn a_collision_is_reported_whether_or_not_its_declarations_are_kept() {
        let text = r#"format_version = 1
statuses = ["open"]

[types.requirement]
id_prefix = "REQ-"
description = "A requirement."

[types.requirement.fields.refines]
kind = "number"

[relations.refines]
inverse = "same"
description = "Refines."
targets = { requirement = ["requirement"] }

[relations.derives]
inverse = "same"
description = "Derives."
"#;
        let (schema, diagnostics) = parse(text);
        assert_eq!(
            diagnostics,
            [
                (9, "invalid-value"),
                (11, "duplicate-name"),
                (16, "missing-key"),
                (17, "duplicate-name"),
            ]
        );
        let schema = schema.unwrap();
        assert!(schema.types["requirement"].fields.is_empty());
        assert_eq!(schema.relations["refines"].inverse.as_deref(), Some("same"));
        assert!(!schema.relations.contains_key("derives"));
    }

    #[test]
    fn a_repeated_name_is_reported_at_its_later_occurrence() {
        let text = r#"format_version = 1
statuses = ["open"]

[types.requirement]
id_prefix = "REQ-"
description = "A requirement."

[relations.a]
inverse = "b"
description = "A."
targets = { requirement = ["requirement"] }

[relations.b]
inverse = "b_of"
description = "B."
targets = { requirement = ["requirement"] }

[relations.c]
inverse = "c"
description = "C."
targets = { requirement = ["requirement"] }
"#;
        let (schema, diagnostics) = parse_schema(PATH, text.as_bytes());
        let printed: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
        assert_eq!(
            printed,
            [
                ".mara/schema.toml:13: error[duplicate-name]: relation name `b` repeats inverse name `b` of relation `a`",
                ".mara/schema.toml:19: error[duplicate-name]: inverse name `c` of relation `c` repeats relation name `c`",
            ]
        );
        let schema = schema.unwrap();
        assert_eq!(schema.relations["a"].inverse, None);
        assert_eq!(schema.relations["b"].inverse.as_deref(), Some("b_of"));
        assert_eq!(schema.relations["c"].inverse, None);
    }

    #[test]
    fn a_repeated_entry_is_reported_even_when_it_is_invalid() {
        let targets = "format_version = 1\nstatuses = [\"open\"]\n[relations.r]\ninverse = \"r_of\"\ndescription = \"R\"\ntargets = { code = [\"missing\", \"missing\"] }\n";
        let (_, diagnostics) = parse(targets);
        assert_eq!(
            diagnostics,
            [
                (6, "undeclared-type"),
                (6, "duplicate-name"),
                (6, "undeclared-type")
            ]
        );

        let statuses = "format_version = 1\nstatuses = [\"Open\", \"Open\", \"done\", \"done\"]\n";
        let (schema, diagnostics) = parse(statuses);
        assert_eq!(
            diagnostics,
            [
                (2, "invalid-value"),
                (2, "duplicate-name"),
                (2, "invalid-value"),
                (2, "duplicate-name")
            ]
        );
        assert_eq!(schema.unwrap().statuses, ["done"]);

        let values = "format_version = 1\nstatuses = [\"open\"]\n[types.risk]\nid_prefix = \"RISK-\"\ndescription = \"A risk.\"\n[types.risk.fields.level]\nkind = \"enum\"\nvalues = [\"High\", \"High\", \"low\"]\n";
        let (schema, diagnostics) = parse(values);
        assert_eq!(
            diagnostics,
            [
                (8, "invalid-value"),
                (8, "duplicate-name"),
                (8, "invalid-value")
            ]
        );
        assert_eq!(
            schema.unwrap().types["risk"].fields["level"].kind,
            FieldKind::Enum(strings(&["low"]))
        );
    }

    #[test]
    fn stops_at_another_format_version_or_invalid_toml() {
        let (schema, diagnostics) = parse("format_version = 2\nstatuses = 1\n");
        assert_eq!(schema, None);
        assert_eq!(diagnostics, [(1, "unsupported-format-version")]);
        let (schema, diagnostics) = parse("format_version = 1\n[types.a\n");
        assert_eq!(schema, None);
        assert_eq!(diagnostics, [(2, "toml-syntax")]);
    }

    #[test]
    fn reports_a_missing_status_list() {
        let (schema, diagnostics) = parse("format_version = 1\n");
        assert_eq!(diagnostics, [(1, "missing-key")]);
        assert!(schema.unwrap().statuses.is_empty());
    }

    #[test]
    fn recognizes_names_and_id_prefixes() {
        for name in ["risk", "derives_from", "almost-certain", "p1", "2fa"] {
            assert!(is_name(name), "{name}");
        }
        for name in ["", "Risk", "a__b", "a_", "_a", "a b", "größe", "a-_b"] {
            assert!(!is_name(name), "{name}");
        }
        for prefix in ["REQ-", "A-", "RISK2-", "SUB-SYS-"] {
            assert!(is_id_prefix(prefix), "{prefix}");
        }
        for prefix in ["", "-", "REQ", "req-", "2A-", "A--", "A-b-", "A_B-"] {
            assert!(!is_id_prefix(prefix), "{prefix}");
        }
    }
}
