//! The built-in checks of DES-SCHEMA-CHECKS: metadata, identity, links and relations. One pass
//! collects what each item's checks read of it, one checks each item and its rows, one each
//! edge, then one the edges of each acyclic relation. Every lookup across items, rows or schema
//! lists is by hash, so the work is linear in items, rows and edges.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::{Finding, list};
use crate::diagnostic::Diagnostic;
use crate::document::{Link, Row, is_mid, label_name};
use crate::schema::{FieldKind, RelationDecl, Schema, TypeDecl};
use crate::snapshot::{Edge, Snapshot};
use crate::work::WorkCounts;
use crate::write::{label, link, relative};

/// The most characters of a value that a message quotes, so that a diagnostic stays small.
const QUOTED: usize = 100;

pub(super) fn run(snapshot: &Snapshot, work: &mut WorkCounts) -> Vec<Finding> {
    let schema = snapshot.schema().map(|schema| &**schema);
    let lookups = schema.map(Lookups::new).unwrap_or_default();
    let facts = collect_facts(snapshot, work);
    let mut checker = Checker {
        snapshot,
        schema,
        lookups: &lookups,
        facts: &facts,
        work,
        findings: Vec::new(),
        acyclic: BTreeMap::new(),
    };
    checker.parse_problems();
    for item in 0..snapshot.items().len() {
        checker.item(item);
    }
    for item in 0..snapshot.items().len() {
        for edge in snapshot.outgoing(item) {
            checker.edge(edge);
        }
    }
    for document in 0..snapshot.documents().len() {
        for edge in snapshot.narrative(document) {
            checker.edge(edge);
        }
    }
    checker.cycles();
    checker.findings
}

struct Checker<'s, 'w> {
    snapshot: &'s Snapshot,
    schema: Option<&'s Schema>,
    lookups: &'w Lookups<'s>,
    /// By snapshot item.
    facts: &'w [Facts<'s>],
    work: &'w mut WorkCounts,
    findings: Vec<Finding>,
    /// The edges of each acyclic relation between items that exist: source, target and the
    /// line of the source's row, in edge order, which is path and line order.
    acyclic: BTreeMap<&'s str, Vec<(usize, usize, usize)>>,
}

/// What the checks read of an item more than once: its first `Type` and `ID` rows, wherever
/// they stand among its rows (DES-METADATA-TABLE).
struct Facts<'s> {
    /// The schema name that the first `Type` value matches.
    kind: String,
    /// The first `ID` value.
    id: Option<&'s str>,
}

/// The schema's lists as sets, built once, so that no row or edge scans a list.
#[derive(Default)]
struct Lookups<'s> {
    statuses: HashSet<&'s str>,
    /// The values of each enum field, by type and field name.
    values: HashMap<(&'s str, &'s str), HashSet<&'s str>>,
    /// The target types of each relation and source type.
    targets: HashMap<(&'s str, &'s str), HashSet<&'s str>>,
    /// The required fields of each type.
    required: HashMap<&'s str, Vec<&'s str>>,
}

impl<'s> Lookups<'s> {
    fn new(schema: &'s Schema) -> Lookups<'s> {
        let mut lookups = Lookups {
            statuses: schema.statuses.iter().map(String::as_str).collect(),
            ..Lookups::default()
        };
        for (kind, declaration) in &schema.types {
            for (name, field) in &declaration.fields {
                if let FieldKind::Enum(values) = &field.kind {
                    let values = values.iter().map(String::as_str).collect();
                    lookups.values.insert((kind, name), values);
                }
                if field.required {
                    lookups.required.entry(kind).or_default().push(name);
                }
            }
        }
        for (relation, declaration) in &schema.relations {
            for (source, targets) in &declaration.targets {
                let targets = targets.iter().map(String::as_str).collect();
                lookups.targets.insert((relation, source), targets);
            }
        }
        lookups
    }
}

/// How a row repeats an earlier row of its item.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Repeat {
    /// No earlier row has its key.
    First,
    /// The earlier row at this line has its key and another value.
    Other(usize),
    /// An earlier row has its key and value.
    Same,
}

