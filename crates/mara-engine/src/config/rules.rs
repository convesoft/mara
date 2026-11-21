//! The programs a configuration declares and their consumers: `[rules]`, `[queries.<name>]`,
//! `[views.<name>]`, `[invariants.<name>]` and `[gates.<name>]` (DES-RULE-CONSUMERS in
//! `docs/rules/index.mara.md`).

use std::collections::BTreeMap;
use std::ops::Range;

use toml::de::DeValue;

use super::{PathSetting, path};
use crate::diagnostic::Severity;
use crate::pattern::Patterns;
use crate::toml_file::{Table, TomlFile, Value};

/// The keys of the configuration's top level.
pub(super) const ROOT_KEYS: &[&str] = &[
    "format_version",
    "schema",
    "documents",
    "code",
    "rules",
    "queries",
    "views",
    "invariants",
    "gates",
];

/// The time limit of an evaluation when `[rules]` sets none (REQ-EVALUATION-TIME-LIMIT in
/// `docs/rules/evaluation.mara.md`).
pub const DEFAULT_TIME_LIMIT_SECONDS: u32 = 10;

/// The longest time limit a configuration may set: an hour.
const MAX_TIME_LIMIT_SECONDS: i64 = 3600;

/// `[rules]` and the entries that consume its programs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    /// The patterns that declare program files.
    pub programs: Vec<PathSetting>,
    /// The time limit of one evaluation, with the line that sets it; `None` for the default.
    pub time_limit: Option<(u32, usize)>,
    /// The line of `[rules]`, 1 when the configuration has none but declares entries.
    pub line: usize,
    /// Every entry that could be read, in the order of the tables, then by name.
    pub entries: Vec<Entry>,
}

impl Rules {
    /// The time limit of one evaluation, in seconds.
    pub fn time_limit_seconds(&self) -> u32 {
        self.time_limit
            .map_or(DEFAULT_TIME_LIMIT_SECONDS, |(seconds, _)| seconds)
    }

    /// The configuration line that declares the program at `path`: the line of the first
    /// pattern that matches it, else the line of `[rules]`.
    pub fn declaring_line(&self, path: &str) -> usize {
        self.programs
            .iter()
            .find(|pattern| Patterns::new([pattern.path.as_str()]).matches(path))
            .map_or(self.line, |pattern| pattern.line)
    }
}

/// The four kinds of consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Query,
    View,
    Invariant,
    Gate,
}

impl EntryKind {
    /// Its name in a result and a message: `query`, `view`, `invariant` or `gate`.
    pub fn name(self) -> &'static str {
        match self {
            EntryKind::Query => "query",
            EntryKind::View => "view",
            EntryKind::Invariant => "invariant",
            EntryKind::Gate => "gate",
        }
    }

    /// The table that holds entries of this kind.
    pub fn table(self) -> &'static str {
        match self {
            EntryKind::Query => "queries",
            EntryKind::View => "views",
            EntryKind::Invariant => "invariants",
            EntryKind::Gate => "gates",
        }
    }
}

/// A name in the configuration, with its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub line: usize,
}

/// One configuration entry: a consumer of a program's relations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub name: String,
    /// The line of the entry's name.
    pub line: usize,
    pub program: PathSetting,
    /// Required for queries, views and gates; an invariant's defaults to its message.
    pub description: String,
    /// The inputs a request supplies (DES-REQUEST-INPUTS). An invariant has none: validation
    /// supplies no input.
    pub inputs: Vec<Named>,
    pub consumer: Consumer,
}

/// What a consumer reads of its program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Consumer {
    /// A query or a view: the rows of one relation.
    Rows { relation: Named },
    /// Each row of the relation is one failure.
    Invariant {
        relation: Named,
        severity: Severity,
        message: String,
    },
    /// DES-GATE-OUTCOME.
    Gate {
        subjects: Vec<Named>,
        conditions: Vec<Named>,
        unavailable: Vec<Named>,
    },
}

impl Entry {
    /// Every relation it reads of its program's outputs.
    pub fn relations(&self) -> Vec<&Named> {
        match &self.consumer {
            Consumer::Rows { relation } | Consumer::Invariant { relation, .. } => vec![relation],
            Consumer::Gate {
                subjects,
                conditions,
                unavailable,
            } => subjects
                .iter()
                .chain(conditions)
                .chain(unavailable)
                .collect(),
        }
    }
}

