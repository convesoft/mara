//! Search and `item list`, and the filters they share (REQ-SEARCH-ITEMS-AND-NARRATIVE,
//! REQ-SEARCH-RANKING, REQ-TYPO-TOLERANT-SEARCH and REQ-LIST-ITEMS in
//! `docs/retrieval/index.mara.md`).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::diagnostic::Diagnostic;
use crate::document::{Node, Zone, label_name, words};
use crate::json::Object;
use crate::paging::PAGE_BYTES;
use crate::schema::FieldKind;
use crate::snapshot::Snapshot;

use crate::snapshot::edits_allowed;

use super::distance::within;
use super::node::{ItemSummary, Named, NodeSummary};
use super::page::{self, Envelope};
use super::{Documents, Page, Paging, ReadError, TOO_LARGE, result_key};

/// Filters of search and `item list`. Values within one filter are alternatives; filters, and
/// field filters of different keys, all apply. Every value matches exactly; keys, type names,
/// relation names, `Status` values and enum values match as labels do (DES-METADATA-TABLE).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    /// Type names.
    pub types: Vec<String>,
    /// Field keys, each with a value its row holds.
    pub fields: Vec<(String, String)>,
    /// Forward names, matching items with such an outgoing relation, and inverse names, matching
    /// items that such a relation targets.
    pub relations: Vec<String>,
    /// Document paths and directories from the project root.
    pub paths: Vec<String>,
    /// `ID` values and MIDs.
    pub ids: Vec<String>,
}

impl Filters {
    /// The filters' values as parts of a request key, normalized.
    fn key_parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        let mut add = |name: &str, mut values: Vec<String>| {
            values.sort_unstable();
            values.dedup();
            parts.push(format!("{name}={}", values.len()));
            parts.extend(values);
        };
        add("type", self.types.iter().map(|t| label_name(t)).collect());
        add(
            "field",
            self.fields
                .iter()
                .map(|(key, value)| format!("{}={value}", label_name(key)))
                .collect(),
        );
        add(
            "relation",
            self.relations.iter().map(|r| label_name(r)).collect(),
        );
        add(
            "path",
            self.paths.iter().map(|p| directory(p).to_owned()).collect(),
        );
        add("id", self.ids.clone());
        parts
    }
}

/// `path` without a trailing `/`.
fn directory(path: &str) -> &str {
    path.strip_suffix('/').unwrap_or(path)
}

/// The filters, ready to test nodes of one snapshot.
struct Matcher<'s> {
    snapshot: &'s Snapshot,
    types: BTreeSet<String>,
    /// Values by key name.
    fields: BTreeMap<String, Vec<&'s str>>,
    /// Relations that a matching item is the source of, and those that target it.
    outgoing: BTreeSet<String>,
    incoming: BTreeSet<String>,
    relations: bool,
    paths: Vec<&'s str>,
    ids: BTreeSet<&'s str>,
}