/// The facts of every item of `snapshot`, one check step each.
fn collect_facts<'s>(snapshot: &'s Snapshot, work: &mut WorkCounts) -> Vec<Facts<'s>> {
    (0..snapshot.items().len())
        .map(|index| {
            work.check_steps += 1;
            let (mut kind, mut id) = (None, None);
            for row in &snapshot.item(index).rows {
                match row.name.as_str() {
                    "type" if kind.is_none() => kind = Some(row.value.as_str()),
                    "id" if id.is_none() => id = Some(row.value.as_str()),
                    _ => {}
                }
            }
            Facts {
                kind: label_name(kind.unwrap_or_default()),
                id,
            }
        })
        .collect()
}

impl<'s, 'w> Checker<'s, 'w> {
    /// The parse problems of each document, each with the item that holds its line.
    fn parse_problems(&mut self) {
        for document in self.snapshot.documents() {
            let mut next = document.items.start;
            for diagnostic in document.file.document.diagnostics(&document.path) {
                while next < document.items.end
                    && self.snapshot.item(next).end_line < diagnostic.line
                {
                    next += 1;
                }
                let item = (next < document.items.end
                    && self.snapshot.item(next).line <= diagnostic.line)
                    .then_some(next);
                self.push(diagnostic, item, true);
            }
        }
    }

    /// The identity of the item at `index`, and each of its rows.
    fn item(&mut self, index: usize) {
        self.work.check_steps += 1;
        let item = self.snapshot.item(index);
        let same_mid = self.snapshot.items_with_mid(&item.mid);
        if let Some((other, primary)) = shared(same_mid, index) {
            let message = format!(
                "MID {} is also the MID of {}: every item has its own MID",
                item.mid,
                self.describe(other)
            );
            self.report(index, item.line, "duplicate-mid", message, primary);
        }
        // The first `Type` row gives the type that the other rows are checked against.
        let facts = self.facts;
        let kind = facts[index].kind.as_str();
        let declared = self.schema.and_then(|schema| schema.types.get(kind));

        // The line of the first row of each name, and of each key and value pair.
        let mut first: HashMap<&str, usize> = HashMap::new();
        let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
        for row in &item.rows {
            // Every row's value is checked on its own, a repeated one included.
            let repeat = match seen.get(&(row.name.as_str(), row.value.as_str())) {
                Some(&line) => {
                    let message = format!(
                        "this row repeats the `{}` row at line {line} with the same value",
                        row.key
                    );
                    self.error(index, row.line, "duplicate-row", message);
                    Repeat::Same
                }
                None => {
                    seen.insert((&row.name, &row.value), row.line);
                    match first.get(row.name.as_str()) {
                        Some(&line) => Repeat::Other(line),
                        None => Repeat::First,
                    }
                }
            };
            first.entry(&row.name).or_insert(row.line);
            self.row(index, row, kind, declared, repeat);
        }

        let title = quote(&item.title);
        for (name, label) in [("id", "ID"), ("status", "Status")] {
            if !first.contains_key(name) {
                let message =
                    format!("the item `{title}` has no `{label}` row, which every item has");
                self.error(index, item.line, "missing-row", message);
            }
        }
        let lookups = self.lookups;
        let required = lookups.required.get(kind).into_iter().flatten();
        for name in required.filter(|name| !first.contains_key(**name)) {
            let message = format!(
                "the item `{title}` has no `{}` row, which type `{kind}` requires",
                label(name)
            );
            self.error(index, item.line, "missing-row", message);
        }
    }

