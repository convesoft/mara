//! `relation list`: the connections one hop from a node (REQ-DIRECT-NEIGHBOURS in
//! `docs/retrieval.mara.md`).

use crate::diagnostic::Diagnostic;
use crate::document::{Link, Node, label_name};
use crate::json::Object;
use crate::paging::PAGE_BYTES;
use crate::snapshot::{Edge, EdgeSource, Snapshot};

use super::node::{Named, NodeSummary, resolve};
use super::page::{self, Envelope, Head};
use super::{Documents, Page, Paging, ReadError, TOO_LARGE, result_key};

/// A connection of the node that `relation list` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    pub kind: ConnectionKind,
    /// The relation's forward name for an outgoing relation, its inverse name for an incoming
    /// one; `None` for a mention.
    pub relation: Option<String>,
    pub direction: Direction,
    pub neighbour: Neighbour,
    /// Where the link is written.
    pub source: Location,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionKind {
    Relation,
    Mention,
}

impl ConnectionKind {
    pub fn name(self) -> &'static str {
        match self {
            ConnectionKind::Relation => "relation",
            ConnectionKind::Mention => "mention",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Outgoing,
    Incoming,
}

impl Direction {
    pub fn name(self) -> &'static str {
        match self {
            Direction::Outgoing => "outgoing",
            Direction::Incoming => "incoming",
        }
    }
}

/// The other end of a connection: a node, or a MID that no item has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Neighbour {
    Node(NodeSummary),
    Unresolved { mid: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: String,
    pub line: usize,
}

/// One page of `relation list`, with the node it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationPage {
    pub node: NodeSummary,
    pub page: Page<Connection>,
}

/// `relation list`: the connections one hop from the item or narrative unit that `address`
/// names. For an item, its outgoing relations in row order and its mentions in source order,
/// then the relations and mentions that target it in path and source order, a relation under
/// its inverse name. For a unit, its mentions. For a whole document, every relation and mention
/// written in it, in source order. A link to a MID that several items have connects to each of
/// them. With `names`, only the relations of those forward or inverse names.
pub fn relation_list(
    documents: &Documents<'_>,
    address: &str,
    names: &[String],
    paging: &Paging<'_>,
) -> Result<RelationPage, ReadError> {
    let snapshot = documents.snapshot;
    let named = resolve(snapshot, address)?;
    let mut connections = Vec::new();
    match named {
        Named::Document(document) => {
            let entry = &snapshot.documents()[document];
            let items = entry.items.clone().flat_map(|item| snapshot.outgoing(item));
            let mut edges: Vec<&Edge> = items.chain(snapshot.narrative(document)).collect();
            edges.sort_by_key(|edge| snapshot.link(edge).span.start);
            for edge in edges {
                outgoing(snapshot, edge, &mut connections);
            }
        }
        Named::Node(Node::Item(item)) => {
            for edge in snapshot.outgoing(item) {
                outgoing(snapshot, edge, &mut connections);
            }
            let mut incoming: Vec<&Edge> = snapshot.incoming(&snapshot.item(item).mid).collect();
            incoming.sort_by_key(|edge| (edge.document, snapshot.link(edge).span.start));
            for edge in incoming {
                connections.push(self::incoming(snapshot, edge));
            }
        }
        Named::Node(Node::Unit(unit)) => {
            let entry = snapshot.units()[unit];
            let document = &snapshot.documents()[entry.document];
            let parsed = snapshot.unit(unit);
            for link in &document.file.document.mentions[parsed.mentions.clone()] {
                targets(snapshot, link, entry.document, None, &mut connections);
            }
        }
    }
    let mut wanted: Vec<String> = names.iter().map(|name| label_name(name)).collect();
    wanted.sort_unstable();
    wanted.dedup();
    if !wanted.is_empty() {
        connections.retain(|connection| {
            connection
                .relation
                .as_ref()
                .is_some_and(|name| wanted.binary_search(name).is_ok())
        });
    }

    let summary = NodeSummary::of(snapshot, named);
    let mut parts = vec![address.to_owned()];
    parts.extend(wanted);
    let key = result_key(
        snapshot.key().0,
        "relation list",
        parts.iter().map(String::as_str),
        documents.diagnostics,
    );
    let (path, line) = summary.location();
    let envelope = Envelope {
        head: Some(Head {
            name: "node",
            json: summary.json(|_| {}),
            path: path.to_owned(),
            line,
        }),
        records: "connections",
        diagnostics: documents.diagnostics,
        kept: 0,
    };
    let page = page::list(
        key,
        paging,
        connections.len(),
        &envelope,
        |index| {
            let connection = connections[index].clone();
            let json = connection.json();
            (connection, json)
        },
        |connection| {
            let Location { path, line } = &connection.source;
            let message = format!(
                "the connection written at {path}:{line} cannot fit a page of {PAGE_BYTES} bytes"
            );
            ReadError::TooLarge(Box::new(Diagnostic::error(
                TOO_LARGE,
                path.clone(),
                *line,
                message,
            )))
        },
    )?;
    Ok(RelationPage {
        node: summary,
        page,
    })
}

