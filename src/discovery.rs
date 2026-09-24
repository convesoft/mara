//! Disposable structure and direct adjacency. Petgraph indexes never leave this module.

use std::collections::BTreeMap;

mod references;
mod summary;

pub(crate) use references::heading_anchor;

pub use summary::{DiscoveryContext, DiscoveryKind, DiscoveryNodeSummary};

use petgraph::{
    Direction,
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};

use crate::{
    Corpus, Document, Item, MarkdownBlock, MarkdownBlockKind, RelationDirection, SourceLocation,
};

#[derive(Debug)]
pub struct DiscoveryGraph<'corpus> {
    graph: DiGraph<NodeData<'corpus>, EdgeData<'corpus>>,
    diagnostics: Vec<crate::Diagnostic>,
    references: BTreeMap<String, Vec<NodeIndex>>,
}

#[derive(Debug)]
struct NodeData<'corpus> {
    kind: DiscoveryNodeKind<'corpus>,
    source: SourceLocation,
    reference: String,
}

/// Source-backed node data. A section retains its original Markdown heading;
/// the node's source location covers the whole derived section.
#[derive(Debug, Clone, Copy)]
pub enum DiscoveryNodeKind<'corpus> {
    Document(&'corpus Document),
    Item(&'corpus Item),
    Section { heading: &'corpus MarkdownBlock },
    MarkdownBlock(&'corpus MarkdownBlock),
}

#[derive(Debug)]
struct EdgeData<'corpus> {
    kind: EdgeKind<'corpus>,
    source: SourceLocation,
    symmetric: bool,
    occurrence_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum EdgeKind<'corpus> {
    Contains,
    Mentions,
    Schema(&'corpus str),
}

/// Built-in connections cannot collide with a schema's authored relation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind<'graph> {
    Contains,
    ContainedBy,
    Mentions,
    Schema(&'graph str),
}

/// A borrowed node in one immutable graph, not a persisted discovery handle.
#[derive(Clone, Copy)]
pub struct DiscoveryNode<'graph, 'corpus> {
    graph: &'graph DiscoveryGraph<'corpus>,
    index: NodeIndex,
}

pub struct DiscoveryConnection<'graph, 'corpus> {
    pub kind: ConnectionKind<'graph>,
    pub direction: RelationDirection,
    pub neighbour: DiscoveryNode<'graph, 'corpus>,
    pub source: &'graph SourceLocation,
    pub occurrence_count: usize,
}

impl<'corpus> DiscoveryGraph<'corpus> {
    pub(crate) fn new(corpus: &'corpus Corpus) -> Self {
        let mut result = Self {
            graph: DiGraph::new(),
            diagnostics: Vec::new(),
            references: BTreeMap::new(),
        };
        for document in corpus.documents() {
            let lines = std::iter::once(0)
                .chain(
                    document
                        .source()
                        .match_indices('\n')
                        .map(|(offset, _)| offset + 1)
                        .filter(|&offset| offset < document.source().len()),
                )
                .collect::<Vec<_>>();
            let source =
                crate::corpus::location(document.path(), &lines, 0, document.source().len());
            let root = result.graph.add_node(NodeData {
                kind: DiscoveryNodeKind::Document(document),
                source,
                reference: String::new(),
            });
            let mut content = document
                .blocks()
                .iter()
                .map(Content::Block)
                .chain(document.items().iter().map(Content::Item))
                .collect::<Vec<_>>();
            content.sort_by_key(|entry| entry.source().span().start_byte());
            result.add_scope(root, content, document.source().len(), &lines);
        }
        result.add_item_connections();
        result.add_references(corpus);
        result.index_references(corpus);
        result
    }

    /// Broken internal destinations and ambiguous anchor declarations.
    pub fn diagnostics(&self) -> &[crate::Diagnostic] {
        &self.diagnostics
    }

    /// Document path order, then structural preorder within each document.
    pub fn nodes(&self) -> impl Iterator<Item = DiscoveryNode<'_, 'corpus>> {
        self.graph
            .node_indices()
            .map(|index| DiscoveryNode { graph: self, index })
    }

    fn add_child(
        &mut self,
        parent: NodeIndex,
        kind: DiscoveryNodeKind<'corpus>,
        source: SourceLocation,
    ) -> NodeIndex {
        let child = self.graph.add_node(NodeData {
            kind,
            source: source.clone(),
            reference: String::new(),
        });
        self.graph.add_edge(
            parent,
            child,
            EdgeData {
                kind: EdgeKind::Contains,
                source,
                symmetric: false,
                occurrence_count: 0,
            },
        );
        child
    }

    fn add_scope(
        &mut self,
        parent: NodeIndex,
        content: Vec<Content<'corpus>>,
        end: usize,
        lines: &[usize],
    ) {
        let mut sections: Vec<(u8, NodeIndex)> = Vec::new();
        for entry in content {
            if let Content::Block(heading) = entry
                && let MarkdownBlockKind::Heading { level } = heading.kind()
            {
                while sections
                    .last()
                    .is_some_and(|&(open_level, _)| open_level >= level)
                {
                    let (_, section) = sections.pop().unwrap();
                    self.end_section(section, heading.source().span().start_byte(), lines);
                }
                let owner = sections.last().map_or(parent, |&(_, node)| node);
                let section = self.add_child(
                    owner,
                    DiscoveryNodeKind::Section { heading },
                    heading.source().clone(),
                );
                sections.push((level, section));
                continue;
            }
            let owner = sections.last().map_or(parent, |&(_, node)| node);
            match entry {
                Content::Item(item) => {
                    let node =
                        self.add_child(owner, DiscoveryNodeKind::Item(item), item.source().clone());
                    self.add_scope(
                        node,
                        item.body_blocks().iter().map(Content::Block).collect(),
                        item.body_source().span().end_byte(),
                        lines,
                    );
                }
                Content::Block(block) => {
                    let node = self.add_child(
                        owner,
                        DiscoveryNodeKind::MarkdownBlock(block),
                        block.source().clone(),
                    );
                    self.add_scope(
                        node,
                        block.children().iter().map(Content::Block).collect(),
                        block.source().span().end_byte(),
                        lines,
                    );
                }
            }
        }
        for (_, section) in sections {
            self.end_section(section, end, lines);
        }
    }

    fn end_section(&mut self, section: NodeIndex, end: usize, lines: &[usize]) {
        let source = &self.graph[section].source;
        let source = crate::corpus::location(source.path(), lines, source.span().start_byte(), end);
        self.graph[section].source = source.clone();
        // The same containment provenance is returned in either direction.
        let edge = self
            .graph
            .edges_directed(section, Direction::Incoming)
            .find(|edge| matches!(edge.weight().kind, EdgeKind::Contains))
            .unwrap()
            .id();
        self.graph[edge].source = source;
    }

    fn add_item_connections(&mut self) {
        let mut targets: BTreeMap<&str, Vec<NodeIndex>> = BTreeMap::new();
        let items = self
            .graph
            .node_indices()
            .filter_map(|index| match self.graph[index].kind {
                DiscoveryNodeKind::Item(item) => Some((index, item)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for &(node, item) in &items {
            targets.entry(item.id()).or_default().push(node);
            if let Some(mid) = item.mid() {
                targets.entry(mid).or_default().push(node);
            }
        }
        let mut canonical: BTreeMap<(usize, usize, &str), petgraph::graph::EdgeIndex> =
            BTreeMap::new();
        for (node, item) in items {
            for relation in item.relations() {
                if let Some([target]) = targets.get(relation.target()).map(Vec::as_slice) {
                    let (mut from, mut to) = if relation.inverse {
                        (*target, node)
                    } else {
                        (node, *target)
                    };
                    if relation.symmetric && from.index() > to.index() {
                        std::mem::swap(&mut from, &mut to);
                    }
                    let key = (from.index(), to.index(), relation.canonical.as_str());
                    if let Some(&edge) = canonical.get(&key) {
                        self.graph[edge].occurrence_count += 1;
                    } else {
                        let edge = self.graph.add_edge(
                            from,
                            to,
                            EdgeData {
                                kind: EdgeKind::Schema(&relation.canonical),
                                source: relation.source().clone(),
                                symmetric: relation.symmetric,
                                occurrence_count: 1,
                            },
                        );
                        canonical.insert(key, edge);
                    }
                }
            }
        }
    }
}

impl<'graph, 'corpus> DiscoveryNode<'graph, 'corpus> {
    pub fn kind(self) -> DiscoveryNodeKind<'corpus> {
        self.graph.graph[self.index].kind
    }

    pub fn source(self) -> &'graph SourceLocation {
        &self.graph.graph[self.index].source
    }

    /// Only immediate connections. Incoming containment is the reverse view
    /// of the stored parent-to-child edge, never a separately authored edge.
    pub fn connections(
        self,
        direction: RelationDirection,
    ) -> Vec<DiscoveryConnection<'graph, 'corpus>> {
        let edges = self
            .graph
            .graph
            .edges_directed(self.index, Direction::Outgoing)
            .chain(
                self.graph
                    .graph
                    .edges_directed(self.index, Direction::Incoming),
            );
        let mut seen = std::collections::HashSet::new();
        let mut connections = Vec::new();
        for edge in edges {
            if !seen.insert(edge.id()) {
                continue;
            }
            let data = edge.weight();
            let index = if data.symmetric {
                if direction != RelationDirection::Symmetric {
                    continue;
                }
                if edge.source() == self.index {
                    edge.target()
                } else {
                    edge.source()
                }
            } else {
                match direction {
                    RelationDirection::Outgoing if edge.source() == self.index => edge.target(),
                    RelationDirection::Incoming if edge.target() == self.index => edge.source(),
                    _ => continue,
                }
            };
            let kind = match (data.kind, direction) {
                (EdgeKind::Contains, RelationDirection::Outgoing) => ConnectionKind::Contains,
                (EdgeKind::Contains, _) => ConnectionKind::ContainedBy,
                (EdgeKind::Mentions, _) => ConnectionKind::Mentions,
                (EdgeKind::Schema(name), _) => ConnectionKind::Schema(name),
            };
            connections.push(DiscoveryConnection {
                kind,
                direction,
                neighbour: DiscoveryNode {
                    graph: self.graph,
                    index,
                },
                source: &data.source,
                occurrence_count: data.occurrence_count,
            });
        }
        connections.sort_by(|a, b| {
            a.neighbour
                .index
                .index()
                .cmp(&b.neighbour.index.index())
                .then_with(|| match (a.kind, b.kind) {
                    (ConnectionKind::Schema(a), ConnectionKind::Schema(b)) => a.cmp(b),
                    (ConnectionKind::Schema(_), _) => std::cmp::Ordering::Less,
                    (_, ConnectionKind::Schema(_)) => std::cmp::Ordering::Greater,
                    _ => a
                        .source
                        .span()
                        .start_byte()
                        .cmp(&b.source.span().start_byte()),
                })
        });
        connections
    }

    pub fn parent(self) -> Option<Self> {
        self.connections(RelationDirection::Incoming)
            .into_iter()
            .find(|connection| connection.kind == ConnectionKind::ContainedBy)
            .map(|connection| connection.neighbour)
    }

    pub fn children(self) -> Vec<Self> {
        self.connections(RelationDirection::Outgoing)
            .into_iter()
            .filter(|connection| connection.kind == ConnectionKind::Contains)
            .map(|connection| connection.neighbour)
            .collect()
    }
}

#[derive(Clone, Copy)]
enum Content<'corpus> {
    Item(&'corpus Item),
    Block(&'corpus MarkdownBlock),
}

impl<'corpus> Content<'corpus> {
    fn source(self) -> &'corpus SourceLocation {
        match self {
            Self::Item(item) => item.source(),
            Self::Block(block) => block.source(),
        }
    }
}