    /// One row of the item at `index`, whose type is `kind`, declared as `declared`, and how
    /// it repeats an earlier row.
    fn row(
        &mut self,
        index: usize,
        row: &Row,
        kind: &str,
        declared: Option<&TypeDecl>,
        repeat: Repeat,
    ) {
        let single = |what: &str| match repeat {
            Repeat::Other(line) => Some(format!(
                "`{}` takes one value{what}, and line {line} already gives one",
                row.key
            )),
            Repeat::First | Repeat::Same => None,
        };
        match row.name.as_str() {
            "type" => {
                if let Some(message) = single(": the first one is the item's type") {
                    self.error(index, row.line, "repeated-key", message);
                }
                if let Some(schema) = self.schema
                    && !schema.types.contains_key(&label_name(&row.value))
                {
                    let message = format!("the schema declares no type `{}`", quote(&row.value));
                    self.error(index, row.line, "undeclared-type", message);
                }
            }
            "id" => {
                if let Some(message) = single(": every item has exactly one ID") {
                    self.error(index, row.line, "repeated-key", message);
                }
                self.id(index, row, kind, declared, repeat);
            }
            "status" => {
                if let Some(message) = single("") {
                    self.error(index, row.line, "repeated-key", message);
                }
                // A schema whose statuses did not load reports that itself.
                let statuses = &self.lookups.statuses;
                if !statuses.is_empty() && !statuses.contains(label_name(&row.value).as_str()) {
                    let all = self.schema.map_or(&[][..], |schema| &schema.statuses);
                    let message = format!(
                        "`{}` is not a status of the schema: {}",
                        quote(&row.value),
                        labels(all)
                    );
                    self.error(index, row.line, "invalid-value", message);
                }
            }
            name => {
                let field = declared.and_then(|declaration| declaration.fields.get(name));
                let relation = self
                    .schema
                    .and_then(|schema| schema.relation_from(kind, name));
                if let Some(field) = field {
                    if !field.repeatable
                        && let Some(message) = single("")
                    {
                        self.error(index, row.line, "repeated-key", message);
                    }
                    if let FieldKind::Enum(values) = &field.kind
                        && !self.lookups.values[&(kind, name)]
                            .contains(label_name(&row.value).as_str())
                    {
                        let message = format!(
                            "`{}` is not a value of `{}`: {}",
                            quote(&row.value),
                            row.key,
                            labels(values)
                        );
                        self.error(index, row.line, "invalid-value", message);
                    }
                } else if relation.is_some() {
                    self.relation_value(index, row);
                } else if declared.is_some() {
                    // Neither a field of the type nor a relation that leads from it: the row is
                    // kept, and is no edge (DES-SNAPSHOT).
                    let declared_elsewhere = self
                        .schema
                        .is_some_and(|schema| schema.relations.contains_key(name));
                    let message = match declared_elsewhere {
                        true => format!(
                            "relation `{name}` does not lead from type `{kind}`, which declares \
                             no field `{}` either",
                            row.key
                        ),
                        false => {
                            format!("type `{kind}` declares no field or relation `{}`", row.key)
                        }
                    };
                    self.error(index, row.line, "undeclared-key", message);
                }
            }
        }
    }

    /// The form and prefix of an `ID` row of the item at `index`, and the uniqueness of its
    /// value, once per value: a row that repeats it shares that problem with its first row.
    fn id(
        &mut self,
        index: usize,
        row: &Row,
        kind: &str,
        declared: Option<&TypeDecl>,
        repeat: Repeat,
    ) {
        let id = row.value.as_str();
        if !is_id(id) {
            let message = format!(
                "`{}` is not an ID: uppercase letters and digits in two or more parts joined by \
                 `-`, the first starting with a letter, such as `REQ-SIGN-IN`",
                quote(id)
            );
            self.error(index, row.line, "invalid-id", message);
        }
        if let Some(prefix) = declared.and_then(|declaration| declaration.id_prefix.as_deref())
            && !id.starts_with(prefix)
        {
            let message = format!(
                "`{}` does not start with `{prefix}`, the ID prefix of type `{kind}`",
                quote(id)
            );
            self.error(index, row.line, "wrong-id-prefix", message);
        }
        if repeat == Repeat::Same {
            return;
        }
        if let Some((other, primary)) = shared(self.snapshot.items_with_id(id), index) {
            let message = format!(
                "ID `{}` is also the ID of {}: every item has its own ID",
                quote(id),
                self.describe(other)
            );
            self.report(index, row.line, "duplicate-id", message, primary);
        }
    }

