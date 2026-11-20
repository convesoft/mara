//! Explanations from Soufflé's provenance mode (REQ-EXPLAIN-OUTCOMES and DES-RULE-EXECUTION in
//! `docs/rules/evaluation.mara.md`), without running it: the commands the host sends on
//! Soufflé's standard input, and the reading of its JSON replies into proofs whose leaves are
//! the facts and supplied inputs of the run, at their document locations.
//!
//! A proof is checked against the row it was asked for; its `subproof` references are expanded
//! with further commands; constraint leaves are left out. A row whose values the `explain`
//! command cannot take, a reply that is no proof of the row or of the subproof asked for, and a
//! proof that passes an aggregate are explanations Mara cannot give.

use std::collections::{HashMap, HashSet};

use crate::facts::{CODE_LINK, Relation, SNAPSHOT_RELATIONS, Value, write_row};
use crate::json::Object;
use crate::program::Program;

/// The commands that start an exchange: replies in JSON, then a command whose reply says that
/// the evaluation ended and its outputs are written. No admitted program declares a relation
/// whose name starts with `__`, so its reply is `Relation not found`.
pub const START: &str = "format json\nexplain __mara_ready()\n";

/// The command that ends an exchange.
pub const EXIT: &str = "exit\n";

/// The most nodes one proof may hold, its expanded subproofs included; a larger proof cannot be
/// explained.
pub const MAX_NODES: usize = 1_000;

/// The most bytes of JSON that one proof may take in a result: half a page.
pub const MAX_PROOF_BYTES: usize = crate::paging::PAGE_BYTES / 2;

/// A row of an output relation to explain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub relation: String,
    pub values: Vec<Value>,
}

/// One step of a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A row the program derived, with the rule that derived it as Soufflé prints it, and the
    /// steps of its body that are no constraint.
    Derived {
        relation: String,
        /// The row as Soufflé prints it, such as `r("a", 1)`.
        atom: String,
        rule: String,
        children: Vec<Node>,
    },
    /// A row of a fact relation, at the path and line of its item or relation row; a code link
    /// at its code file and the marker's line.
    Fact {
        relation: String,
        values: Vec<Value>,
        path: String,
        line: usize,
    },
    /// A row of an input the request supplied, with its values as supplied.
    Input { name: String, values: Vec<Value> },
    /// A negated atom of a rule: a row that holds in no relation.
    Absent { atom: String },
}

/// The explanation of one row, or why Mara cannot give it.
pub type Explanation = Result<Node, String>;

impl Node {
    /// Its JSON form: `{"relation", "atom", "rule", "children"}` for a derived row,
    /// `{"relation", "values", "path", "line"}` for a fact, `{"input", "values"}` for a supplied
    /// row and `{"absent"}` for a negated atom.
    pub fn json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        let mut object = Object::new(out);
        match self {
            Node::Derived {
                relation,
                atom,
                rule,
                children,
            } => {
                object.string("relation", relation);
                object.string("atom", atom);
                object.string("rule", rule);
                let children: Vec<String> = children.iter().map(Node::json).collect();
                object.array("children", children.iter().map(String::as_str));
            }
            Node::Fact {
                relation,
                values,
                path,
                line,
            } => {
                object.string("relation", relation);
                write_row(object.key("values"), values);
                object.string("path", path);
                object.number("line", *line as u64);
            }
            Node::Input { name, values } => {
                object.string("input", name);
                write_row(object.key("values"), values);
            }
            Node::Absent { atom } => object.string("absent", atom),
        }
        object.end();
    }
}

/// `relation(<values>)` as Soufflé prints a row in a proof: each symbol between quotes as it
/// is, without escapes, each number in decimal, separated by `, `.
pub fn atom(relation: &str, values: &[Value]) -> String {
    let values: Vec<String> = values.iter().map(printed).collect();
    format!("{relation}({})", values.join(", "))
}

fn printed(value: &Value) -> String {
    match value {
        Value::Symbol(text) => format!("\"{text}\""),
        Value::Number(number) => number.to_string(),
    }
}

/// The `explain` command for `target`, or why the command cannot name it: Soufflé's command
/// does not decode escapes, so a symbol holding a quote, a backslash or a control character,
/// and a negative number, cannot be written in it.
pub fn explain_command(target: &Target) -> Result<String, String> {
    for value in &target.values {
        match value {
            Value::Symbol(text)
                if text.contains(['"', '\\']) || text.contains(char::is_control) =>
            {
                return Err(
                    "a value of the row holds a quote, a backslash or a control character, \
                     which Soufflé's `explain` command cannot take"
                        .to_owned(),
                );
            }
            Value::Number(number) if *number < 0 => {
                return Err(
                    "a value of the row is a negative number, which Soufflé's `explain` \
                     command cannot take"
                        .to_owned(),
                );
            }
            _ => {}
        }
    }
    let values: Vec<String> = target.values.iter().map(printed).collect();
    Ok(format!(
        "explain {}({})\n",
        target.relation,
        values.join(",")
    ))
}

/// A proof as Soufflé's JSON reply holds it.
#[derive(Clone, Debug)]
enum Raw {
    /// `{"premises", "rule-number", "children"}`.
    Premises {
        atom: String,
        rule: String,
        children: Vec<Raw>,
    },
    /// `{"axiom"}`: a leaf, a constraint, a `subproof` reference, or a message such as
    /// `Tuple not found`.
    Axiom(String),
}

impl Raw {
    /// The proof in a reply's `proof` member; `None` for anything else.
    fn read(value: &serde_json::Value, depth: usize) -> Option<Raw> {
        let object = value.as_object()?;
        if depth > MAX_NODES {
            return None;
        }
        if let Some(axiom) = object.get("axiom") {
            return (object.len() == 1).then(|| Some(Raw::Axiom(axiom.as_str()?.to_owned())))?;
        }
        let atom = object.get("premises")?.as_str()?.to_owned();
        let rule = object.get("rule-number")?.as_str()?.to_owned();
        let children = object.get("children")?.as_array()?;
        let children: Option<Vec<Raw>> = children.iter().map(|c| Raw::read(c, depth + 1)).collect();
        Some(Raw::Premises {
            atom,
            rule,
            children: children?,
        })
    }

    fn nodes(&self) -> usize {
        match self {
            Raw::Premises { children, .. } => 1 + children.iter().map(Raw::nodes).sum::<usize>(),
            Raw::Axiom(_) => 1,
        }
    }

    /// The path, by child index, to the first `subproof` leaf, with the command that expands it.
    fn first_subproof(&self) -> Option<(Vec<usize>, &str)> {
        match self {
            Raw::Axiom(text) => text
                .starts_with("subproof ")
                .then(|| (Vec::new(), text.as_str())),
            Raw::Premises { children, .. } => children.iter().enumerate().find_map(|(i, child)| {
                let (mut path, text) = child.first_subproof()?;
                path.insert(0, i);
                Some((path, text))
            }),
        }
    }