impl<'s> Matcher<'s> {
    fn new(snapshot: &'s Snapshot, filters: &'s Filters) -> Matcher<'s> {
        let mut fields: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for (key, value) in &filters.fields {
            fields.entry(label_name(key)).or_default().push(value);
        }
        let (mut outgoing, mut incoming) = (BTreeSet::new(), BTreeSet::new());
        if let Some(schema) = snapshot.schema() {
            for name in filters.relations.iter().map(|r| label_name(r)) {
                for (forward, relation) in &schema.relations {
                    if *forward == name {
                        outgoing.insert(forward.clone());
                    }
                    if relation.inverse.as_deref() == Some(name.as_str()) {
                        incoming.insert(forward.clone());
                    }
                }
            }
        }
        Matcher {
            snapshot,
            types: filters.types.iter().map(|t| label_name(t)).collect(),
            fields,
            outgoing,
            incoming,
            relations: !filters.relations.is_empty(),
            paths: filters.paths.iter().map(|p| directory(p)).collect(),
            ids: filters.ids.iter().map(String::as_str).collect(),
        }
    }

    /// Whether only items can match: a narrative unit has no type, field, relation or ID.
    fn items_only(&self) -> bool {
        !self.types.is_empty() || !self.fields.is_empty() || self.relations || !self.ids.is_empty()
    }

    fn matches(&self, node: Node) -> bool {
        match node {
            Node::Item(item) => self.item(item),
            Node::Unit(unit) => {
                !self.items_only() && self.path(self.snapshot.units()[unit].document)
            }
        }
    }

    fn path(&self, document: usize) -> bool {
        let path = self.snapshot.documents()[document].path.as_str();
        self.paths.is_empty()
            || self.paths.iter().any(|filter| {
                path == *filter
                    || path
                        .strip_prefix(filter)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
    }

    fn item(&self, index: usize) -> bool {
        let snapshot = self.snapshot;
        let item = snapshot.item(index);
        if !self.path(snapshot.items()[index].document) {
            return false;
        }
        let type_name = item.value("type").map(label_name).unwrap_or_default();
        if !self.types.is_empty() && !self.types.contains(&type_name) {
            return false;
        }
        if !self.ids.is_empty()
            && !self.ids.contains(item.mid.as_str())
            && !item
                .rows
                .iter()
                .any(|row| row.name == "id" && self.ids.contains(row.value.as_str()))
        {
            return false;
        }
        let declared = snapshot
            .schema()
            .and_then(|schema| schema.types.get(&type_name));
        for (key, values) in &self.fields {
            // `Type`, `Status` and enum values are labels; any other value is text as written.
            let label = matches!(key.as_str(), "type" | "status")
                || declared.is_some_and(|declared| {
                    matches!(
                        declared.fields.get(key).map(|field| &field.kind),
                        Some(FieldKind::Enum(_))
                    )
                });
            let holds = |value: &str, wanted: &str| {
                if label {
                    label_name(value) == label_name(wanted)
                } else {
                    value == wanted
                }
            };
            let found = item.rows.iter().any(|row| {
                row.name == *key && values.iter().any(|wanted| holds(&row.value, wanted))
            });
            if !found {
                return false;
            }
        }
        if self.relations {
            let outgoing = snapshot.outgoing(index).iter().any(|edge| {
                snapshot
                    .relation(edge)
                    .is_some_and(|name| self.outgoing.contains(name))
            });
            let incoming = !self.incoming.is_empty()
                && snapshot.incoming(&item.mid).any(|edge| {
                    snapshot
                        .relation(edge)
                        .is_some_and(|name| self.incoming.contains(name))
                });
            if !outgoing && !incoming {
                return false;
            }
        }
        true
    }
}

/// A search result: the node and one excerpt of its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub node: NodeSummary,
    pub excerpt: Excerpt,
}

/// A line of a node that holds a query word, or a word within its edits, or else the node's first
/// line of text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Excerpt {
    pub line: usize,
    /// The line without the spaces around it; at most [`EXCERPT_CHARS`] characters of it,
    /// starting up to [`EXCERPT_LEAD`] characters before the query word.
    pub text: String,
}

pub const EXCERPT_CHARS: usize = 240;
pub const EXCERPT_LEAD: usize = 60;

/// How one query word matches one node.
#[derive(Clone, Copy, Debug, Default)]
struct WordMatch {
    exact: bool,
    /// An exact match in the ID, the title or a heading.
    exact_high: bool,
    /// Any match there, typo-tolerant ones included.
    high: bool,
    /// Where the node's first exact occurrence of the word starts in the source.
    exact_at: Option<usize>,
    /// Where its first occurrence of a word within the allowed edits starts.
    near_at: Option<usize>,
}

/// The earlier of two source offsets.
fn earlier(a: Option<usize>, b: usize) -> Option<usize> {
    Some(a.map_or(b, |a| a.min(b)))
}

/// A query word, normalized, with its characters.
struct QueryWord {
    text: String,
    chars: Vec<char>,
    edits: usize,
}

impl QueryWord {
    fn new(text: String) -> QueryWord {
        let chars: Vec<char> = text.chars().collect();
        let edits = edits_allowed(chars.len());
        QueryWord { text, chars, edits }
    }

    /// The nodes whose text holds this word exactly, from the token index, or within its edits,
    /// outside the ID and MID, from the candidates of the vocabulary index.
    fn nodes(&self, snapshot: &Snapshot) -> HashMap<Node, WordMatch> {
        let mut nodes: HashMap<Node, WordMatch> = HashMap::new();
        for posting in snapshot.postings(&self.text) {
            let found = nodes.entry(posting.node).or_default();
            found.exact = true;
            found.exact_at = earlier(found.exact_at, posting.at as usize);
            if posting.zone.ranks_high() {
                found.exact_high = true;
                found.high = true;
            }
        }
        if self.edits > 0 {
            for word in snapshot.spellings(self.edits).candidates(&self.chars) {
                let chars: Vec<char> = word.chars().collect();
                if word == self.text || !within(&self.chars, &chars, self.edits) {
                    continue;
                }
                for posting in snapshot.postings(word) {
                    if posting.zone == Zone::Id {
                        continue;
                    }
                    let found = nodes.entry(posting.node).or_default();
                    found.high |= posting.zone.ranks_high();
                    found.near_at = earlier(found.near_at, posting.at as usize);
                }
            }
        }
        nodes
    }
}