    /// A relation row whose value is no link to a MID, which therefore is no edge.
    fn relation_value(&mut self, index: usize, row: &Row) {
        let fragment = row.link.as_ref().and_then(|link| link.fragment.as_deref());
        if fragment.is_some_and(is_mid) {
            return;
        }
        if row.link.is_none() && is_id(&row.value) {
            let replacement = match self.snapshot.items_with_id(&row.value) {
                &[target] => {
                    let (from, to) = (self.path(index), self.path(target));
                    let mid = &self.snapshot.item(target).mid;
                    format!(": write `{}`", link(&row.value, mid, from, to))
                }
                [] => ", and no item has this ID".to_owned(),
                _ => String::new(),
            };
            let message = format!(
                "the value of `{}` is the bare ID `{}`, not a link `[ID](path#MID)`{replacement}",
                row.key, row.value
            );
            self.error(index, row.line, "bare-id-value", message);
        } else {
            let message = format!(
                "the value of `{}` is not a link to an item, `[ID](path#MID)`: `{}`",
                row.key,
                quote(&row.value)
            );
            self.error(index, row.line, "invalid-relation-value", message);
        }
    }

    /// The target, path and link text of one edge, and for a relation the target's type.
    fn edge(&mut self, edge: &'s Edge) {
        self.work.check_steps += 1;
        let snapshot = self.snapshot;
        let link = snapshot.link(edge);
        let relation = snapshot.relation(edge);
        let from = snapshot.documents()[edge.document].path.as_str();
        let mid = link.fragment.as_deref().expect("an edge links to a MID");
        let target = match snapshot.items_with_mid(mid) {
            [] => {
                let message = match relation {
                    Some(_) => format!("no item has MID {mid}, the target of this relation"),
                    None => format!("no item has MID {mid}, the target of this link"),
                };
                return self.at(edge.item, from, link.line, "broken-link", message);
            }
            &[target] => target,
            // A repeated MID is reported at its items.
            _ => return,
        };
        let to = self.path(target);
        if resolve(from, &link.path).as_deref() != Some(to) {
            let written = match link.path.as_str() {
                "" => "this file".to_owned(),
                path => format!("`{}`", quote(path)),
            };
            let message = format!(
                "the link names {written}, but MID {mid} is in {to}: write `{}#{mid}`",
                relative(from, to)
            );
            self.at(edge.item, from, link.line, "stale-link-path", message);
        }
        let (Some(relation), Some(source)) = (relation, edge.item) else {
            return;
        };
        if let Some(id) = self.facts[target].id
            && link.text != id
        {
            let message = format!(
                "the link text `{}` is not `{id}`, the ID of the item with MID {mid}",
                quote(&link.text)
            );
            self.error(source, link.line, "stale-link-text", message);
        }
        let Some((name, declaration)) = self
            .schema
            .and_then(|schema| schema.relations.get_key_value(relation))
        else {
            return;
        };
        self.target_type(source, target, link, name, declaration);
        if declaration.acyclic {
            self.acyclic
                .entry(name.as_str())
                .or_default()
                .push((source, target, link.line));
        }
    }