/// Reads `[rules]` and the entry tables of the configuration's top level `root`. Every problem
/// becomes a diagnostic, and an entry with one is left out.
pub(super) fn read(file: &mut TomlFile, root: &Table) -> Option<Rules> {
    let tables = [
        EntryKind::Query,
        EntryKind::View,
        EntryKind::Invariant,
        EntryKind::Gate,
    ];
    let has_entries = tables.iter().any(|kind| root.contains_key(kind.table()));
    let mut rules = match root.get_key_value("rules") {
        Some((key, value)) => {
            let table = file.table("rules", value)?;
            read_rules(file, table, key.span())
        }
        None if has_entries => {
            let first = tables
                .iter()
                .find_map(|kind| root.get_key_value(kind.table()))
                .expect("an entry table exists");
            let message = "entries consume programs, and the configuration declares none: \
                           add `[rules]` with `programs`"
                .to_owned();
            file.error("missing-key", first.0.span(), message);
            return None;
        }
        None => return None,
    };
    let patterns = Patterns::new(rules.programs.iter().map(|pattern| pattern.path.as_str()));
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    for kind in tables {
        let Some(value) = root.get(kind.table()) else {
            continue;
        };
        let Some(table) = file.table(kind.table(), value) else {
            continue;
        };
        for (key, value) in table {
            let name = key.get_ref().to_string();
            let line = file.line(key.span().start);
            // An entry with a name that cannot address it is still checked, then left out.
            let mut addressable = true;
            if !valid_name(&name) {
                let message = format!(
                    "the entry name `{name}` must start with a lowercase ASCII letter and hold only lowercase ASCII letters, digits, `-` and `_`"
                );
                file.error("invalid-value", key.span(), message);
                addressable = false;
            } else if let Some(earlier) = names.get(&name) {
                let message = format!(
                    "an entry named `{name}` already exists at line {earlier}: a request addresses an entry only by its name"
                );
                file.error("duplicate-name", key.span(), message);
                addressable = false;
            } else {
                names.insert(name.clone(), line);
            }
            let Some(table) = file.table(&name, value) else {
                continue;
            };
            let place = format!("`[{}.{name}]`", kind.table());
            let at = key.span();
            let entry = read_entry(file, table, kind, name, at, &place, &patterns);
            if let Some(entry) = entry.filter(|_| addressable) {
                rules.entries.push(entry);
            }
        }
    }
    Some(rules)
}

fn read_rules(file: &mut TomlFile, table: &Table, at: Range<usize>) -> Rules {
    const PLACE: &str = "`[rules]`";
    let line = file.line(at.start);
    file.unknown_keys(table, &["programs", "time_limit_seconds"], PLACE);
    let mut rules = Rules {
        programs: Vec::new(),
        time_limit: None,
        line,
        entries: Vec::new(),
    };
    if let Some(value) = file.required(table, "programs", at, PLACE)
        && let Some(patterns) = file.array("programs", value)
    {
        rules.programs = patterns
            .iter()
            .filter_map(|pattern| path(file, "programs", pattern))
            .collect();
    }
    if let Some(value) = table.get("time_limit_seconds") {
        let seconds = match value.get_ref() {
            DeValue::Integer(integer) => {
                i64::from_str_radix(integer.as_str(), integer.radix()).ok()
            }
            _ => None,
        };
        match seconds {
            Some(seconds @ 1..=MAX_TIME_LIMIT_SECONDS) => {
                let line = file.line(value.span().start);
                rules.time_limit = Some((seconds as u32, line));
            }
            _ => file.error(
                "invalid-value",
                value.span(),
                format!(
                    "`time_limit_seconds` must be an integer from 1 to {MAX_TIME_LIMIT_SECONDS}"
                ),
            ),
        }
    }
    rules
}

