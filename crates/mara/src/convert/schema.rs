//! The 0.3.0 schema, `schema.yaml`, as the vocabulary of the converted project, and the schema
//! file that replaces it (DES-CONVERSION-MAPPING in `docs/migration.mara.md`, DES-SCHEMA-MODEL in
//! `docs/project.mara.md`).

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::label_name;
use mara_engine::schema::CODE_SOURCE;
use mara_engine::write::label;

use super::report::Entry;
use super::toml_string;
use super::yaml::{Node, Value};

/// The name of the 0.3.0 field that holds an item's status.
pub const STATUS: &str = "status";

/// The vocabulary of the converted schema, with what each 0.3.0 name becomes.
#[derive(Debug, Default)]
pub struct Vocabulary {
    pub types: Vec<Type>,
    pub relations: Vec<Relation>,
    /// The status values of the flavours, each once, in their order in the schema, with their
    /// names.
    pub statuses: Vec<(String, String)>,
    /// Each name of a relation that the conversion leaves out, with the reason.
    dropped: HashMap<String, &'static str>,
}

#[derive(Debug)]
pub struct Type {
    pub name: String,
    pub id_prefix: Option<String>,
    pub description: Option<String>,
    /// Every field but the status.
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    /// The values of an enum field, each with its name; `None` for a text field.
    pub values: Option<Vec<(String, String)>>,
    pub required: bool,
    pub repeatable: bool,
}

#[derive(Debug)]
pub struct Relation {
    pub name: String,
    pub inverse: String,
    pub description: Option<String>,
    /// The allowed target types of each source type, `code` included.
    pub targets: Vec<(String, Vec<String>)>,
    pub same_type: bool,
    pub acyclic: bool,
}