    fn at(&mut self, path: &[usize]) -> &mut Raw {
        match (path.split_first(), self) {
            (None, raw) => raw,
            (Some((&index, rest)), Raw::Premises { children, .. }) => children[index].at(rest),
            (Some(_), raw) => raw,
        }
    }
}

/// The name of the relation of `atom`, such as `r` of `r("a")` or `c.r` of `c.r(1)`: the text
/// before its first `(`, when it is a name and the atom ends with `)`. A relation's name holds
/// no quote, so the values after it cannot change it.
fn relation_name(atom: &str) -> Option<&str> {
    let (name, _) = atom.split_once('(')?;
    let named = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '?' | '+' | '@'));
    (named && atom.ends_with(')')).then_some(name)
}

/// `subproof <relation>(<label>)\n`, with the relation it expands, from a leaf that refers to
/// a subproof, when it has that form; nothing else is sent.
fn subproof_command(text: &str) -> Option<(String, &str)> {
    let reference = text.strip_prefix("subproof ")?;
    let name = relation_name(reference)?;
    let label = &reference[name.len() + 1..reference.len() - 1];
    let digits = !label.is_empty() && label.bytes().all(|byte| byte.is_ascii_digit());
    digits.then(|| (format!("subproof {reference}\n"), name))
}

/// The comparisons whose leaf Soufflé 2.5 prints as `<number> <operator> <number>`, a symbol
/// as its internal number (`ExplainProvenanceImpl.h`, `constraintList` and
/// `isOrderedBinaryConstraintOp`).
pub const ORDERED_CONSTRAINTS: [&str; 6] = ["=", "!=", "<", "<=", ">=", ">"];

/// The constraints whose leaf Soufflé 2.5 prints as `<name>("<symbol>", "<symbol>")`, the
/// symbols decoded from internal numbers that need not be the ones the rule compared. A negated
/// `contains` or `match` is `not_contains` or `not_match`. `match` and `contains` are keywords;
/// the other two can be relation names.
pub const NAMED_CONSTRAINTS: [&str; 4] = ["match", "contains", "not_match", "not_contains"];

/// A leaf of a proof, by the form Soufflé gives it.
#[derive(Debug, PartialEq, Eq)]
enum Leaf<'t> {
    /// A binary constraint, which a proof leaves out.
    Constraint,
    /// `!<relation>(<values>)`, a negated atom.
    Negation,
    /// `<relation>(<values>)`, a row of a relation.
    Atom(&'t str),
}

/// A fact a program states: its values, when it writes them as literals, its path and line.
type StatedFact = (Option<Vec<Value>>, String, usize);

/// What Mara knows of a program to read what Soufflé prints of it.
///
/// Soufflé prints a string constant of a rule between quotes as it is, without escapes, so a
/// quote, a comma, an operator or a space inside one cannot be told from the rule's own text by
/// the characters alone. The reader takes, at each quote, the one constant of the program that
/// a quote closes there; when none or several of different lengths do, the text cannot be read.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    constants: Vec<String>,
    relations: HashSet<String>,
    /// The facts the program states, by relation: each with its values, when it writes them
    /// as literals, and its path and line.
    facts: HashMap<String, Vec<StatedFact>>,
}