fn read_entry(
    file: &mut TomlFile,
    table: &Table,
    kind: EntryKind,
    name: String,
    at: Range<usize>,
    place: &str,
    patterns: &Patterns,
) -> Option<Entry> {
    let line = file.line(at.start);
    let mut known = vec!["program", "description"];
    known.extend(match kind {
        EntryKind::Query | EntryKind::View => &["relation", "inputs"][..],
        EntryKind::Invariant => &["relation", "severity", "message"],
        EntryKind::Gate => &["subjects", "conditions", "unavailable", "inputs"],
    });
    file.unknown_keys(table, &known, place);
    let mut complete = true;
    let program = file
        .required(table, "program", at.clone(), place)
        .and_then(|value| {
            let setting = path(file, "program", value)?;
            if !patterns.matches(&setting.path) {
                let message = format!(
                    "no pattern of `[rules] programs` declares `{}`, so it is no program",
                    setting.path
                );
                file.error("invalid-value", value.span(), message);
                return None;
            }
            Some(setting)
        });
    complete &= program.is_some();
    let description = match (kind, table.get("description")) {
        (_, Some(value)) => file.text("description", value).map(str::to_owned),
        (EntryKind::Invariant, None) => Some(String::new()),
        (_, None) => {
            file.required(table, "description", at.clone(), place);
            None
        }
    };
    complete &= description.is_some();
    let inputs = match table.get("inputs") {
        Some(value) if kind != EntryKind::Invariant => names(file, "inputs", value, true),
        _ => Some(Vec::new()),
    };
    complete &= inputs.is_some();
    let relation = |file: &mut TomlFile| {
        let value = file.required(table, "relation", at.clone(), place)?;
        let name = file.text("relation", value)?.to_owned();
        Some(Named {
            name,
            line: file.line(value.span().start),
        })
    };
    let consumer = match kind {
        EntryKind::Query | EntryKind::View => {
            relation(file).map(|relation| Consumer::Rows { relation })
        }
        EntryKind::Invariant => {
            let relation = relation(file);
            let severity = file
                .required(table, "severity", at.clone(), place)
                .and_then(|value| match file.string("severity", value)? {
                    "error" => Some(Severity::Error),
                    "warning" => Some(Severity::Warning),
                    other => {
                        let message =
                            format!("`severity` must be `error` or `warning`, not `{other}`");
                        file.error("invalid-value", value.span(), message);
                        None
                    }
                });
            let message = file
                .required(table, "message", at.clone(), place)
                .and_then(|value| file.text("message", value).map(str::to_owned));
            match (relation, severity, message) {
                (Some(relation), Some(severity), Some(message)) => Some(Consumer::Invariant {
                    relation,
                    severity,
                    message,
                }),
                _ => None,
            }
        }
        EntryKind::Gate => {
            let mut list = |key: &str, required: bool| match table.get(key) {
                Some(value) => names(file, key, value, !required),
                None if required => {
                    file.required(table, key, at.clone(), place);
                    None
                }
                None => Some(Vec::new()),
            };
            let subjects = list("subjects", true);
            let conditions = list("conditions", true);
            let unavailable = list("unavailable", false);
            match (subjects, conditions, unavailable) {
                (Some(subjects), Some(conditions), Some(unavailable)) => Some(Consumer::Gate {
                    subjects,
                    conditions,
                    unavailable,
                }),
                _ => None,
            }
        }
    };
    let (Some(program), Some(description), Some(inputs), Some(consumer), true) =
        (program, description, inputs, consumer, complete)
    else {
        return None;
    };
    let description = match (&consumer, description.is_empty()) {
        (Consumer::Invariant { message, .. }, true) => message.clone(),
        _ => description,
    };
    Some(Entry {
        kind,
        name,
        line,
        program,
        description,
        inputs,
        consumer,
    })
}

/// A list of relation names, each with its line; none repeated, and not empty unless
/// `may_be_empty`.
fn names(file: &mut TomlFile, key: &str, value: &Value, may_be_empty: bool) -> Option<Vec<Named>> {
    let values = file.array(key, value)?;
    if values.is_empty() && !may_be_empty {
        file.error(
            "invalid-value",
            value.span(),
            format!("`{key}` must name at least one relation"),
        );
        return None;
    }
    let mut named: Vec<Named> = Vec::with_capacity(values.len());
    let mut valid = true;
    for value in values {
        let Some(name) = file.text(key, value) else {
            valid = false;
            continue;
        };
        if named.iter().any(|earlier| earlier.name == name) {
            file.error(
                "duplicate-name",
                value.span(),
                format!("`{key}` names `{name}` twice"),
            );
            valid = false;
            continue;
        }
        named.push(Named {
            name: name.to_owned(),
            line: file.line(value.span().start),
        });
    }
    valid.then_some(named)
}