    /// Whether the relation `name`, declared as `declaration`, leads from the source's type to
    /// the target's.
    fn target_type(
        &mut self,
        source: usize,
        target: usize,
        link: &Link,
        name: &str,
        declaration: &RelationDecl,
    ) {
        let facts = self.facts;
        let (source_type, target_type) = (&facts[source].kind, &facts[target].kind);
        let declared = self
            .schema
            .is_some_and(|schema| schema.types.contains_key(target_type));
        // An undeclared target type is reported at the target.
        let Some(allowed) = self.lookups.targets.get(&(name, source_type.as_str())) else {
            return;
        };
        if !declared || allowed.contains(target_type.as_str()) {
            return;
        }
        let same = if declaration.same_type {
            ", the source's own type"
        } else {
            ""
        };
        let message = format!(
            "this relation leads from type `{source_type}` only to {}{same}, and {} has type \
             `{target_type}`",
            names(&declaration.targets[source_type]),
            self.describe(target)
        );
        self.error(source, link.line, "disallowed-target", message);
    }

    /// Reports each cycle of each acyclic relation once to the project, at its first edge in
    /// path and line order, and to each item on it at the item's first edge on it.
    fn cycles(&mut self) {
        let acyclic = std::mem::take(&mut self.acyclic);
        for (relation, edges) in acyclic {
            let mut local: HashMap<usize, usize> = HashMap::new();
            let mut nodes: Vec<usize> = Vec::new();
            let mut adjacency: Vec<Vec<usize>> = Vec::new();
            let mut node = |item: usize, adjacency: &mut Vec<Vec<usize>>| {
                *local.entry(item).or_insert_with(|| {
                    nodes.push(item);
                    adjacency.push(Vec::new());
                    nodes.len() - 1
                })
            };
            let mut ends = Vec::with_capacity(edges.len());
            for &(source, target, _) in &edges {
                let (s, t) = (node(source, &mut adjacency), node(target, &mut adjacency));
                adjacency[s].push(t);
                ends.push((s, t));
            }
            self.work.check_steps += (nodes.len() + edges.len()) as u64;
            let component = components(&adjacency);
            let mut size = vec![0usize; nodes.len()];
            for &c in &component {
                size[c] += 1;
            }
            let cyclic = |s: usize, t: usize| {
                component[s] == component[t] && (size[component[s]] > 1 || s == t)
            };
            // Each cycle's first edge of each member, in edge order, then its members.
            struct Cycle {
                firsts: Vec<(usize, usize)>,
                members: Vec<usize>,
            }
            let mut cycles: Vec<Cycle> = Vec::new();
            let mut cycle_of: HashMap<usize, usize> = HashMap::new();
            let mut reported: HashSet<usize> = HashSet::new();
            for (&(s, t), &(source, _, line)) in ends.iter().zip(&edges) {
                if !cyclic(s, t) {
                    continue;
                }
                let at = *cycle_of.entry(component[s]).or_insert_with(|| {
                    cycles.push(Cycle {
                        firsts: Vec::new(),
                        members: Vec::new(),
                    });
                    cycles.len() - 1
                });
                if reported.insert(source) {
                    cycles[at].firsts.push((source, line));
                }
            }
            for (node, item) in nodes.iter().enumerate() {
                if let Some(&at) = cycle_of.get(&component[node]) {
                    cycles[at].members.push(*item);
                }
            }
            for Cycle {
                firsts,
                mut members,
            } in cycles
            {
                members.sort_unstable();
                let named = members.iter().map(|&item| self.name(item));
                let message = format!(
                    "the `{}` links of {} form a cycle, which relation `{relation}` forbids",
                    label(relation),
                    list(named, members.len())
                );
                for (position, (source, line)) in firsts.into_iter().enumerate() {
                    self.report(
                        source,
                        line,
                        "relation-cycle",
                        message.clone(),
                        position == 0,
                    );
                }
            }
        }
    }