/// A part of printed text: code, or one string constant with its quotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part<'t> {
    Code(&'t str),
    Constant(&'t str),
}

impl Vocabulary {
    /// The string constants of `program` and the names of its relations.
    pub fn of(program: &Program) -> Vocabulary {
        let mut vocabulary = Vocabulary::new(
            program.string_constants(),
            program.relations.keys().cloned(),
        );
        for fact in &program.facts {
            let location = program.location(fact.place);
            vocabulary
                .facts
                .entry(fact.relation.clone())
                .or_default()
                .push((program.fact_values(fact), location.path, location.line));
        }
        vocabulary
    }

    /// The vocabulary with the fact `values` of `relation` stated at `path` and `line`.
    pub fn with_fact(
        mut self,
        relation: &str,
        values: Vec<Value>,
        path: &str,
        line: usize,
    ) -> Self {
        let facts = self.facts.entry(relation.to_owned()).or_default();
        facts.push((Some(values), path.to_owned(), line));
        self
    }

    /// The leaf `printed` of `relation`, which the program derives and Soufflé shows without a
    /// proof: a fact the program states, at the line that states it. A relation with an
    /// aggregate in one of its rules, a row that no fact writes as Soufflé printed it, or a
    /// relation the program does not declare, such as one Soufflé makes for an aggregate,
    /// cannot be explained.
    fn stated(
        &self,
        relation: &str,
        printed: &str,
        rules: &HashMap<(String, String), String>,
    ) -> Result<Node, String> {
        let without_proof = || {
            format!(
                "a branch of the proof ends in `{relation}`, a relation that Soufflé derives \
                 without a proof, as it does for an aggregate"
            )
        };
        for ((head, _), rule) in rules {
            if head == relation && self.has_aggregate(rule) != Some(false) {
                return Err(without_proof());
            }
        }
        let facts = self.facts.get(relation).map_or(&[][..], Vec::as_slice);
        let stated = facts.iter().find_map(|(values, path, line)| {
            let values = values.as_ref()?;
            (atom(relation, values) == printed).then_some((values, path, line))
        });
        let (values, path, line) = stated.ok_or_else(without_proof)?;
        Ok(Node::Fact {
            relation: relation.to_owned(),
            values: values.clone(),
            path: path.clone(),
            line: *line,
        })
    }

    /// A vocabulary of the string constants `constants`, decoded, and the relations
    /// `relations`.
    pub fn new(
        constants: impl IntoIterator<Item = String>,
        relations: impl IntoIterator<Item = String>,
    ) -> Vocabulary {
        Vocabulary {
            constants: constants.into_iter().collect(),
            relations: relations.into_iter().collect(),
            facts: HashMap::new(),
        }
    }

    /// `text` cut into code and string constants.
    fn parts<'t>(&self, text: &'t str) -> Option<Vec<Part<'t>>> {
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(quote) = rest.find('"') {
            if quote > 0 {
                parts.push(Part::Code(&rest[..quote]));
            }
            let after = &rest[quote + 1..];
            let mut lengths = (self.constants.iter())
                .filter(|c| after.starts_with(c.as_str()) && after[c.len()..].starts_with('"'))
                .map(String::len);
            let length = lengths.next()?;
            if lengths.any(|other| other != length) {
                return None;
            }
            let end = quote + 1 + length + 1;
            parts.push(Part::Constant(&rest[quote..end]));
            rest = &rest[end..];
        }
        if !rest.is_empty() {
            parts.push(Part::Code(rest));
        }
        Some(parts)
    }

    /// `text` on one line: each run of whitespace in its code one space, its constants as
    /// they are.
    fn one_line(&self, text: &str) -> Option<String> {
        let mut out = String::with_capacity(text.len());
        for part in self.parts(text)? {
            match part {
                Part::Constant(constant) => out.push_str(constant),
                Part::Code(code) => {
                    for (index, c) in code.char_indices() {
                        if !c.is_whitespace() {
                            out.push(c);
                        } else if !code[..index].ends_with(char::is_whitespace)
                            && !(index == 0 && out.ends_with(char::is_whitespace))
                        {
                            out.push(' ');
                        }
                    }
                }
            }
        }
        Some(out.trim().to_owned())
    }

    /// The byte offsets in `text` of each `separator` in its code outside parentheses,
    /// brackets and braces, and of the closing `)` that matches the first `(`.
    fn scan(&self, text: &str, separator: &str) -> Option<(Vec<usize>, Option<usize>)> {
        let (mut separators, mut depth, mut offset) = (Vec::new(), 0usize, 0);
        let mut first_close = None;
        let mut opened = false;
        for part in self.parts(text)? {
            let code = match part {
                Part::Code(code) => code,
                Part::Constant(constant) => {
                    offset += constant.len();
                    continue;
                }
            };
            for (index, c) in code.char_indices() {
                let at = offset + index;
                match c {
                    '(' | '[' | '{' => {
                        opened |= c == '(';
                        depth += 1;
                    }
                    ')' | ']' | '}' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 && opened && first_close.is_none() {
                            first_close = Some(at);
                        }
                    }
                    _ if depth == 0 && code[index..].starts_with(separator) => separators.push(at),
                    _ => {}
                }
            }
            offset += code.len();
        }
        (depth == 0).then_some((separators, first_close))
    }

    /// `text` split at each top-level `separator` of its code.
    fn split<'t>(&self, text: &'t str, separator: &str) -> Option<Vec<&'t str>> {
        let (at, _) = self.scan(text, separator)?;
        let mut parts = Vec::with_capacity(at.len() + 1);
        let mut start = 0;
        for at in at {
            parts.push(&text[start..at]);
            start = at + separator.len();
        }
        parts.push(&text[start..]);
        Some(parts)
    }

    /// The relation of `literal` when it is one atom, `<name>(...)` with the parenthesis that
    /// opens after the name closing at its end; `None` for anything else, such as `f(x) < 2`.
    fn atom_of<'t>(&self, literal: &'t str) -> Option<&'t str> {
        let name = relation_name(literal)?;
        let (_, close) = self.scan(literal, ",")?;
        (close? == literal.len() - 1).then_some(name)
    }

    /// The body literals of `rule` as Soufflé prints it, `<head> :- <literal>, ... .`, each
    /// trimmed.
    fn body<'t>(&self, rule: &'t str) -> Option<Vec<&'t str>> {
        let (at, _) = self.scan(rule, " :- ")?;
        let body = rule[*at.first()? + " :- ".len()..]
            .trim()
            .strip_suffix('.')?;
        Some(self.split(body, ",")?.into_iter().map(str::trim).collect())
    }

    /// Whether `rule` holds an aggregate, `<operator> ... : { ... }`, in its code.
    fn has_aggregate(&self, rule: &str) -> Option<bool> {
        let parts = self.parts(rule)?;
        Some(
            parts
                .iter()
                .any(|part| matches!(part, Part::Code(code) if code.contains(": {"))),
        )
    }

    /// The form of the leaf `text` of a proof, or why it is none that Soufflé prints.
    fn leaf<'t>(&self, text: &'t str) -> Result<Leaf<'t>, String> {
        let mut words = text.splitn(3, ' ');
        let number = |word: Option<&str>| word.is_some_and(|w| w.parse::<i64>().is_ok());
        let (left, operator, right) = (words.next(), words.next(), words.next());
        if number(left)
            && number(right)
            && operator.is_some_and(|o| ORDERED_CONSTRAINTS.contains(&o))
        {
            return Ok(Leaf::Constraint);
        }
        if let Some(negated) = text.strip_prefix('!')
            && relation_name(negated).is_some()
        {
            return Ok(Leaf::Negation);
        }
        let Some(name) = relation_name(text) else {
            return Err("Soufflé's reply is not a proof of this row".to_owned());
        };
        if !NAMED_CONSTRAINTS.contains(&name) {
            return Ok(Leaf::Atom(name));
        }
        if self.relations.contains(name) {
            return Err(format!(
                "the program declares a relation `{name}`, whose rows a proof cannot tell from \
                 Soufflé's `{name}` constraint"
            ));
        }
        Ok(Leaf::Constraint)
    }
}

/// An item's index in the snapshot, its path and its line.
type Place<'a> = (usize, &'a str, usize);

/// The input relations of a run, which a proof's leaves name: the facts of the snapshot, at the
/// locations their rows give, and the inputs the request supplied.
struct Leaves<'a> {
    relations: HashMap<&'a str, &'a Relation>,
    /// Each relation's rows by the atom Soufflé prints for them, built when a leaf names it.
    printed: HashMap<&'a str, HashMap<String, Vec<usize>>>,
    /// The snapshot index, path and line of each item by its MID, built when a leaf needs
    /// them.
    items: Option<HashMap<&'a str, Vec<Place<'a>>>>,
}