/// An entry name: a lowercase ASCII letter, then lowercase ASCII letters, digits, `-` and `_`.
fn valid_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;
    use crate::diagnostic::Diagnostic;

    const HEAD: &str = "format_version = 1\nschema = \"s.toml\"\n[documents]\nformat_version = 1\ninclude = [\"*.mara.md\"]\n";

    fn parse(text: &str) -> (Option<Rules>, Vec<Diagnostic>) {
        let (config, diagnostics) = parse_config(format!("{HEAD}{text}").as_bytes());
        (config.rules, diagnostics)
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<(usize, &'static str)> {
        diagnostics
            .iter()
            .map(|d| (d.line - 5, d.built_in_code()))
            .collect()
    }

    /// The example of DES-RULE-CONSUMERS in `docs/rules/index.mara.md` loads as written.
    #[test]
    fn loads_the_example_of_the_design() {
        let page = include_str!("../../../../docs/rules/index.mara.md");
        let start = page.find("```toml\n[rules]").expect("the example") + "```toml\n".len();
        let example = &page[start..start + page[start..].find("```").unwrap()];
        let (rules, diagnostics) = parse(example);
        assert_eq!(diagnostics, []);
        let rules = rules.unwrap();
        assert_eq!(rules.programs[0].path, ".mara/rules/*.dl");
        assert_eq!(rules.time_limit_seconds(), DEFAULT_TIME_LIMIT_SECONDS);
        let kinds: Vec<(EntryKind, &str)> = rules
            .entries
            .iter()
            .map(|entry| (entry.kind, entry.name.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (EntryKind::Query, "requirement-basis"),
                (EntryKind::Invariant, "requirement-verified"),
                (EntryKind::Gate, "verified"),
            ]
        );
        let invariant = &rules.entries[1];
        assert_eq!(
            invariant.description,
            "Accepted requirement has no accepted verification"
        );
        let gate = &rules.entries[2];
        let names = |list: &[&Named]| list.iter().map(|n| n.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(&gate.relations()),
            [
                "requirement",
                "not_accepted",
                "not_implemented",
                "verification_not_passed",
                "code_unavailable",
                "result_unavailable"
            ]
        );
        assert_eq!(
            names(&gate.inputs.iter().collect::<Vec<_>>()),
            ["candidate", "verification_result"]
        );
        assert_eq!(
            rules.declaring_line(".mara/rules/gates.dl"),
            rules.programs[0].line
        );
    }

    #[test]
    fn reports_each_problem_of_an_entry_at_its_line() {
        let text = "[rules]\nprograms = [\"rules/*.dl\"]\ntime_limit_seconds = 0\n\
                    [queries.q]\nprogram = \"other/q.dl\"\nrelation = \"r\"\n\
                    [views.Upper]\nprogram = \"rules/v.dl\"\n\
                    [invariants.i]\nprogram = \"rules/i.dl\"\nrelation = \"r\"\nseverity = \"fatal\"\nmessage = \"m\"\ninputs = []\n\
                    [gates.q]\nprogram = \"rules/g.dl\"\n\
                    [gates.g]\nprogram = \"rules/g.dl\"\ndescription = \"d\"\nsubjects = []\nconditions = [\"c\", \"c\"]\n";
        let (rules, diagnostics) = parse(text);
        assert_eq!(
            codes(&diagnostics),
            [
                (3, "invalid-value"),
                (4, "missing-key"),
                (5, "invalid-value"),
                (7, "invalid-value"),
                (7, "missing-key"),
                (7, "missing-key"),
                (12, "invalid-value"),
                (14, "unknown-key"),
                (15, "duplicate-name"),
                (15, "missing-key"),
                (15, "missing-key"),
                (15, "missing-key"),
                (20, "invalid-value"),
                (21, "duplicate-name"),
            ],
            "{diagnostics:#?}"
        );
        let rules = rules.unwrap();
        assert_eq!(rules.time_limit, None);
        assert_eq!(rules.entries, []);
    }

    #[test]
    fn reads_a_time_limit_and_requires_rules_for_entries() {
        let (rules, diagnostics) = parse("[rules]\nprograms = []\ntime_limit_seconds = 2\n");
        assert_eq!(diagnostics, []);
        let rules = rules.unwrap();
        assert_eq!(
            (rules.time_limit_seconds(), rules.time_limit),
            (2, Some((2, 8)))
        );
        let (rules, diagnostics) = parse("[queries.q]\nprogram = \"p.dl\"\n");
        assert_eq!(codes(&diagnostics), [(1, "missing-key")]);
        assert_eq!(rules, None);
        let (_, diagnostics) = parse("[rules]\ntime_limit_seconds = 3601\n");
        assert_eq!(
            codes(&diagnostics),
            [(1, "missing-key"), (2, "invalid-value")]
        );
    }

    /// An entry with a name that cannot address it is left out, and its other problems are
    /// still reported.
    #[test]
    fn checks_an_entry_whose_name_is_invalid_or_taken() {
        let text = "[rules]\nprograms = [\"r/*.dl\"]\n\
                    [invariants.Bad]\nprogram = \"r/p.dl\"\nrelation = \"r\"\nseverity = \"fatal\"\nmessage = \"m\"\noops = 1\n\
                    [invariants.good]\nprogram = \"r/p.dl\"\nrelation = \"r\"\nseverity = \"error\"\nmessage = \"m\"\n\
                    [queries.good]\nprogram = \"r/p.dl\"\nrelation = \"r\"\ndescription = \"d\"\nextra = 2\n";
        let (rules, diagnostics) = parse(text);
        assert_eq!(
            codes(&diagnostics),
            [
                (3, "invalid-value"),
                (6, "invalid-value"),
                (8, "unknown-key"),
                (9, "duplicate-name"),
                (18, "unknown-key"),
            ],
            "{diagnostics:#?}"
        );
        // Queries are read first, so the invariant takes a name already taken.
        let entries: Vec<_> = rules
            .unwrap()
            .entries
            .into_iter()
            .map(|e| (e.kind, e.name))
            .collect();
        assert_eq!(entries, [(EntryKind::Query, "good".to_owned())]);
    }
}