    /// The path of the document that holds the item at `index`.
    fn path(&self, index: usize) -> &'s str {
        let snapshot = self.snapshot;
        &snapshot.documents()[snapshot.items()[index].document].path
    }

    /// The item at `index` by its ID, or by its MID when it has no ID.
    fn name(&self, index: usize) -> String {
        match self.facts[index].id {
            Some(id) => format!("`{}`", quote(id)),
            None => format!("the item with MID {}", self.snapshot.item(index).mid),
        }
    }

    /// The item at `index` by its name and location.
    fn describe(&self, index: usize) -> String {
        let line = self.snapshot.item(index).line;
        format!("{} at {}:{line}", self.name(index), self.path(index))
    }

    fn error(&mut self, item: usize, line: usize, code: &'static str, message: String) {
        self.report(item, line, code, message, true);
    }

    fn report(
        &mut self,
        item: usize,
        line: usize,
        code: &'static str,
        message: String,
        primary: bool,
    ) {
        let diagnostic = Diagnostic::error(code, self.path(item), line, message);
        self.push(diagnostic, Some(item), primary);
    }

    /// An error at `line` of the document at `path`, in the item at `item` if any.
    fn at(
        &mut self,
        item: Option<usize>,
        path: &str,
        line: usize,
        code: &'static str,
        message: String,
    ) {
        self.push(Diagnostic::error(code, path, line, message), item, true);
    }

    fn push(&mut self, mut diagnostic: Diagnostic, item: Option<usize>, primary: bool) {
        if let Some(index) = item {
            diagnostic.id = self.facts[index].id.map(str::to_owned);
            diagnostic.mid = Some(self.snapshot.item(index).mid.clone());
        }
        self.findings.push(Finding {
            diagnostic,
            item,
            primary,
        });
    }
}

/// When `holders`, distinct items in path and line order, one of them `index`, share a key:
/// another holder to name, and whether `index` comes after the first holder, which makes it a
/// place where the project reports the repetition. Constant time.
fn shared(holders: &[usize], index: usize) -> Option<(usize, bool)> {
    match holders {
        [] | [_] => None,
        [first, second, ..] if *first == index => Some((*second, false)),
        [first, ..] => Some((*first, true)),
    }
}

/// Whether `text` has the form of an ID: `[A-Z][A-Z0-9]*(-[A-Z0-9]+)+` (DES-ITEM-IDS).
pub fn is_id(text: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    };
    let mut parts = text.split('-');
    let head = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    head.starts_with(|c: char| c.is_ascii_uppercase())
        && part(head)
        && !rest.is_empty()
        && rest.into_iter().all(part)
}

/// The labels of `names`, each in backticks, as [`list`] lists them.
fn labels(names: &[String]) -> String {
    list(
        names.iter().map(|name| format!("`{}`", label(name))),
        names.len(),
    )
}

/// `names`, each in backticks, as [`list`] lists them.
fn names(names: &[String]) -> String {
    list(names.iter().map(|name| format!("`{name}`")), names.len())
}