impl<'a> Leaves<'a> {
    fn new(relations: &[&'a Relation]) -> Leaves<'a> {
        Leaves {
            relations: relations.iter().map(|r| (r.name.as_str(), *r)).collect(),
            printed: HashMap::new(),
            items: None,
        }
    }

    fn is_input(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    /// The leaf for `text`, a row of the input relation `name`.
    fn leaf(&mut self, name: &str, text: &str) -> Result<Node, String> {
        let relation = self.relations[name];
        let printed = self
            .printed
            .entry(relation.name.as_str())
            .or_insert_with(|| {
                let mut printed: HashMap<String, Vec<usize>> = HashMap::new();
                for (index, row) in relation.rows.iter().enumerate() {
                    printed.entry(atom(name, row)).or_default().push(index);
                }
                printed
            });
        let rows = printed.get(text).map_or(&[][..], Vec::as_slice);
        let Some(&first) = rows.first() else {
            return Err(format!(
                "a leaf of Soufflé's proof, `{text}`, is no row of `{name}` that Mara supplied"
            ));
        };
        let values = &relation.rows[first];
        if rows.iter().any(|&index| &relation.rows[index] != values) {
            return Err(format!(
                "a leaf of Soufflé's proof, `{text}`, could be several rows of `{name}`"
            ));
        }
        if !SNAPSHOT_RELATIONS.contains(&name) && name != CODE_LINK {
            return Ok(Node::Input {
                name: name.to_owned(),
                values: values.clone(),
            });
        }
        let (path, line) = self.location(name, values, text)?;
        Ok(Node::Fact {
            relation: name.to_owned(),
            values: values.clone(),
            path: path.to_owned(),
            line,
        })
    }

    /// The path and line of a fact: its own for an item or an edge; the code file and the
    /// marker's line for a code link; the item's for its title,
    /// revision or field; and for a folded text the first item, in snapshot order, whose title
    /// or field value it is. A place found through a MID that several items have is no one
    /// place, and the leaf `text` cannot be placed.
    fn location(
        &mut self,
        name: &str,
        values: &[Value],
        text: &str,
    ) -> Result<(&'a str, usize), String> {
        let unplaced = || format!("a leaf of Soufflé's proof, `{text}`, is about no item");
        let at = |path: Option<&'a Value>, line: Option<&Value>| match (path, line) {
            (Some(Value::Symbol(path)), Some(Value::Number(line))) => {
                Some((path.as_str(), *line as usize))
            }
            _ => None,
        };
        match name {
            "item" | "edge" | CODE_LINK => {
                let relation = self.relations[name];
                let row = (relation.rows.iter())
                    .find(|row| row.as_slice() == values)
                    .ok_or_else(unplaced)?;
                let (path, line) = match name {
                    "item" => (4, 5),
                    "edge" => (3, 4),
                    _ => (2, 4),
                };
                at(row.get(path), row.get(line)).ok_or_else(unplaced)
            }
            "folded" => {
                let Some(Value::Symbol(folded)) = values.first() else {
                    return Err(unplaced());
                };
                let relations = &self.relations;
                let rows =
                    |relation: &str| relations.get(relation).map_or(&[][..], |r| &r.rows[..]);
                // Each item that holds the text, as a title (rank 0) or a field value (rank 1).
                let mut holders: Vec<(&'a str, usize)> = Vec::new();
                for row in rows("title") {
                    if let [Value::Symbol(mid), Value::Symbol(title)] = row.as_slice()
                        && title == folded
                    {
                        holders.push((mid.as_str(), 0));
                    }
                }
                for row in rows("field") {
                    if let [Value::Symbol(mid), _, Value::Symbol(value)] = row.as_slice()
                        && value == folded
                    {
                        holders.push((mid.as_str(), 1));
                    }
                }
                let mut first: Option<((usize, usize), Place<'a>)> = None;
                for (mid, rank) in holders {
                    let (index, path, line) = self.only_item(mid, text)?;
                    if first.is_none_or(|(order, _)| (index, rank) < order) {
                        first = Some(((index, rank), (index, path, line)));
                    }
                }
                first
                    .map(|(_, (_, path, line))| (path, line))
                    .ok_or_else(unplaced)
            }
            _ => {
                let Some(Value::Symbol(mid)) = values.first() else {
                    return Err(unplaced());
                };
                let (_, path, line) = self.only_item(mid, text)?;
                Ok((path, line))
            }
        }
    }

    /// The snapshot index, path and line of the one item whose MID is `mid`.
    fn only_item(&mut self, mid: &str, text: &str) -> Result<Place<'a>, String> {
        let relations = &self.relations;
        let items = self.items.get_or_insert_with(|| {
            let mut items: HashMap<&'a str, Vec<Place<'a>>> = HashMap::new();
            let rows = relations.get("item").map_or(&[][..], |r| &r.rows[..]);
            for (index, row) in rows.iter().enumerate() {
                if let [
                    Value::Symbol(mid),
                    _,
                    _,
                    _,
                    Value::Symbol(path),
                    Value::Number(line),
                ] = row.as_slice()
                {
                    items.entry(mid.as_str()).or_default().push((
                        index,
                        path.as_str(),
                        *line as usize,
                    ));
                }
            }
            items
        });
        match items.get(mid).map_or(&[][..], Vec::as_slice) {
            [item] => Ok(*item),
            [] => Err(format!(
                "a leaf of Soufflé's proof, `{text}`, is about no item"
            )),
            _ => Err(format!(
                "a leaf of Soufflé's proof, `{text}`, has no one place: several items have the \
                 MID `{mid}`"
            )),
        }
    }
}

/// The proof being received for one target.
struct Current {
    target: usize,
    proof: Raw,
    /// The rules of the program by their relation and number, such as `("r", "(R1)")`.
    rules: HashMap<(String, String), String>,
    /// The path of the `subproof` leaf whose expansion was asked for, and its relation.
    expanding: Option<(Vec<usize>, String)>,
}

/// The exchange that explains a list of rows: it gives the next command to send and takes each
/// reply, in turn.
pub struct Explainer<'a> {
    vocabulary: Vocabulary,
    leaves: Leaves<'a>,
    targets: Vec<Target>,
    explanations: Vec<Option<Explanation>>,
    current: Option<Current>,
    /// Whether a command was sent whose reply has not come.
    waiting: bool,
    /// Whether a reply came that no command awaited.
    stray: bool,
    next: usize,
}

impl<'a> Explainer<'a> {
    /// An exchange that explains `targets`, rows of the run's outputs, with `relations`, the
    /// fact relations and the supplied inputs that the run read, and `vocabulary`, the
    /// program's.
    pub fn new(
        vocabulary: Vocabulary,
        relations: &[&'a Relation],
        targets: Vec<Target>,
    ) -> Explainer<'a> {
        let explanations = vec![None; targets.len()];
        Explainer {
            vocabulary,
            leaves: Leaves::new(relations),
            targets,
            explanations,
            current: None,
            waiting: false,
            stray: false,
            next: 0,
        }
    }

    /// The rows to explain, in the order given.
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// The next command to send, or `None` when every row has its explanation or the exchange
    /// broke off.
    pub fn command(&mut self) -> Option<String> {
        if self.waiting || self.stray {
            return None;
        }
        if let Some(current) = &mut self.current {
            let (path, text) = current.proof.first_subproof()?;
            let (command, relation) = subproof_command(text)?;
            current.expanding = Some((path, relation.to_owned()));
            self.waiting = true;
            return Some(command);
        }
        while self.next < self.targets.len() {
            let index = self.next;
            self.next += 1;
            match explain_command(&self.targets[index]) {
                Ok(command) => {
                    self.current = Some(Current {
                        target: index,
                        proof: Raw::Axiom(String::new()),
                        rules: HashMap::new(),
                        expanding: None,
                    });
                    self.waiting = true;
                    return Some(command);
                }
                Err(reason) => self.explanations[index] = Some(Err(reason)),
            }
        }
        None
    }

    /// Takes the reply to the last command: a JSON object `{"proof", "rules"}`. A reply that no
    /// command awaits breaks the exchange off: no later reply can be paired with its command.
    pub fn reply(&mut self, reply: &serde_json::Value) {
        let current = match (self.waiting, self.current.as_mut()) {
            (true, Some(current)) => current,
            _ => {
                self.stray = true;
                return;
            }
        };
        self.waiting = false;
        let target = current.target;
        let refuse = |reason: &str| Some(Err(reason.to_owned()));
        let not_a_proof = "Soufflé's reply is not a proof of this row";
        let proof = reply.get("proof").and_then(|proof| Raw::read(proof, 0));
        let rules = reply.get("rules").and_then(rules);
        let (Some(proof), Some(rules)) = (proof, rules) else {
            self.current = None;
            self.explanations[target] = refuse(not_a_proof);
            return;
        };
        match current.expanding.take() {
            None => {
                let wanted = &self.targets[target];
                let wanted_atom = atom(&wanted.relation, &wanted.values);
                let matches = match &proof {
                    Raw::Premises { atom, .. } => *atom == wanted_atom,
                    Raw::Axiom(atom) => {
                        *atom == wanted_atom && self.leaves.is_input(&wanted.relation)
                    }
                };
                if !matches {
                    self.current = None;
                    self.explanations[target] = refuse(not_a_proof);
                    return;
                }
                current.proof = proof;
                current.rules = rules;
            }
            Some((path, relation)) => {
                // The expansion must be a step of the relation the reference names.
                let fits = matches!(&proof, Raw::Premises { atom, .. }
                    if relation_name(atom) == Some(relation.as_str()));
                if !fits {
                    self.current = None;
                    self.explanations[target] = refuse(not_a_proof);
                    return;
                }
                *current.proof.at(&path) = proof;
                current.rules.extend(rules);
            }
        }
        if current.proof.nodes() > MAX_NODES {
            self.current = None;
            self.explanations[target] =
                Some(Err(format!("the proof holds more than {MAX_NODES} steps")));
            return;
        }
        match current.proof.first_subproof() {
            Some((_, text)) if subproof_command(text).is_some() => return,
            Some(_) => {
                self.current = None;
                self.explanations[target] = refuse(not_a_proof);
                return;
            }
            None => {}
        }
        let current = self.current.take().expect("received");
        let explanation = convert(
            &current.proof,
            &current.rules,
            &self.vocabulary,
            &mut self.leaves,
        );
        let explanation = explanation.and_then(|node| {
            let bytes = node.json().len();
            if bytes > MAX_PROOF_BYTES {
                return Err(format!(
                    "the proof takes {bytes} bytes, more than the {MAX_PROOF_BYTES} a result \
                     gives one"
                ));
            }
            Ok(node)
        });
        self.explanations[target] = Some(explanation);
    }

    /// The explanation of each row, in the order given: a row whose exchange did not end gets
    /// `reason`, such as the time limit or an aborted run.
    pub fn finish(self, reason: &str) -> Vec<Explanation> {
        let reason = match self.stray {
            true => "Soufflé sent a reply that no command asked for",
            false => reason,
        };
        self.explanations
            .into_iter()
            .map(|explanation| explanation.unwrap_or_else(|| Err(reason.to_owned())))
            .collect()
    }
}

/// The rules of a reply by their relation, the name before the head's `(`, and number.
fn rules(value: &serde_json::Value) -> Option<HashMap<(String, String), String>> {
    let mut rules = HashMap::new();
    for rule in value.as_array()? {
        let number = rule.get("rule-number")?.as_str()?;
        let text = rule.get("rule")?.as_str()?;
        if let Some((head, _)) = text.split_once('(') {
            rules.insert((head.to_owned(), number.to_owned()), text.to_owned());
        }
    }
    Some(rules)
}

/// The proof `raw` with its leaves found among the inputs, its constraint leaves left out, each
/// rule written on one line; or why it cannot be explained.
///
/// Soufflé lists a step's atoms and negations in the order of its rule, then its constraints
/// (`UnitTranslator.cpp`, the info relations of provenance). So no atom or negation follows a
/// constraint, the atoms and negations are as many as the rule's, and the nth negation is the
/// rule's nth negated literal.
fn convert(
    raw: &Raw,
    rules: &HashMap<(String, String), String>,
    vocabulary: &Vocabulary,
    leaves: &mut Leaves<'_>,
) -> Result<Node, String> {
    let not_a_proof = || "Soufflé's reply is not a proof of this row".to_owned();
    match raw {
        Raw::Premises {
            atom,
            rule,
            children,
        } => {
            let relation = relation_name(atom).ok_or_else(not_a_proof)?;
            let text = rules
                .get(&(relation.to_owned(), rule.clone()))
                .ok_or_else(not_a_proof)?;
            let unreadable = || {
                format!(
                    "the rule of `{relation}` that Soufflé printed cannot be read with the \
                     program's string constants"
                )
            };
            if vocabulary.has_aggregate(text).ok_or_else(unreadable)? {
                return Err(format!(
                    "the proof passes an aggregate in a rule of `{relation}`, whose rows \
                     Soufflé does not explain"
                ));
            }
            let one_line = vocabulary.one_line(text).ok_or_else(unreadable)?;
            let body = vocabulary.body(text).ok_or_else(unreadable)?;
            let negated: Vec<&str> = (body.iter().copied())
                .filter(|literal| {
                    literal
                        .strip_prefix('!')
                        .is_some_and(|atom| vocabulary.atom_of(atom).is_some())
                })
                .collect();
            let steps = (body.iter())
                .filter(|literal| {
                    let atom = literal.strip_prefix('!').unwrap_or(literal);
                    vocabulary.atom_of(atom).is_some_and(|name| {
                        !NAMED_CONSTRAINTS.contains(&name) || vocabulary.relations.contains(name)
                    })
                })
                .count();
            let mut negations = negated.iter();
            let mut nodes = Vec::new();
            let mut constraints = false;
            for child in children {
                let node = match child {
                    Raw::Axiom(printed) => match vocabulary.leaf(printed)? {
                        Leaf::Constraint => {
                            constraints = true;
                            continue;
                        }
                        Leaf::Negation => {
                            let literal = negations.next().ok_or_else(not_a_proof)?;
                            absent(printed, literal, vocabulary).ok_or_else(not_a_proof)?
                        }
                        Leaf::Atom(name) if leaves.is_input(name) => leaves.leaf(name, printed)?,
                        Leaf::Atom(name) => vocabulary.stated(name, printed, rules)?,
                    },
                    Raw::Premises { .. } => convert(child, rules, vocabulary, leaves)?,
                };
                if constraints {
                    return Err(not_a_proof());
                }
                nodes.push(node);
            }
            if nodes.len() != steps || negations.next().is_some() {
                return Err(not_a_proof());
            }
            Ok(Node::Derived {
                relation: relation.to_owned(),
                atom: atom.clone(),
                rule: one_line,
                children: nodes,
            })
        }
        Raw::Axiom(text) => match vocabulary.leaf(text)? {
            Leaf::Atom(name) if leaves.is_input(name) => leaves.leaf(name, text),
            _ => Err(not_a_proof()),
        },
    }
}

/// The leaf of the negated atom `printed`, `literal` being the negated literal of the rule that
/// it stands for. Soufflé prints arbitrary values for the literal's wildcards, so a literal with a
/// wildcard is shown as the rule writes it, and one without as Soufflé printed it, with its
/// values. `None` when the two do not fit together.
fn absent(printed: &str, literal: &str, vocabulary: &Vocabulary) -> Option<Node> {
    let printed = printed.strip_prefix('!')?;
    let literal = literal.strip_prefix('!')?;
    let name = vocabulary.atom_of(literal)?;
    if relation_name(printed)? != name {
        return None;
    }
    let arguments = vocabulary.split(&literal[name.len() + 1..literal.len() - 1], ",")?;
    let wildcard = arguments.iter().any(|argument| {
        let argument = argument.trim();
        argument == "_" || argument.starts_with("+underscore")
    });
    let atom = match wildcard {
        true => literal,
        false => printed,
    };
    Some(Node::Absent {
        atom: atom.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn symbol(text: &str) -> Value {
        Value::Symbol(text.to_owned())
    }

    fn item_relation() -> Relation {
        let row = |mid: &str, line| {
            vec![
                symbol(mid),
                symbol("requirement"),
                symbol(&mid.to_uppercase()),
                symbol("accepted"),
                symbol("docs/a.md"),
                Value::Number(line),
            ]
        };
        Relation::new("item", vec![row("a", 3), row("b", 9)])
    }

    fn target(relation: &str, values: &[&str]) -> Target {
        Target {
            relation: relation.to_owned(),
            values: values.iter().map(|value| symbol(value)).collect(),
        }
    }

    fn rules() -> serde_json::Value {
        json!([
            {"rule-number": "(R1)", "rule": "r(m) :- \n   s(m)."},
            {"rule-number": "(R1)", "rule": "s(m) :- \n   item(m,_,_,_,_,_)."},
            {"rule-number": "(R2)", "rule": "s(m) :- \n   t(m, c),\n   c = count : { item(_,_,_,_,_,_) }."},
        ])
    }

    fn vocabulary(constants: &[&str]) -> Vocabulary {
        Vocabulary::new(
            constants.iter().map(|c| (*c).to_owned()),
            ["r", "s", "q", "t", "item", "field", "denied"].map(str::to_owned),
        )
    }

    /// The one explanation of `relation(values)` from one reply `proof` with `rules`.
    fn explain_one(
        vocabulary: Vocabulary,
        relations: &[&Relation],
        wanted: Target,
        proof: serde_json::Value,
        rules: serde_json::Value,
    ) -> Explanation {
        let mut explainer = Explainer::new(vocabulary, relations, vec![wanted]);
        explainer.command().expect("a command");
        explainer.reply(&json!({"proof": proof, "rules": rules}));
        assert_eq!(explainer.command(), None);
        explainer.finish("unfinished").remove(0)
    }

    fn leaf(text: &str) -> serde_json::Value {
        json!({"axiom": text})
    }

    fn step(atom: &str, rule: &str, children: Vec<serde_json::Value>) -> serde_json::Value {
        json!({"premises": atom, "rule-number": rule, "children": children})
    }

    #[test]
    fn a_proof_of_another_row_is_no_explanation() {
        let item = item_relation();
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], vec![target("r", &["a"])]);
        assert_eq!(explainer.command().as_deref(), Some("explain r(\"a\")\n"));
        let other = step("r(\"b\")", "(R1)", vec![leaf("s(\"b\")")]);
        explainer.reply(&json!({"proof": other, "rules": rules()}));
        assert_eq!(explainer.command(), None);
        let explanations = explainer.finish("unused");
        assert_eq!(
            explanations,
            [Err("Soufflé's reply is not a proof of this row".to_owned())]
        );
    }

    #[test]
    fn subproofs_are_expanded_and_leaves_found_at_their_rows() {
        let item = item_relation();
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], vec![target("r", &["b"])]);
        explainer.command();
        let proof = step("r(\"b\")", "(R1)", vec![leaf("subproof s(4)")]);
        explainer.reply(&json!({"proof": proof, "rules": rules()}));
        assert_eq!(explainer.command().as_deref(), Some("subproof s(4)\n"));
        let item_atom = "item(\"b\", \"requirement\", \"B\", \"accepted\", \"docs/a.md\", 9)";
        let sub = step("s(\"b\")", "(R1)", vec![leaf(item_atom), leaf("3 < 4")]);
        explainer.reply(&json!({"proof": sub, "rules": rules()}));
        assert_eq!(explainer.command(), None);
        let [Ok(Node::Derived { children, .. })] = &explainer.finish("unused")[..] else {
            panic!("not explained");
        };
        let [Node::Derived { children, rule, .. }] = &children[..] else {
            panic!("{children:?}");
        };
        assert_eq!(rule, "s(m) :- item(m,_,_,_,_,_).");
        let [Node::Fact { path, line, .. }] = &children[..] else {
            panic!("{children:?}");
        };
        assert_eq!((path.as_str(), *line), ("docs/a.md", 9));
    }