/// A node that matches the whole query.
struct Hit {
    node: Node,
    exact: bool,
    /// Query words matched in the ID, the title or a heading.
    high: usize,
    /// Where the excerpt's word starts.
    at: Option<usize>,
}

/// Search: the items and narrative units whose own text holds every word of `query`, exactly
/// or within the allowed edits, that `filters` match. Nodes that match every word exactly come
/// first; within each group, nodes with more words matched in the ID, title or a heading, then
/// path and source order. A query without words matches every node.
pub fn search(
    documents: &Documents<'_>,
    query: &str,
    filters: &Filters,
    paging: &Paging<'_>,
) -> Result<Page<SearchHit>, ReadError> {
    let snapshot = documents.snapshot;
    let mut texts: Vec<String> = words(query).collect();
    texts.sort_unstable();
    texts.dedup();
    let query: Vec<QueryWord> = texts.iter().cloned().map(QueryWord::new).collect();
    let matcher = Matcher::new(snapshot, filters);

    // Each node with the match of each query word so far.
    let mut found: HashMap<Node, Vec<WordMatch>> = HashMap::new();
    match query.split_first() {
        None => {
            let items = (0..snapshot.items().len()).map(Node::Item);
            let units = (0..snapshot.units().len()).map(Node::Unit);
            let exact = WordMatch {
                exact: true,
                ..WordMatch::default()
            };
            found.extend(items.chain(units).map(|node| (node, vec![exact])));
        }
        Some((first, rest)) => {
            let nodes = first.nodes(snapshot).into_iter();
            found.extend(nodes.map(|(node, m)| (node, vec![m])));
            for word in rest {
                if found.is_empty() {
                    break;
                }
                let nodes = word.nodes(snapshot);
                found.retain(|node, matches| match nodes.get(node) {
                    Some(m) => {
                        matches.push(*m);
                        true
                    }
                    None => false,
                });
            }
        }
    }
    let mut hits: Vec<Hit> = found
        .into_iter()
        .filter(|(node, _)| matcher.matches(*node))
        .map(|(node, matches)| {
            let exact = matches.iter().all(|m| m.exact);
            let high = matches
                .iter()
                .filter(|m| if exact { m.exact_high } else { m.high })
                .count();
            let first = |at: fn(&WordMatch) -> Option<usize>| matches.iter().filter_map(at).min();
            let at = first(|m| m.exact_at).or_else(|| first(|m| m.near_at));
            Hit {
                node,
                exact,
                high,
                at,
            }
        })
        .collect();
    hits.sort_unstable_by_key(|hit| (!hit.exact, Reverse(hit.high), snapshot.position(hit.node)));

    let mut parts = vec![query.len().to_string()];
    parts.extend(texts);
    parts.extend(filters.key_parts());
    let key = result_key(
        snapshot.key().0,
        "search",
        parts.iter().map(String::as_str),
        documents.diagnostics,
    );
    let envelope = Envelope {
        head: None,
        records: "results",
        diagnostics: documents.diagnostics,
        kept: 0,
    };
    page::list(
        key,
        paging,
        hits.len(),
        &envelope,
        |index| {
            let hit = &hits[index];
            let summary = NodeSummary::of(snapshot, Named::Node(hit.node));
            let excerpt = excerpt(snapshot, hit.node, hit.at);
            let json = summary.json(|object| {
                let mut text = String::new();
                let mut member = Object::new(&mut text);
                member.number("line", excerpt.line as u64);
                member.string("text", &excerpt.text);
                member.end();
                object.key("excerpt").push_str(&text);
            });
            (
                SearchHit {
                    node: summary,
                    excerpt,
                },
                json,
            )
        },
        |hit| too_large(&hit.node),
    )
}

