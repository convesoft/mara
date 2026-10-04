//! Disposable structure and direct adjacency. Petgraph indexes never leave this module.

use std::collections::BTreeMap;

mod references;
mod summary;

pub use summary::{DiscoveryContext, DiscoveryKind, DiscoveryNodeSummary};

use petgraph::{
    Direction,
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};

use crate::{
    Corpus, Document, Item, MarkdownBlock, MarkdownBlockKind, RelationDirection, SourceLocation,
};

/// Owned topology reusable across immutable corpus clones; no filesystem/cache policy.
#[derive(Debug)]
pub struct DiscoveryProjection {
    graph: DiGraph<OwnedNode, OwnedEdge>,
    diagnostics: Vec<crate::Diagnostic>,
    references: BTreeMap<String, Vec<NodeIndex>>,
}
#[derive(Debug)]
struct OwnedNode {
    kind: OwnedKind,
    source: SourceLocation,
    reference: String,
}
#[derive(Debug)]
enum OwnedKind {
    Document(usize),
    Item(usize, usize),
    Section {
        document: usize,
        item: Option<usize>,
        path: Vec<usize>,
    },
    Block {
        document: usize,
        item: Option<usize>,
        path: Vec<usize>,
    },
}
fn block_keys(
    blocks: &[MarkdownBlock],
    document: usize,
    item: Option<usize>,
    prefix: &mut Vec<usize>,
    result: &mut BTreeMap<usize, (usize, Option<usize>, Vec<usize>)>,
) {
    for (index, block) in blocks.iter().enumerate() {
        prefix.push(index);
        result.insert(block as *const _ as usize, (document, item, prefix.clone()));
        block_keys(block.children(), document, item, prefix, result);
        prefix.pop();
    }
}
#[derive(Debug)]
struct OwnedEdge {
    kind: OwnedEdgeKind,
    source: SourceLocation,
    symmetric: bool,
    occurrence_count: usize,
}
#[derive(Debug)]
enum OwnedEdgeKind {
    Contains,
    Mentions,
    Schema(String),
}

#[derive(Debug)]
pub struct DiscoveryGraph<'corpus> {
    projection: std::sync::Arc<DiscoveryProjection>,
    corpus: &'corpus Corpus,
}
impl std::ops::Deref for DiscoveryGraph<'_> {
    type Target = DiscoveryProjection;
    fn deref(&self) -> &Self::Target {
        &self.projection
    }
}
impl<'corpus> DiscoveryGraph<'corpus> {
    pub(crate) fn from_projection(
        corpus: &'corpus Corpus,
        projection: std::sync::Arc<DiscoveryProjection>,
    ) -> Self {
        Self { projection, corpus }
    }
    pub(crate) fn project(corpus: &Corpus) -> DiscoveryProjection {
        let builder = DiscoveryBuilder::new(corpus);
        let mut documents = BTreeMap::new();
        let mut items = BTreeMap::new();
        let mut blocks = BTreeMap::new();
        for (d, document) in corpus.documents().iter().enumerate() {
            documents.insert(document as *const _ as usize, d);
            block_keys(document.blocks(), d, None, &mut Vec::new(), &mut blocks);
            for (i, item) in document.items().iter().enumerate() {
                items.insert(item as *const _ as usize, (d, i));
                block_keys(item.body_blocks(), d, Some(i), &mut Vec::new(), &mut blocks);
            }
        }
        DiscoveryProjection {
            graph: builder.graph.map(
                |_, n| OwnedNode {
                    kind: match n.kind {
                        DiscoveryNodeKind::Document(d) => {
                            OwnedKind::Document(documents[&(d as *const _ as usize)])
                        }
                        DiscoveryNodeKind::Item(i) => {
                            let (d, i) = items[&(i as *const _ as usize)];
                            OwnedKind::Item(d, i)
                        }
                        DiscoveryNodeKind::Section { heading } => {
                            let (document, item, path) =
                                blocks[&(heading as *const _ as usize)].clone();
                            OwnedKind::Section {
                                document,
                                item,
                                path,
                            }
                        }
                        DiscoveryNodeKind::MarkdownBlock(b) => {
                            let (document, item, path) = blocks[&(b as *const _ as usize)].clone();
                            OwnedKind::Block {
                                document,
                                item,
                                path,
                            }
                        }
                    },
                    source: n.source.clone(),
                    reference: n.reference.clone(),
                },
                |_, e| OwnedEdge {
                    kind: match e.kind {
                        EdgeKind::Contains => OwnedEdgeKind::Contains,
                        EdgeKind::Mentions => OwnedEdgeKind::Mentions,
                        EdgeKind::Schema(n) => OwnedEdgeKind::Schema(n.to_owned()),
                    },
                    source: e.source.clone(),
                    symmetric: e.symmetric,
                    occurrence_count: e.occurrence_count,
                },
            ),
            diagnostics: builder.diagnostics,
            references: builder.references,
        }
    }
    pub fn diagnostics(&self) -> &[crate::Diagnostic] {
        &self.diagnostics
    }
    pub fn nodes(&self) -> impl Iterator<Item = DiscoveryNode<'_, 'corpus>> {
        self.graph
            .node_indices()
            .map(|index| DiscoveryNode { graph: self, index })
    }
}

#[derive(Debug)]
struct DiscoveryBuilder<'corpus> {
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

impl<'corpus> DiscoveryBuilder<'corpus> {
    fn new(corpus: &'corpus Corpus) -> Self {
        // @mara implements DES-DOCUMENT-STRUCTURE
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
        let documents = self.graph.corpus.documents();
        let block = |d: usize, i: Option<usize>, path: &[usize]| {
            let mut blocks = i.map_or_else(
                || documents[d].blocks(),
                |i| documents[d].items()[i].body_blocks(),
            );
            let mut selected = None;
            for &index in path {
                let b = &blocks[index];
                selected = Some(b);
                blocks = b.children();
            }
            selected.expect("projection block path")
        };
        match &self.graph.graph[self.index].kind {
            OwnedKind::Document(d) => DiscoveryNodeKind::Document(&documents[*d]),
            OwnedKind::Item(d, i) => DiscoveryNodeKind::Item(&documents[*d].items()[*i]),
            OwnedKind::Section {
                document,
                item,
                path,
            } => DiscoveryNodeKind::Section {
                heading: block(*document, *item, path),
            },
            OwnedKind::Block {
                document,
                item,
                path,
            } => DiscoveryNodeKind::MarkdownBlock(block(*document, *item, path)),
        }
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
            let kind = match (&data.kind, direction) {
                (OwnedEdgeKind::Contains, RelationDirection::Outgoing) => ConnectionKind::Contains,
                (OwnedEdgeKind::Contains, _) => ConnectionKind::ContainedBy,
                (OwnedEdgeKind::Mentions, _) => ConnectionKind::Mentions,
                (OwnedEdgeKind::Schema(name), _) => ConnectionKind::Schema(name),
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