    #[test]
    fn a_reply_that_is_no_proof_or_comes_unasked_ends_the_explanations() {
        let item = item_relation();
        let targets = vec![target("r", &["a"]), target("r", &["b"])];
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], targets.clone());
        explainer.command();
        explainer.reply(&json!({"proof": leaf("Tuple not found"), "rules": []}));
        explainer.command();
        explainer.reply(&json!({"proof": 5, "rules": []}));
        let not_a_proof = Err("Soufflé's reply is not a proof of this row".to_owned());
        assert_eq!(explainer.finish("x"), [not_a_proof.clone(), not_a_proof]);
        // A reply that no command awaits: no later reply can be paired with its row.
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], targets);
        explainer.command();
        explainer.reply(&json!({"proof": leaf("Tuple not found"), "rules": []}));
        explainer.reply(&json!({"proof": leaf("Tuple not found"), "rules": []}));
        assert_eq!(explainer.command(), None);
        let explanations = explainer.finish("x");
        assert_eq!(
            explanations[1],
            Err("Soufflé sent a reply that no command asked for".to_owned())
        );
    }

    #[test]
    fn an_aggregate_or_a_leaf_of_several_rows_is_not_explained() {
        let item = item_relation();
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], vec![target("r", &["a"])]);
        explainer.command();
        let proof = step(
            "r(\"a\")",
            "(R1)",
            vec![step(
                "s(\"a\")",
                "(R2)",
                vec![leaf("t(\"a\", 2)"), leaf("2 = 2")],
            )],
        );
        explainer.reply(&json!({"proof": proof, "rules": rules()}));
        let [Err(reason)] = &explainer.finish("x")[..] else {
            panic!("explained");
        };
        assert!(reason.contains("aggregate"), "{reason}");
        // Two rows that Soufflé prints alike.
        let quoted = Relation::new(
            "t",
            vec![
                vec![symbol("a\", \"b"), symbol("c")],
                vec![symbol("a"), symbol("b\", \"c")],
            ],
        );
        let rules = json!([{"rule-number": "(R1)", "rule": "r(m) :- \n   t(m,_)."}]);
        let mut explainer = Explainer::new(vocabulary(&[]), &[&quoted], vec![target("r", &["x"])]);
        explainer.command();
        let proof = step("r(\"x\")", "(R1)", vec![leaf("t(\"a\", \"b\", \"c\")")]);
        explainer.reply(&json!({"proof": proof, "rules": rules}));
        let [Err(reason)] = &explainer.finish("x")[..] else {
            panic!("explained");
        };
        assert!(reason.contains("several rows"), "{reason}");
    }

    #[test]
    fn rows_the_command_cannot_take_send_no_command() {
        let targets = vec![
            target("r", &["a\"b"]),
            target("r", &["a\\b"]),
            target("r", &["a\nb"]),
            Target {
                relation: "r".to_owned(),
                values: vec![Value::Number(-1)],
            },
        ];
        let mut explainer = Explainer::new(vocabulary(&[]), &[], targets);
        assert_eq!(explainer.command(), None);
        let explanations = explainer.finish("x");
        assert!(
            explanations
                .iter()
                .all(|e| e.as_ref().is_err_and(|r| r.contains("cannot take")))
        );
    }

    /// Finding 1 of the review of `aeb4975`: an operator inside a quoted value of a negation
    /// does not make it a constraint, and a negation the proof leaves out is no proof.
    #[test]
    fn a_negation_holding_an_operator_stays_in_the_proof() {
        let item = item_relation();
        let rules = json!([{"rule-number": "(R1)",
            "rule": "r(m) :- \n   item(m,_,\"A\",_,_,_),\n   !denied(m,\"a = b\")."}]);
        let item_atom = "item(\"a\", \"requirement\", \"A\", \"accepted\", \"docs/a.md\", 3)";
        let negation = "!denied(\"a\", \"a = b\")";
        let proof = step("r(\"a\")", "(R1)", vec![leaf(item_atom), leaf(negation)]);
        let explained = explain_one(
            vocabulary(&["A", "a = b"]),
            &[&item],
            target("r", &["a"]),
            proof,
            rules.clone(),
        );
        let Ok(Node::Derived { children, .. }) = explained else {
            panic!("{explained:?}");
        };
        assert_eq!(
            children[1],
            Node::Absent {
                atom: "denied(\"a\", \"a = b\")".to_owned()
            }
        );
        let without = step("r(\"a\")", "(R1)", vec![leaf(item_atom)]);
        let explained = explain_one(
            vocabulary(&["A", "a = b"]),
            &[&item],
            target("r", &["a"]),
            without,
            rules,
        );
        assert_eq!(
            explained,
            Err("Soufflé's reply is not a proof of this row".to_owned())
        );
    }

    /// Finding 2: a rule's string constants keep every character on one line, quotes and runs
    /// of spaces included; a rule whose constants the program does not have is not read.
    #[test]
    fn a_rule_on_one_line_keeps_its_constants() {
        let field = Relation::new(
            "field",
            vec![vec![symbol("a"), symbol("owner"), symbol("a  b")]],
        );
        let item = item_relation();
        let rule = "r(m) :- \n   field(m,\"owner\",\"a  b\"),\n   m != \"q\"z, \"y\"\".";
        let rules = json!([{"rule-number": "(R1)", "rule": rule}]);
        let proof = || {
            step(
                "r(\"a\")",
                "(R1)",
                vec![leaf("field(\"a\", \"owner\", \"a  b\")"), leaf("3 != 9")],
            )
        };
        let constants = ["owner", "a  b", "q\"z, \"y\""];
        let relations = [&item, &field];
        let explained = explain_one(
            vocabulary(&constants),
            &relations,
            target("r", &["a"]),
            proof(),
            rules.clone(),
        );
        let Ok(Node::Derived { rule, .. }) = explained else {
            panic!("{explained:?}");
        };
        assert_eq!(
            rule,
            "r(m) :- field(m,\"owner\",\"a  b\"), m != \"q\"z, \"y\"\"."
        );
        let explained = explain_one(
            vocabulary(&["owner"]),
            &relations,
            target("r", &["a"]),
            proof(),
            rules,
        );
        assert!(
            explained
                .as_ref()
                .is_err_and(|reason| reason.contains("cannot be read")),
            "{explained:?}"
        );
    }

    /// Finding 3: every constraint form that Soufflé 2.5 prints in a proof is left out, and a
    /// named one is no constraint where the program declares a relation of its name.
    #[test]
    fn every_constraint_form_is_left_out() {
        let item = item_relation();
        let item_atom = "item(\"a\", \"requirement\", \"A\", \"accepted\", \"docs/a.md\", 3)";
        let forms = [
            "5 = 5",
            "-1 != 0",
            "1 < 2",
            "1 <= 2",
            "3 >= -2147483648",
            "3 > 2",
            "match(\"a\", \"b\")",
            "contains(\"x, y\", \"z\")",
            "not_match(\"a\", \"b\")",
            "not_contains(\"@\", \"abc\")",
        ];
        let body: Vec<String> = std::iter::once("item(m,_,_,_,_,_)".to_owned())
            .chain((0..forms.len()).map(|n| format!("m != \"{n}\"")))
            .collect();
        let rule = format!("r(m) :- \n   {}.", body.join(",\n   "));
        let rules = json!([{"rule-number": "(R1)", "rule": rule}]);
        let mut children = vec![leaf(item_atom)];
        children.extend(forms.iter().map(|form| leaf(form)));
        let constants: Vec<String> = (0..forms.len()).map(|n| n.to_string()).collect();
        let constants: Vec<&str> = constants.iter().map(String::as_str).collect();
        let proof = step("r(\"a\")", "(R1)", children.clone());
        let explained = explain_one(
            vocabulary(&constants),
            &[&item],
            target("r", &["a"]),
            proof,
            rules.clone(),
        );
        let Ok(Node::Derived {
            children: steps, ..
        }) = &explained
        else {
            panic!("{explained:?}");
        };
        assert_eq!(steps.len(), 1, "{explained:?}");
        // A program with a relation `not_contains`: the leaf could be either.
        let declared = Vocabulary::new(
            constants.iter().map(|c| (*c).to_owned()),
            ["r", "item", "not_contains"].map(str::to_owned),
        );
        let proof = step("r(\"a\")", "(R1)", children);
        let explained = explain_one(declared, &[&item], target("r", &["a"]), proof, rules);
        assert!(
            explained
                .as_ref()
                .is_err_and(|reason| reason.contains("not_contains")),
            "{explained:?}"
        );
        // An atom after a constraint is no proof Soufflé gives.
        let rules = json!([{"rule-number": "(R1)", "rule": "r(m) :- \n   item(m,_,_,_,_,_),\n   m != \"0\"."}]);
        let proof = step("r(\"a\")", "(R1)", vec![leaf("1 != 2"), leaf(item_atom)]);
        let explained = explain_one(
            vocabulary(&["0"]),
            &[&item],
            target("r", &["a"]),
            proof,
            rules,
        );
        assert!(explained.is_err(), "{explained:?}");
    }

    /// Finding 4: a leaf placed through a MID that several items have has no one place; an
    /// item or edge leaf keeps the place its own row gives.
    #[test]
    fn a_leaf_through_a_shared_mid_is_not_placed() {
        let row = |line| {
            vec![
                symbol("a"),
                symbol("requirement"),
                symbol(&format!("A{line}")),
                symbol("accepted"),
                symbol("docs/a.md"),
                Value::Number(line),
            ]
        };
        let item = Relation::new("item", vec![row(5), row(19)]);
        let field = Relation::new(
            "field",
            vec![vec![symbol("a"), symbol("owner"), symbol("abc")]],
        );
        let title = Relation::new("title", vec![vec![symbol("a"), symbol("T")]]);
        let revision = Relation::new("revision", vec![vec![symbol("a"), symbol("h")]]);
        let folded = Relation::new("folded", vec![vec![symbol("T"), symbol("t")]]);
        let relations = [&item, &field, &title, &revision, &folded];
        for leaf_atom in [
            "field(\"a\", \"owner\", \"abc\")",
            "title(\"a\", \"T\")",
            "revision(\"a\", \"h\")",
            "folded(\"T\", \"t\")",
        ] {
            let name = relation_name(leaf_atom).unwrap();
            let rule = format!("r(m) :- \n   {name}(m,_).");
            let rules = json!([{"rule-number": "(R1)", "rule": rule}]);
            let proof = step("r(\"a\")", "(R1)", vec![leaf(leaf_atom)]);
            let explained = explain_one(
                vocabulary(&[]),
                &relations,
                target("r", &["a"]),
                proof,
                rules,
            );
            assert!(
                explained
                    .as_ref()
                    .is_err_and(|reason| reason.contains("several items have the MID `a`")),
                "{leaf_atom}: {explained:?}"
            );
        }
        let rules = json!([{"rule-number": "(R1)", "rule": "r(m) :- \n   item(m,_,_,_,_,_)."}]);
        let item_atom = "item(\"a\", \"requirement\", \"A19\", \"accepted\", \"docs/a.md\", 19)";
        let proof = step("r(\"a\")", "(R1)", vec![leaf(item_atom)]);
        let explained = explain_one(
            vocabulary(&[]),
            &relations,
            target("r", &["a"]),
            proof,
            rules,
        );
        let Ok(Node::Derived { children, .. }) = explained else {
            panic!("{explained:?}");
        };
        assert!(
            matches!(&children[0], Node::Fact { line: 19, .. }),
            "{children:?}"
        );
    }

    /// Finding 5: an expansion that proves a row of another relation than the reference names
    /// is no part of the proof.
    #[test]
    fn a_subproof_of_another_relation_is_refused() {
        let item = item_relation();
        let mut explainer = Explainer::new(vocabulary(&[]), &[&item], vec![target("r", &["a"])]);
        explainer.command();
        let proof = step("r(\"a\")", "(R1)", vec![leaf("subproof s(4)")]);
        explainer.reply(&json!({"proof": proof, "rules": rules()}));
        assert_eq!(explainer.command().as_deref(), Some("subproof s(4)\n"));
        let item_atom = "item(\"b\", \"requirement\", \"B\", \"accepted\", \"docs/a.md\", 9)";
        let other = step("q(\"b\")", "(R1)", vec![leaf(item_atom)]);
        let rules = json!([{"rule-number": "(R1)", "rule": "q(m) :- \n   item(m,_,_,_,_,_)."}]);
        explainer.reply(&json!({"proof": other, "rules": rules}));
        assert_eq!(explainer.command(), None);
        assert_eq!(
            explainer.finish("x"),
            [Err("Soufflé's reply is not a proof of this row".to_owned())]
        );
    }

    /// A leaf of a relation the program states by facts is at the line of the fact that writes
    /// it; a relation with an aggregate in one of its rules, or a row no fact writes, is not.
    #[test]
    fn a_fact_of_the_program_is_placed_at_its_line() {
        let item = item_relation();
        let rules = json!([{"rule-number": "(R1)", "rule": "r(m) :- \n   item(m,_,_,_,_,_),\n   k(\"x\")."}]);
        let item_atom = "item(\"a\", \"requirement\", \"A\", \"accepted\", \"docs/a.md\", 3)";
        let proof = || step("r(\"a\")", "(R1)", vec![leaf(item_atom), leaf("k(\"x\")")]);
        let stated = vocabulary(&["x"]).with_fact("k", vec![symbol("x")], ".mara/rules/p.dl", 7);
        let explained = explain_one(
            stated.clone(),
            &[&item],
            target("r", &["a"]),
            proof(),
            rules.clone(),
        );
        let Ok(Node::Derived { children, .. }) = explained else {
            panic!("{explained:?}");
        };
        assert_eq!(
            children[1],
            Node::Fact {
                relation: "k".to_owned(),
                values: vec![symbol("x")],
                path: ".mara/rules/p.dl".to_owned(),
                line: 7
            }
        );
        let other = vocabulary(&["x"]).with_fact("k", vec![symbol("y")], ".mara/rules/p.dl", 7);
        let explained = explain_one(other, &[&item], target("r", &["a"]), proof(), rules.clone());
        assert!(explained.is_err(), "{explained:?}");
        let mut with_aggregate = rules.as_array().unwrap().clone();
        with_aggregate.push(json!({"rule-number": "(R1)", "rule": "k(\"x\") :- \n   1 = count : { item(_,_,_,_,_,_) }."}));
        let explained = explain_one(
            stated,
            &[&item],
            target("r", &["a"]),
            proof(),
            json!(with_aggregate),
        );
        assert!(
            explained
                .as_ref()
                .is_err_and(|reason| reason.contains("aggregate")),
            "{explained:?}"
        );
    }
}