/// What a metadata key or inline relation name of a 0.3.0 item is.
pub enum Role<'v> {
    /// The forward name of a relation: the item is the source.
    Forward(&'v Relation),
    /// The inverse name: the item is the target.
    Inverse(&'v Relation),
    /// A relation that the conversion leaves out, and why.
    Dropped(&'static str),
    /// No relation.
    None,
}

impl Vocabulary {
    pub fn role(&self, name: &str) -> Role<'_> {
        if let Some(why) = self.dropped.get(name) {
            return Role::Dropped(why);
        }
        for relation in &self.relations {
            if relation.name == name {
                return Role::Forward(relation);
            }
            if relation.inverse == name {
                return Role::Inverse(relation);
            }
        }
        Role::None
    }

    pub fn field(&self, flavour: &str, name: &str) -> Option<&Field> {
        let flavour = self.types.iter().find(|ty| ty.name == flavour)?;
        flavour.fields.iter().find(|field| field.name == name)
    }

    /// The status name of a 0.3.0 status value.
    pub fn status(&self, value: &str) -> Option<&str> {
        self.statuses
            .iter()
            .find(|(status, _)| status == value)
            .map(|(_, name)| name.as_str())
    }

    /// The converted schema, whose status list is `statuses`.
    pub fn render(&self, statuses: &[String]) -> String {
        let list = |names: &[String]| {
            let quoted: Vec<String> = names.iter().map(|name| toml_string(name)).collect();
            format!("[{}]", quoted.join(", "))
        };
        let mut out = format!("format_version = 1\nstatuses = {}\n", list(statuses));
        for ty in &self.types {
            let _ = write!(out, "\n[types.{}]\n", key(&ty.name));
            if let Some(prefix) = &ty.id_prefix {
                let _ = writeln!(out, "id_prefix = {}", toml_string(prefix));
            }
            if let Some(description) = &ty.description {
                let _ = writeln!(out, "description = {}", toml_string(description));
            }
            for field in &ty.fields {
                let _ = write!(
                    out,
                    "\n[types.{}.fields.{}]\n",
                    key(&ty.name),
                    key(&field.name)
                );
                match &field.values {
                    Some(values) => {
                        let names: Vec<String> =
                            values.iter().map(|(_, name)| name.clone()).collect();
                        let _ = write!(out, "kind = \"enum\"\nvalues = {}\n", list(&names));
                    }
                    None => out.push_str("kind = \"text\"\n"),
                }
                if field.required {
                    out.push_str("required = true\n");
                }
                if field.repeatable {
                    out.push_str("repeatable = true\n");
                }
            }
        }
        for relation in &self.relations {
            let _ = write!(out, "\n[relations.{}]\n", key(&relation.name));
            let _ = writeln!(out, "inverse = {}", toml_string(&relation.inverse));
            if let Some(description) = &relation.description {
                let _ = writeln!(out, "description = {}", toml_string(description));
            }
            let targets: Vec<String> = relation
                .targets
                .iter()
                .map(|(source, targets)| format!("{} = {}", key(source), list(targets)))
                .collect();
            let _ = writeln!(out, "targets = {{ {} }}", targets.join(", "));
            if relation.same_type {
                out.push_str("same_type = true\n");
            }
            if relation.acyclic {
                out.push_str("acyclic = true\n");
            }
        }
        out
    }
}

/// A TOML key: bare when it can be.
fn key(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if bare {
        name.to_owned()
    } else {
        toml_string(name)
    }
}

/// Whether `name` is a schema name: lowercase ASCII letters and digits in parts joined by single
/// `_` or `-` (DES-SCHEMA-MODEL).
pub fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.split(['_', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// The schema name that a 0.3.0 value becomes, read as a label (DES-METADATA-TABLE).
pub fn name_of(value: &str) -> String {
    label_name(value)
}

/// Reads the 0.3.0 schema at `path`, already parsed. What the converted schema leaves out joins
/// `entries`, and so does a value that cannot be read as what its key needs. Values that would
/// become the same label, or no name at all, refuse the conversion with diagnostics at their
/// lines.
pub fn read(
    path: &str,
    root: &Node,
    entries: &mut Vec<Entry>,
) -> Result<Vocabulary, Vec<Diagnostic>> {
    if root.get("flavours").is_none() && root.get("relations").is_none() {
        return Err(vec![Diagnostic::error(
            "invalid-value",
            path,
            root.line,
            format!("{path} is not a 0.3.0 schema: it has no `flavours` or `relations`"),
        )]);
    }
    let mut vocabulary = Vocabulary::default();
    let mut problems = Vec::new();
    let mut reader = Reader { path, entries };
    for line in root.merges() {
        reader.report(
            line,
            "the YAML merge key `<<` is not applied, so the properties it would bring in are \
             left out: write them out in the 0.3.0 schema and convert again, or add them to the \
             converted schema"
                .to_owned(),
        );
    }
    for (name, line, _) in root.entries() {
        if !matches!(name.as_str(), "format_version" | "flavours" | "relations") {
            reader.report(
                *line,
                format!("`{name}` is left out: a 0.4.0 schema has no such key"),
            );
        }
    }
    let mut statuses: Vec<(String, usize)> = Vec::new();
    let flavours = root.get("flavours");
    for (flavour, line, node) in flavours.into_iter().flat_map(Node::entries) {
        let place = format!("flavour `{flavour}`");
        let mut ty = Type {
            name: flavour.clone(),
            id_prefix: reader.text(node, "id_prefix", &place).map(str::to_owned),
            description: reader.text(node, "description", &place).map(str::to_owned),
            fields: Vec::new(),
        };
        let guidance: Vec<String> = ["use_when", "avoid_when", "distinguish_from"]
            .iter()
            .filter(|key| node.get(key).is_some_and(Node::is_filled))
            .map(|key| format!("`{key}`"))
            .collect();
        if !guidance.is_empty() {
            reader.report(
                *line,
                format!(
                    "the guidance {} of {place} is left out: a 0.4.0 type keeps only its \
                     description",
                    join(&guidance)
                ),
            );
        }
        for (key, key_line, value) in node.entries() {
            match key.as_str() {
                "description" | "use_when" | "avoid_when" | "distinguish_from" | "id_prefix"
                | "fields" => {}
                "body" if value.as_str() == Some("required") => {}
                "body" => reader.report(
                    *key_line,
                    format!(
                        "`body: {}` of {place} is left out: every 0.4.0 type requires a body",
                        value.as_str().unwrap_or_default()
                    ),
                ),
                other => reader.report(
                    *key_line,
                    format!("`{other}` of {place} is left out: a 0.4.0 type has no such key"),
                ),
            }
        }
        for (field, field_line, node) in node.get("fields").into_iter().flat_map(Node::entries) {
            let place = format!("field `{field}` of flavour `{flavour}`");
            let kind = reader.text(node, "type", &place).unwrap_or("string");
            for (key, key_line, _) in node.entries() {
                if !matches!(key.as_str(), "type" | "required" | "repeatable" | "values") {
                    reader.report(
                        *key_line,
                        format!("`{key}` of {place} is left out: a 0.4.0 field has no such key"),
                    );
                }
            }
            let values: Vec<(String, usize)> = reader
                .list(node, "values", &place)
                .into_iter()
                .map(|(value, line)| (value.to_owned(), line))
                .collect();
            let required = reader.flag(node, "required", &place);
            let repeatable = reader.flag(node, "repeatable", &place);
            if field == STATUS {
                for value in values {
                    if !statuses.iter().any(|(status, _)| *status == value.0) {
                        statuses.push(value);
                    }
                }
                continue;
            }
            let values = match kind {
                "enum" => Some(names(path, &place, &values, &mut problems)),
                "string" => None,
                other => {
                    reader.report(
                        *field_line,
                        format!(
                            "{place} has type `{other}` and becomes a text field: a 0.4.0 field \
                             is text or an enum"
                        ),
                    );
                    None
                }
            };
            ty.fields.push(Field {
                name: field.clone(),
                values,
                required,
                repeatable,
            });
        }
        vocabulary.types.push(ty);
    }
    vocabulary.statuses = names(
        path,
        "the flavours' `status` fields",
        &statuses,
        &mut problems,
    );
    let relations = root.get("relations");
    for (relation, line, node) in relations.into_iter().flat_map(Node::entries) {
        let place = format!("relation `{relation}`");
        let inverse = reader.text(node, "inverse", &place);
        let why = if reader.flag(node, "symmetric", &place) {
            Some("is symmetric, and a 0.4.0 relation has a direction")
        } else if reader.flag(node, "external", &place) {
            Some("leads to external addresses, and a 0.4.0 relation leads to an item")
        } else {
            None
        };
        if let Some(why) = why {
            reader.report(*line, format!("{place} is left out: it {why}"));
            vocabulary.dropped.insert(relation.clone(), why);
            if let Some(inverse) = inverse {
                vocabulary.dropped.insert(inverse.to_owned(), why);
            }
            continue;
        }
        for (key, key_line, value) in node.entries() {
            match key.as_str() {
                "description" | "source" | "target" | "same_flavour" | "inverse" | "symmetric"
                | "external" | "code_source" | "acyclic" => {}
                "cardinality" if value.is_filled() => reader.report(
                    *key_line,
                    format!(
                        "the cardinality of {place} is left out: a 0.4.0 schema declares none, \
                         and a program can check counts"
                    ),
                ),
                "cardinality" => {}
                other => reader.report(
                    *key_line,
                    format!("`{other}` of {place} is left out: a 0.4.0 relation has no such key"),
                ),
            }
        }
        let names = |list: Vec<(&str, usize)>| -> Vec<String> {
            list.into_iter().map(|(name, _)| name.to_owned()).collect()
        };
        let sources = names(reader.list(node, "source", &place));
        let targets = names(reader.list(node, "target", &place));
        let same_type = reader.flag(node, "same_flavour", &place);
        let code_source = reader.flag(node, "code_source", &place);
        let mut allowed: Vec<(String, Vec<String>)> = Vec::new();
        for source in &sources {
            let to: Vec<String> = match same_type {
                true if targets.contains(source) => vec![source.clone()],
                true => continue,
                false => targets.clone(),
            };
            if !to.is_empty() && !allowed.iter().any(|(other, _)| other == source) {
                allowed.push((source.clone(), to));
            }
        }
        if code_source && !targets.is_empty() && !same_type {
            allowed.push((CODE_SOURCE.to_owned(), targets.clone()));
        }
        let acyclic = reader.acyclic(node, &place);
        if allowed.is_empty() {
            let why = "allows no source and target type";
            reader.report(*line, format!("{place} is left out: it {why}"));
            vocabulary.dropped.insert(relation.clone(), why);
            continue;
        }
        let inverse = match inverse {
            Some(inverse) => inverse.to_owned(),
            None => {
                let generated = format!("inverse_of_{relation}");
                reader.report(
                    *line,
                    format!(
                        "{place} has no inverse name, which a 0.4.0 relation requires: it gets \
                         `{generated}`, which you can rename"
                    ),
                );
                generated
            }
        };
        vocabulary.relations.push(Relation {
            name: relation.clone(),
            inverse,
            description: reader.text(node, "description", &place).map(str::to_owned),
            targets: allowed,
            same_type,
            acyclic,
        });
    }
    match problems.is_empty() {
        true => Ok(vocabulary),
        false => Err(problems),
    }
}

/// Reads values as their keys need them, and reports a value it cannot read so: the conversion
/// then reads it as absent, never silently.
struct Reader<'e> {
    path: &'e str,
    entries: &'e mut Vec<Entry>,
}

impl Reader<'_> {
    fn report(&mut self, line: usize, message: String) {
        self.entries.push(Entry::new(self.path, line, message));
    }

    fn unread(&mut self, value: &Node, key: &str, place: &str, expected: &str) {
        self.report(
            value.line,
            format!(
                "`{key}` of {place} is left out: it is not {expected}, so the conversion cannot \
                 read it; correct it in the 0.3.0 schema and convert again"
            ),
        );
    }

    /// A flag, `false` when absent, in the spellings 0.3.0 read ([`Node::boolean`]).
    fn flag(&mut self, node: &Node, key: &str, place: &str) -> bool {
        let Some(value) = node.get(key) else {
            return false;
        };
        value.boolean().unwrap_or_else(|| {
            self.unread(value, key, place, "`true` or `false`");
            false
        })
    }

    fn text<'n>(&mut self, node: &'n Node, key: &str, place: &str) -> Option<&'n str> {
        let value = node.get(key)?;
        let text = value.as_str();
        if text.is_none() {
            self.unread(value, key, place, "text");
        }
        text
    }

    /// A list of names, each with its line; empty when absent.
    fn list<'n>(&mut self, node: &'n Node, key: &str, place: &str) -> Vec<(&'n str, usize)> {
        let Some(value) = node.get(key) else {
            return Vec::new();
        };
        let items = match &value.value {
            Value::Sequence(items) => items
                .iter()
                .map(|item| Some((item.as_str()?, item.line)))
                .collect::<Option<Vec<_>>>(),
            _ => None,
        };
        items.unwrap_or_else(|| {
            self.unread(value, key, place, "a list of names");
            Vec::new()
        })
    }

    /// Whether the relation is acyclic: an `acyclic` mapping whose `severity` is `error`, as
    /// by default. Severity `warning` is left out, since 0.4.0 reports a cycle as an error.
    fn acyclic(&mut self, node: &Node, place: &str) -> bool {
        let Some(value) = node.get("acyclic") else {
            return false;
        };
        let severity = value.get("severity").map(Node::as_str);
        match (&value.value, severity) {
            (Value::Mapping(_), None | Some(Some("error"))) => true,
            (Value::Mapping(_), Some(Some("warning"))) => {
                self.report(
                    value.line,
                    format!(
                        "the acyclic check of {place} is left out: its severity is `warning`, and \
                         0.4.0 reports a cycle as an error"
                    ),
                );
                false
            }
            _ => {
                let expected = "a mapping whose `severity` is `error` or `warning`";
                self.unread(value, "acyclic", place, expected);
                false
            }
        }
    }
}

/// `values`, each with the name it becomes. Two values that become one label, and a value that
/// becomes no name, are problems at the later value's line (DES-CONVERSION-MAPPING).
fn names(
    path: &str,
    place: &str,
    values: &[(String, usize)],
    problems: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut out = Vec::with_capacity(values.len());
    for (value, line) in values {
        let name = name_of(value);
        if !is_name(&name) {
            problems.push(Diagnostic::error(
                "invalid-value",
                path,
                *line,
                format!(
                    "the value `{value}` of {place} cannot become a 0.4.0 name, which is lowercase \
                     letters and digits in parts joined by `_` or `-`: rename it in the schema and \
                     its items, then convert again"
                ),
            ));
            continue;
        }
        match seen.get(&name) {
            Some(other) if other == value => {}
            Some(other) => problems.push(Diagnostic::error(
                "duplicate-name",
                path,
                *line,
                format!(
                    "the values `{other}` and `{value}` of {place} both become the label `{}`: \
                     rename one in the schema and its items, then convert again",
                    label(&name)
                ),
            )),
            None => {
                seen.insert(name.clone(), value);
                out.push((value.clone(), name));
            }
        }
    }
    out
}

/// `a`, `a and b`, `a, b and c`.
fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}