impl Connection {
    fn json(&self) -> String {
        let neighbour = match &self.neighbour {
            Neighbour::Node(node) => node.json(|_| {}),
            Neighbour::Unresolved { mid } => {
                let mut out = String::new();
                let mut object = Object::new(&mut out);
                object.string("kind", "unresolved");
                object.string("mid", mid);
                object.end();
                out
            }
        };
        let mut source = String::new();
        let mut object = Object::new(&mut source);
        object.string("path", &self.source.path);
        object.number("line", self.source.line as u64);
        object.end();
        let mut out = String::new();
        let mut object = Object::new(&mut out);
        object.string("kind", self.kind.name());
        object.optional_string("relation", self.relation.as_deref());
        object.string("direction", self.direction.name());
        object.key("neighbour").push_str(&neighbour);
        object.key("source").push_str(&source);
        object.end();
        out
    }
}

/// The connections of `edge`, which leaves the node being read.
fn outgoing(snapshot: &Snapshot, edge: &Edge, connections: &mut Vec<Connection>) {
    let relation = snapshot.relation(edge).map(str::to_owned);
    targets(
        snapshot,
        snapshot.link(edge),
        edge.document,
        relation,
        connections,
    );
}

/// A connection to each item that `link`, in the document at `document`, targets, or to its MID
/// when no item has it.
fn targets(
    snapshot: &Snapshot,
    link: &Link,
    document: usize,
    relation: Option<String>,
    connections: &mut Vec<Connection>,
) {
    let mid = link.fragment.as_deref().unwrap_or_default();
    let source = Location {
        path: snapshot.documents()[document].path.clone(),
        line: link.line,
    };
    let kind = match relation {
        Some(_) => ConnectionKind::Relation,
        None => ConnectionKind::Mention,
    };
    let items = snapshot.items_with_mid(mid);
    let neighbours: Vec<Neighbour> = match items {
        [] => vec![Neighbour::Unresolved {
            mid: mid.to_owned(),
        }],
        items => items
            .iter()
            .map(|&item| Neighbour::Node(NodeSummary::of(snapshot, Named::Node(Node::Item(item)))))
            .collect(),
    };
    for neighbour in neighbours {
        connections.push(Connection {
            kind,
            relation: relation.clone(),
            direction: Direction::Outgoing,
            neighbour,
            source: source.clone(),
        });
    }
}

/// The connection of `edge`, which targets the node being read, from its item or unit.
fn incoming(snapshot: &Snapshot, edge: &Edge) -> Connection {
    let from = match (edge.item, edge.source) {
        (Some(item), _) => Some(Node::Item(item)),
        (None, EdgeSource::Narrative(_)) => snapshot.narrative_unit(edge).map(Node::Unit),
        (None, _) => None,
    };
    let neighbour = match from {
        Some(node) => Neighbour::Node(NodeSummary::of(snapshot, Named::Node(node))),
        None => Neighbour::Node(NodeSummary::of(snapshot, Named::Document(edge.document))),
    };
    let relation = snapshot.relation(edge).map(|forward| {
        let inverse = snapshot
            .schema()
            .and_then(|schema| schema.relations.get(forward))
            .and_then(|relation| relation.inverse.as_deref());
        inverse.unwrap_or(forward).to_owned()
    });
    Connection {
        kind: match relation {
            Some(_) => ConnectionKind::Relation,
            None => ConnectionKind::Mention,
        },
        relation,
        direction: Direction::Incoming,
        neighbour,
        source: Location {
            path: snapshot.documents()[edge.document].path.clone(),
            line: snapshot.link(edge).line,
        },
    }
}