/// The excerpt of `node`: the line that holds the word at `at`, a source offset from the
/// token index, or else the node's first line of text that is not its MID anchor.
fn excerpt(snapshot: &Snapshot, node: Node, at: Option<usize>) -> Excerpt {
    let (document, span, first) = match node {
        Node::Item(item) => {
            let parsed = snapshot.item(item);
            (
                snapshot.items()[item].document,
                parsed.span.clone(),
                parsed.line,
            )
        }
        Node::Unit(unit) => {
            let parsed = snapshot.unit(unit);
            (
                snapshot.units()[unit].document,
                parsed.span.clone(),
                parsed.line,
            )
        }
    };
    let contents = &snapshot.documents()[document].file.contents;
    let empty = Excerpt {
        line: first,
        text: String::new(),
    };
    // A document with nodes is UTF-8, and a node's span lies in it; anything else shows nothing.
    let Some(contents) = std::str::from_utf8(contents)
        .ok()
        .filter(|c| c.get(span.clone()).is_some())
    else {
        debug_assert!(false, "the node of {span:?} lies in UTF-8 text");
        return empty;
    };
    // The whole line that holds `at`, which may start before a node that starts at a merged
    // anchor.
    let line_of = |at: usize| {
        let start = contents[..at].rfind(['\n', '\r']).map_or(0, |end| end + 1);
        let end = contents[at..]
            .find(['\n', '\r'])
            .map_or(contents.len(), |end| at + end);
        start..end
    };
    // The token index gives the start of a word of the node; any other offset shows the node's
    // first line.
    let at = at.filter(|&at| {
        let usable = span.contains(&at) && contents.is_char_boundary(at);
        debug_assert!(usable, "a word of {span:?} at {at}");
        usable
    });
    let Some(at) = at else {
        let anchor = |line: &str| line.starts_with("<a id=\"") && line.ends_with("\"></a>");
        let lines = source_lines(&contents[line_of(span.start).start..span.end]);
        let lines: Vec<(usize, &str)> = lines
            .enumerate()
            .map(|(index, line)| (first + index, line.trim()))
            .filter(|(_, line)| !line.is_empty())
            .collect();
        // A node whose text is all spaces, Unicode ones included, shows its first line empty.
        let Some(&(line, text)) = lines
            .iter()
            .find(|(_, line)| !anchor(line))
            .or(lines.first())
        else {
            return empty;
        };
        return Excerpt {
            line,
            text: text.chars().take(EXCERPT_CHARS).collect(),
        };
    };
    let lines = line_of(at);
    let line = first + line_breaks(&contents[span.start..at]);
    let text = &contents[lines.clone()];
    let indent = text.len() - text.trim_start().len();
    let text = text.trim();
    // A word is no space, so it starts at or after the line's indent.
    let before = contents[(lines.start + indent).min(at)..at].chars().count();
    let lead = before.saturating_sub(EXCERPT_LEAD);
    let text = text.chars().skip(lead).take(EXCERPT_CHARS).collect();
    Excerpt { line, text }
}

/// The line breaks in `text`: LF, CRLF or a lone CR (DES-REGIONS).
fn line_breaks(text: &str) -> usize {
    let bytes = text.as_bytes();
    let lone_cr = |at: usize| bytes[at] == b'\r' && bytes.get(at + 1) != Some(&b'\n');
    (0..bytes.len())
        .filter(|&at| bytes[at] == b'\n' || lone_cr(at))
        .count()
}

/// The lines of `text`, ending at LF, CRLF or a lone CR (DES-REGIONS), without their endings.
fn source_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .flat_map(|line| line.strip_suffix('\r').unwrap_or(line).split('\r'))
}

/// The error of a record that cannot fit a page, at the node's location.
pub(crate) fn too_large(node: &NodeSummary) -> ReadError {
    let (path, line) = node.location();
    let message =
        format!("what the result says of {path}:{line} cannot fit a page of {PAGE_BYTES} bytes");
    ReadError::TooLarge(Box::new(Diagnostic::error(TOO_LARGE, path, line, message)))
}

/// `item list`: a summary of each item that `filters` match, in path and source order.
pub fn item_list(
    documents: &Documents<'_>,
    filters: &Filters,
    paging: &Paging<'_>,
) -> Result<Page<ItemSummary>, ReadError> {
    let snapshot = documents.snapshot;
    let matcher = Matcher::new(snapshot, filters);
    let items: Vec<usize> = (0..snapshot.items().len())
        .filter(|&item| matcher.item(item))
        .collect();
    let parts = filters.key_parts();
    let key = result_key(
        snapshot.key().0,
        "item list",
        parts.iter().map(String::as_str),
        documents.diagnostics,
    );
    let envelope = Envelope {
        head: None,
        records: "items",
        diagnostics: documents.diagnostics,
        kept: 0,
    };
    page::list(
        key,
        paging,
        items.len(),
        &envelope,
        |index| {
            let summary = ItemSummary::of(snapshot, items[index]);
            let json = summary.json();
            (summary, json)
        },
        |summary| too_large(&NodeSummary::Item(summary.clone())),
    )
}