/// At most [`QUOTED`] characters of `text`, with `…` when it is longer.
fn quote(text: &str) -> String {
    match text.char_indices().nth(QUOTED) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

/// The project path that a link's `path`, written in the document at `from`, names: the
/// document itself when it is empty. `None` when it is absolute, leaves the project root or is
/// not UTF-8 once percent-decoded.
pub(crate) fn resolve(from: &str, path: &str) -> Option<String> {
    if path.is_empty() {
        return Some(from.to_owned());
    }
    if path.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(path)?;
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// `path` with each `%XX` escape decoded, as a browser reads a link's path.
fn percent_decode(path: &str) -> Option<String> {
    if !path.contains('%') {
        return Some(path.to_owned());
    }
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escape = (bytes[at] == b'%')
            .then(|| path.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escape {
            Some(byte) => {
                decoded.push(byte);
                at += 3;
            }
            None => {
                decoded.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

/// The strongly connected component of each node of the graph `adjacency`, by Tarjan's
/// algorithm without recursion: linear in nodes plus edges.
fn components(adjacency: &[Vec<usize>]) -> Vec<usize> {
    const UNSEEN: usize = usize::MAX;
    let count = adjacency.len();
    let mut index = vec![UNSEEN; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut component = vec![UNSEEN; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut calls: Vec<(usize, usize)> = Vec::new();
    let (mut next_index, mut next_component) = (0, 0);
    for start in 0..count {
        if index[start] != UNSEEN {
            continue;
        }
        calls.push((start, 0));
        index[start] = next_index;
        low[start] = next_index;
        next_index += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&(node, edge)) = calls.last() {
            if let Some(&next) = adjacency[node].get(edge) {
                calls.last_mut().expect("a call is running").1 += 1;
                if index[next] == UNSEEN {
                    index[next] = next_index;
                    low[next] = next_index;
                    next_index += 1;
                    stack.push(next);
                    on_stack[next] = true;
                    calls.push((next, 0));
                } else if on_stack[next] {
                    low[node] = low[node].min(index[next]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(caller, _)) = calls.last() {
                low[caller] = low[caller].min(low[node]);
            }
            if low[node] == index[node] {
                loop {
                    let member = stack.pop().expect("the component is on the stack");
                    on_stack[member] = false;
                    component[member] = next_component;
                    if member == node {
                        break;
                    }
                }
                next_component += 1;
            }
        }
    }
    component
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_ids() {
        for id in ["REQ-A", "REQ-SIGN-IN", "A1-2", "X-1-B2"] {
            assert!(is_id(id), "{id}");
        }
        for id in [
            "REQ", "REQ-", "-A", "req-a", "1A-B", "REQ--A", "REQ-a", "REQ_A", "",
        ] {
            assert!(!is_id(id), "{id}");
        }
    }

    #[test]
    fn resolves_link_paths_from_the_linking_file() {
        let cases = [
            ("docs/a.mara.md", "", Some("docs/a.mara.md")),
            ("docs/a.mara.md", "b.mara.md", Some("docs/b.mara.md")),
            ("docs/a.mara.md", "./b.mara.md", Some("docs/b.mara.md")),
            (
                "docs/x/a.mara.md",
                "../y/b.mara.md",
                Some("docs/y/b.mara.md"),
            ),
            (
                "docs/a.mara.md",
                "my%20b.mara.md",
                Some("docs/my b.mara.md"),
            ),
            ("docs/a.mara.md", "../../b.mara.md", None),
            ("docs/a.mara.md", "/docs/b.mara.md", None),
            ("a.mara.md", "%FF.mara.md", None),
        ];
        for (from, path, expected) in cases {
            assert_eq!(resolve(from, path).as_deref(), expected, "{from} {path}");
        }
    }

    #[test]
    fn resolves_the_paths_the_writer_writes() {
        for (from, to) in [
            ("docs/x/a.mara.md", "docs/y/b.mara.md"),
            ("a.mara.md", "d/e/f.mara.md"),
            ("d/e/f.mara.md", "a.mara.md"),
            ("a.mara.md", "my notes (1)#%.mara.md"),
        ] {
            assert_eq!(resolve(from, &relative(from, to)).as_deref(), Some(to));
        }
    }

    #[test]
    fn finds_strongly_connected_components() {
        // 0 → 1 → 2 → 0 is a cycle; 3 → 3 a loop; 4 → 0 enters the cycle.
        let adjacency = vec![vec![1], vec![2], vec![0], vec![3], vec![0]];
        let component = components(&adjacency);
        assert_eq!(component[0], component[1]);
        assert_eq!(component[1], component[2]);
        assert_ne!(component[3], component[0]);
        assert_ne!(component[4], component[0]);
        assert_ne!(component[4], component[3]);
    }

    #[test]
    fn quotes_at_most_a_hundred_characters() {
        assert_eq!(quote("short"), "short");
        let long = "é".repeat(150);
        assert_eq!(quote(&long), format!("{}…", "é".repeat(100)));
    }
}
