use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
};

use petgraph::{
    Direction,
    graph::{Graph, NodeIndex},
    visit::EdgeRef,
};

use crate::{CanonicalRelationEdge, Mid, SourceSpan};

/// Stable Mara identity for one query-graph endpoint.
#[derive(Debug, Clone)]
pub enum NodeRef {
    Item {
        mid: Mid,
    },
    SourceSpan {
        source: SourceSpan,
        symbol: Option<String>,
    },
    External {
        uri: String,
    },
}

impl NodeRef {
    pub fn item(mid: Mid) -> Self {
        Self::Item { mid }
    }

    pub fn source_span(source: SourceSpan, symbol: Option<String>) -> Self {
        Self::SourceSpan { source, symbol }
    }

    pub fn external(uri: impl Into<String>) -> Self {
        Self::External { uri: uri.into() }
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Item { .. } => "item",
            Self::SourceSpan { .. } => "source_span",
            Self::External { .. } => "external",
        }
    }

    pub const fn mid(&self) -> Option<&Mid> {
        match self {
            Self::Item { mid } => Some(mid),
            Self::SourceSpan { .. } | Self::External { .. } => None,
        }
    }

    pub const fn source(&self) -> Option<&SourceSpan> {
        match self {
            Self::SourceSpan { source, .. } => Some(source),
            Self::Item { .. } | Self::External { .. } => None,
        }
    }

    pub fn symbol(&self) -> Option<&str> {
        match self {
            Self::SourceSpan { symbol, .. } => symbol.as_deref(),
            Self::Item { .. } | Self::External { .. } => None,
        }
    }

    pub fn uri(&self) -> Option<&str> {
        match self {
            Self::External { uri } => Some(uri),
            Self::Item { .. } | Self::SourceSpan { .. } => None,
        }
    }
}

impl From<Mid> for NodeRef {
    fn from(mid: Mid) -> Self {
        Self::item(mid)
    }
}

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for NodeRef {}

impl PartialOrd for NodeRef {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeRef {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind()
            .as_bytes()
            .cmp(other.kind().as_bytes())
            .then_with(|| match (self, other) {
                (Self::Item { mid: left }, Self::Item { mid: right }) => left.cmp(right),
                (
                    Self::SourceSpan {
                        source: left_source,
                        symbol: left_symbol,
                    },
                    Self::SourceSpan {
                        source: right_source,
                        symbol: right_symbol,
                    },
                ) => compare_source_identities(left_source, right_source)
                    .then_with(|| left_symbol.cmp(right_symbol)),
                (Self::External { uri: left }, Self::External { uri: right }) => {
                    left.as_bytes().cmp(right.as_bytes())
                }
                _ => Ordering::Equal,
            })
    }
}

impl Hash for NodeRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind().hash(state);
        match self {
            Self::Item { mid } => mid.hash(state),
            Self::SourceSpan { source, symbol } => {
                source.path().hash(state);
                source.start_byte().hash(state);
                source.end_byte().hash(state);
                symbol.hash(state);
            }
            Self::External { uri } => uri.hash(state),
        }
    }
}

/// One canonical typed relation admitted to the private graph projection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectionEdge {
    relation: String,
    source: NodeRef,
    target: NodeRef,
}

impl ProjectionEdge {
    pub fn new(relation: impl Into<String>, source: NodeRef, target: NodeRef) -> Self {
        Self {
            relation: relation.into(),
            source,
            target,
        }
    }

    pub fn from_canonical(edge: &CanonicalRelationEdge) -> Self {
        Self::new(
            edge.relation(),
            NodeRef::item(edge.source().clone()),
            NodeRef::item(edge.target().clone()),
        )
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn source(&self) -> &NodeRef {
        &self.source
    }

    pub const fn target(&self) -> &NodeRef {
        &self.target
    }
}

impl From<&CanonicalRelationEdge> for ProjectionEdge {
    fn from(edge: &CanonicalRelationEdge) -> Self {
        Self::from_canonical(edge)
    }
}

/// Edge directions admitted by one trace request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TraceDirection {
    Incoming,
    Outgoing,
    Bidirectional,
}

impl TraceDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Bidirectional => "bidirectional",
        }
    }
}

/// Actual direction followed across one canonical edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TraversalDirection {
    Incoming,
    Outgoing,
}

impl TraversalDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
        }
    }
}

/// One relation-bearing step in a returned trace path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraceStep {
    relation: String,
    traversal: TraversalDirection,
    source: NodeRef,
    target: NodeRef,
}

impl TraceStep {
    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub const fn traversal(&self) -> TraversalDirection {
        self.traversal
    }

    pub const fn source(&self) -> &NodeRef {
        &self.source
    }

    pub const fn target(&self) -> &NodeRef {
        &self.target
    }
}

/// One distinct non-empty simple edge path from the trace focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePath {
    nodes: Vec<NodeRef>,
    edges: Vec<TraceStep>,
}

impl TracePath {
    pub fn nodes(&self) -> &[NodeRef] {
        &self.nodes
    }

    pub fn edges(&self) -> &[TraceStep] {
        &self.edges
    }

    pub fn steps(&self) -> &[TraceStep] {
        &self.edges
    }
}

/// Deterministic result of one bounded trace request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceResult {
    focus: NodeRef,
    direction: TraceDirection,
    max_depth: usize,
    nodes: Vec<NodeRef>,
    paths: Vec<TracePath>,
}

impl TraceResult {
    pub const fn focus(&self) -> &NodeRef {
        &self.focus
    }

    pub const fn direction(&self) -> TraceDirection {
        self.direction
    }

    pub const fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub fn nodes(&self) -> &[NodeRef] {
        &self.nodes
    }

    pub fn paths(&self) -> &[TracePath] {
        &self.paths
    }
}

/// Mara-owned request failures for pure trace traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    MissingFocus { focus: NodeRef },
    ZeroDepth,
}

impl TraceError {
    pub const fn focus(&self) -> Option<&NodeRef> {
        match self {
            Self::MissingFocus { focus } => Some(focus),
            Self::ZeroDepth => None,
        }
    }
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFocus { .. } => {
                formatter.write_str("trace focus is not in the projection")
            }
            Self::ZeroDepth => formatter.write_str("trace depth must be positive"),
        }
    }
}

impl Error for TraceError {}

/// Rebuildable query projection whose petgraph state never crosses the public API.
#[derive(Debug, Clone)]
pub struct QueryGraph {
    graph: Graph<NodeRef, ProjectionEdge>,
    node_indexes: BTreeMap<NodeRef, NodeIndex>,
    nodes: Vec<NodeRef>,
    edges: Vec<ProjectionEdge>,
}

impl QueryGraph {
    /// Rebuilds a projection from already-normalized Mara node and edge values.
    pub fn build(
        nodes: impl IntoIterator<Item = NodeRef>,
        edges: impl IntoIterator<Item = ProjectionEdge>,
    ) -> Self {
        let mut edges = edges.into_iter().collect::<Vec<_>>();
        let mut representatives = BTreeMap::<NodeRef, NodeRef>::new();
        for node in nodes.into_iter().chain(
            edges
                .iter()
                .flat_map(|edge| [edge.source.clone(), edge.target.clone()]),
        ) {
            match representatives.get_mut(&node) {
                Some(current)
                    if canonical_node_json(&node).as_slice()
                        < canonical_node_json(current).as_slice() =>
                {
                    *current = node;
                }
                Some(_) => {}
                None => {
                    representatives.insert(node.clone(), node);
                }
            }
        }

        for edge in &mut edges {
            edge.source = representatives
                .get(&edge.source)
                .expect("every edge source was inserted")
                .clone();
            edge.target = representatives
                .get(&edge.target)
                .expect("every edge target was inserted")
                .clone();
        }

        let mut deduplicated = BTreeMap::new();
        for edge in edges {
            deduplicated
                .entry((
                    edge.source.clone(),
                    edge.relation.clone(),
                    edge.target.clone(),
                ))
                .or_insert(edge);
        }
        let mut edges = deduplicated.into_values().collect::<Vec<_>>();
        edges.sort_by(compare_projection_edges);

        let nodes = representatives.into_values().collect::<Vec<_>>();
        let mut graph = Graph::new();
        let mut node_indexes = BTreeMap::new();
        for node in &nodes {
            let index = graph.add_node(node.clone());
            node_indexes.insert(node.clone(), index);
        }
        for edge in &edges {
            let source = node_indexes[edge.source()];
            let target = node_indexes[edge.target()];
            graph.add_edge(source, target, edge.clone());
        }

        Self {
            graph,
            node_indexes,
            nodes,
            edges,
        }
    }

    /// Adapts canonical item-to-item relation data and normalized fixture seams.
    pub fn from_canonical<'a>(
        items: impl IntoIterator<Item = &'a Mid>,
        canonical_edges: impl IntoIterator<Item = &'a CanonicalRelationEdge>,
        normalized_edges: impl IntoIterator<Item = ProjectionEdge>,
    ) -> Self {
        let nodes = items.into_iter().cloned().map(NodeRef::item);
        let edges = canonical_edges
            .into_iter()
            .map(ProjectionEdge::from_canonical)
            .chain(normalized_edges);
        Self::build(nodes, edges)
    }

    pub fn nodes(&self) -> &[NodeRef] {
        &self.nodes
    }

    pub fn edges(&self) -> &[ProjectionEdge] {
        &self.edges
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn contains(&self, node: &NodeRef) -> bool {
        self.node_indexes.contains_key(node)
    }

    /// Returns every selected simple edge path with one through `max_depth` steps.
    pub fn trace(
        &self,
        focus: &NodeRef,
        direction: TraceDirection,
        relation: Option<&str>,
        max_depth: usize,
    ) -> Result<TraceResult, TraceError> {
        if max_depth == 0 {
            return Err(TraceError::ZeroDepth);
        }
        let Some(&focus_index) = self.node_indexes.get(focus) else {
            return Err(TraceError::MissingFocus {
                focus: focus.clone(),
            });
        };
        let focus = self.graph[focus_index].clone();
        let mut visited = BTreeSet::from([focus.clone()]);
        let mut path_nodes = vec![focus.clone()];
        let mut path_edges = Vec::new();
        let mut paths = Vec::new();
        self.collect_paths(
            focus_index,
            direction,
            relation,
            max_depth,
            &mut visited,
            &mut path_nodes,
            &mut path_edges,
            &mut paths,
        );
        paths.sort_by(compare_trace_paths);
        paths.dedup_by(|left, right| left.edges == right.edges);

        let nodes = paths
            .iter()
            .flat_map(|path| path.nodes.iter().cloned())
            .chain([focus.clone()])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok(TraceResult {
            focus,
            direction,
            max_depth,
            nodes,
            paths,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_paths(
        &self,
        current: NodeIndex,
        direction: TraceDirection,
        relation: Option<&str>,
        max_depth: usize,
        visited: &mut BTreeSet<NodeRef>,
        path_nodes: &mut Vec<NodeRef>,
        path_edges: &mut Vec<TraceStep>,
        paths: &mut Vec<TracePath>,
    ) {
        if path_edges.len() == max_depth {
            return;
        }
        for candidate in self.candidates(current, direction, relation) {
            let next = self.graph[candidate.next].clone();
            if !visited.insert(next.clone()) {
                continue;
            }
            path_nodes.push(next.clone());
            path_edges.push(candidate.step);
            paths.push(TracePath {
                nodes: path_nodes.clone(),
                edges: path_edges.clone(),
            });
            self.collect_paths(
                candidate.next,
                direction,
                relation,
                max_depth,
                visited,
                path_nodes,
                path_edges,
                paths,
            );
            path_edges.pop();
            path_nodes.pop();
            visited.remove(&next);
        }
    }

    fn candidates(
        &self,
        current: NodeIndex,
        direction: TraceDirection,
        relation: Option<&str>,
    ) -> Vec<TraversalCandidate> {
        let mut candidates = Vec::new();
        if matches!(
            direction,
            TraceDirection::Outgoing | TraceDirection::Bidirectional
        ) {
            candidates.extend(
                self.graph
                    .edges_directed(current, Direction::Outgoing)
                    .filter(|edge| relation.is_none_or(|name| edge.weight().relation() == name))
                    .map(|edge| TraversalCandidate {
                        next: edge.target(),
                        step: trace_step(edge.weight(), TraversalDirection::Outgoing),
                    }),
            );
        }
        if matches!(
            direction,
            TraceDirection::Incoming | TraceDirection::Bidirectional
        ) {
            candidates.extend(
                self.graph
                    .edges_directed(current, Direction::Incoming)
                    .filter(|edge| relation.is_none_or(|name| edge.weight().relation() == name))
                    .map(|edge| TraversalCandidate {
                        next: edge.source(),
                        step: trace_step(edge.weight(), TraversalDirection::Incoming),
                    }),
            );
        }
        candidates.sort_by(|left, right| {
            canonical_step_json(&left.step)
                .cmp(&canonical_step_json(&right.step))
                .then_with(|| {
                    canonical_node_json(&self.graph[left.next])
                        .cmp(&canonical_node_json(&self.graph[right.next]))
                })
        });
        candidates
    }
}

impl Default for QueryGraph {
    fn default() -> Self {
        Self::build([], [])
    }
}

#[derive(Debug)]
struct TraversalCandidate {
    next: NodeIndex,
    step: TraceStep,
}

fn trace_step(edge: &ProjectionEdge, traversal: TraversalDirection) -> TraceStep {
    TraceStep {
        relation: edge.relation.clone(),
        traversal,
        source: edge.source.clone(),
        target: edge.target.clone(),
    }
}

fn compare_source_identities(left: &SourceSpan, right: &SourceSpan) -> Ordering {
    left.path()
        .as_bytes()
        .cmp(right.path().as_bytes())
        .then_with(|| left.start_byte().cmp(&right.start_byte()))
        .then_with(|| left.end_byte().cmp(&right.end_byte()))
}

fn compare_projection_edges(left: &ProjectionEdge, right: &ProjectionEdge) -> Ordering {
    canonical_node_json(left.source())
        .cmp(&canonical_node_json(right.source()))
        .then_with(|| left.relation().as_bytes().cmp(right.relation().as_bytes()))
        .then_with(|| canonical_node_json(left.target()).cmp(&canonical_node_json(right.target())))
}

fn compare_trace_paths(left: &TracePath, right: &TracePath) -> Ordering {
    left.edges
        .len()
        .cmp(&right.edges.len())
        .then_with(|| {
            canonical_node_sequence_json(&left.nodes)
                .cmp(&canonical_node_sequence_json(&right.nodes))
        })
        .then_with(|| {
            canonical_step_sequence_json(&left.edges)
                .cmp(&canonical_step_sequence_json(&right.edges))
        })
}

fn canonical_node_sequence_json(nodes: &[NodeRef]) -> Vec<u8> {
    canonical_json_array(nodes.iter().map(canonical_node_json))
}

fn canonical_step_sequence_json(steps: &[TraceStep]) -> Vec<u8> {
    canonical_json_array(steps.iter().map(canonical_step_json))
}

fn canonical_json_array(values: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    let mut bytes = vec![b'['];
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        bytes.extend(value);
    }
    bytes.push(b']');
    bytes
}

fn canonical_node_json(node: &NodeRef) -> Vec<u8> {
    let mut bytes = Vec::new();
    match node {
        NodeRef::Item { mid } => {
            bytes.extend(b"{\"kind\":\"item\",\"mid\":");
            push_json_string(&mut bytes, mid.as_str());
            bytes.push(b'}');
        }
        NodeRef::SourceSpan { source, symbol } => {
            bytes.extend(b"{\"kind\":\"source_span\",\"source\":{\"path\":");
            push_json_string(&mut bytes, source.path());
            bytes.extend(b",\"start_byte\":");
            push_number(&mut bytes, source.start_byte());
            bytes.extend(b",\"end_byte\":");
            push_number(&mut bytes, source.end_byte());
            bytes.extend(b",\"start_line\":");
            push_number(&mut bytes, source.start_line());
            bytes.extend(b",\"start_column\":");
            push_number(&mut bytes, source.start_column());
            bytes.extend(b",\"end_line\":");
            push_number(&mut bytes, source.end_line());
            bytes.extend(b",\"end_column\":");
            push_number(&mut bytes, source.end_column());
            bytes.extend(b"},\"symbol\":");
            match symbol {
                Some(symbol) => push_json_string(&mut bytes, symbol),
                None => bytes.extend(b"null"),
            }
            bytes.push(b'}');
        }
        NodeRef::External { uri } => {
            bytes.extend(b"{\"kind\":\"external\",\"uri\":");
            push_json_string(&mut bytes, uri);
            bytes.push(b'}');
        }
    }
    bytes
}

fn canonical_step_json(step: &TraceStep) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(b"{\"relation\":");
    push_json_string(&mut bytes, step.relation());
    bytes.extend(b",\"traversal\":");
    push_json_string(&mut bytes, step.traversal().as_str());
    bytes.extend(b",\"source\":");
    bytes.extend(canonical_node_json(step.source()));
    bytes.extend(b",\"target\":");
    bytes.extend(canonical_node_json(step.target()));
    bytes.push(b'}');
    bytes
}

fn push_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend(serde_json::to_vec(value).expect("serializing a string cannot fail"));
}

fn push_number(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_string().bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthoredReference, AuthoredRelationOrigin, CanonicalRelationInput, CanonicalRelations,
        MidFormat, MidIdentity, ReferenceOrigin, RelationOccurrence, ResolvedReference,
        SchemaField, SourceIndex,
    };

    fn identity() -> MidIdentity {
        let source = span("schema.yaml", "x", 0, 1);
        MidIdentity::new(
            SchemaField::new(source.clone(), source.clone(), MidFormat::Ulid),
            SchemaField::new(source.clone(), source, "m_".to_owned()),
        )
    }

    fn mid(number: u8) -> Mid {
        Mid::parse(&format!("m_{number:026}"), &identity()).unwrap()
    }

    fn item(number: u8) -> NodeRef {
        NodeRef::item(mid(number))
    }

    fn span(path: &str, source: &str, start: u64, end: u64) -> SourceSpan {
        let index = SourceIndex::try_new(path, source).unwrap();
        let (start_line, start_column) = index.coordinates_at(start).unwrap();
        let (end_line, end_column) = index.coordinates_at(end).unwrap();
        index
            .try_span(start, end, start_line, start_column, end_line, end_column)
            .unwrap()
    }

    fn edge(relation: &str, source: u8, target: u8) -> ProjectionEdge {
        ProjectionEdge::new(relation, item(source), item(target))
    }

    fn canonical_input(
        relation: &str,
        source: u8,
        target: u8,
        path: &str,
    ) -> CanonicalRelationInput {
        let source_mid = mid(source);
        let target_mid = mid(target);
        let reference_source = span(path, "ref", 0, 3);
        let authored = AuthoredReference::new(
            target_mid.to_string(),
            None,
            Some(relation.to_owned()),
            ReferenceOrigin::Item {
                mid: source_mid.clone(),
                display_id: None,
            },
            reference_source,
        );
        CanonicalRelationInput::new(
            relation.to_owned(),
            source_mid,
            target_mid.clone(),
            RelationOccurrence::new(
                ResolvedReference::new(target_mid, authored),
                AuthoredRelationOrigin::Direct,
            ),
        )
    }

    #[test]
    fn ordering_keys_follow_the_wire_json_key_order() {
        let source = NodeRef::source_span(span("src/lib.rs", "fn", 0, 2), Some("f".to_owned()));
        assert_eq!(
            String::from_utf8(canonical_node_json(&source)).unwrap(),
            r#"{"kind":"source_span","source":{"path":"src/lib.rs","start_byte":0,"end_byte":2,"start_line":1,"start_column":1,"end_line":1,"end_column":3},"symbol":"f"}"#
        );

        let step = trace_step(
            &ProjectionEdge::new("uses", item(1), item(2)),
            TraversalDirection::Incoming,
        );
        assert_eq!(
            String::from_utf8(canonical_step_json(&step)).unwrap(),
            format!(
                r#"{{"relation":"uses","traversal":"incoming","source":{{"kind":"item","mid":"{}"}},"target":{{"kind":"item","mid":"{}"}}}}"#,
                mid(1),
                mid(2)
            )
        );
    }

    #[test]
    fn canonical_projection_is_deterministic_and_deduplicates_occurrences() {
        let relations = CanonicalRelations::build(
            [
                canonical_input("verifies", 1, 2, "z.md"),
                canonical_input("satisfies", 1, 2, "b.md"),
                canonical_input("verifies", 1, 2, "a.md"),
            ],
            [],
        );
        let first = QueryGraph::from_canonical(
            [&mid(2), &mid(1)],
            relations.edges().iter().rev(),
            [edge("verifies", 1, 2)],
        );
        let second = QueryGraph::from_canonical([&mid(1), &mid(2)], relations.edges(), []);

        assert_eq!(first.nodes(), second.nodes());
        assert_eq!(first.edges(), second.edges());
        assert_eq!(first.edge_count(), 2);
        assert_eq!(
            first
                .edges()
                .iter()
                .map(ProjectionEdge::relation)
                .collect::<Vec<_>>(),
            ["satisfies", "verifies"]
        );
    }

    #[test]
    fn trace_applies_direction_relation_and_depth_to_canonical_steps() {
        let graph = QueryGraph::build(
            [item(1), item(2), item(3), item(4)],
            [
                edge("uses", 1, 2),
                edge("uses", 3, 1),
                edge("implements", 2, 4),
            ],
        );

        let outgoing = graph
            .trace(&item(1), TraceDirection::Outgoing, None, 2)
            .unwrap();
        assert_eq!(outgoing.paths().len(), 2);
        assert_eq!(
            outgoing.paths()[0].edges()[0].traversal(),
            TraversalDirection::Outgoing
        );
        assert_eq!(outgoing.paths()[1].edges().len(), 2);
        assert_eq!(outgoing.paths()[1].edges()[1].relation(), "implements");

        let incoming = graph
            .trace(&item(1), TraceDirection::Incoming, None, 2)
            .unwrap();
        assert_eq!(incoming.paths().len(), 1);
        assert_eq!(incoming.paths()[0].nodes(), [item(1), item(3)]);
        assert_eq!(incoming.paths()[0].edges()[0].source(), &item(3));
        assert_eq!(incoming.paths()[0].edges()[0].target(), &item(1));
        assert_eq!(
            incoming.paths()[0].edges()[0].traversal(),
            TraversalDirection::Incoming
        );

        let filtered = graph
            .trace(&item(1), TraceDirection::Bidirectional, Some("uses"), 1)
            .unwrap();
        assert_eq!(filtered.paths().len(), 2);
        assert!(
            filtered
                .paths()
                .iter()
                .all(|path| path.edges()[0].relation() == "uses")
        );
    }

    #[test]
    fn cycle_and_diamond_return_distinct_simple_paths_without_zero_step_path() {
        let cycle = QueryGraph::build(
            [item(1), item(2), item(3)],
            [edge("next", 1, 2), edge("next", 2, 3), edge("next", 3, 1)],
        );
        let cycle_result = cycle
            .trace(&item(1), TraceDirection::Outgoing, None, 5)
            .unwrap();
        assert_eq!(cycle_result.paths().len(), 2);
        assert_eq!(cycle_result.paths()[0].nodes(), [item(1), item(2)]);
        assert_eq!(cycle_result.paths()[1].nodes(), [item(1), item(2), item(3)]);
        assert!(
            cycle_result
                .paths()
                .iter()
                .all(|path| !path.edges().is_empty())
        );

        let diamond = QueryGraph::build(
            [item(1), item(2), item(3), item(4)],
            [
                edge("branch", 1, 2),
                edge("branch", 1, 3),
                edge("join", 2, 4),
                edge("join", 3, 4),
            ],
        );
        let diamond_result = diamond
            .trace(&item(1), TraceDirection::Outgoing, None, 2)
            .unwrap();
        assert_eq!(diamond_result.paths().len(), 4);
        assert_eq!(
            diamond_result
                .paths()
                .iter()
                .filter(|path| path.nodes().last() == Some(&item(4)))
                .count(),
            2
        );
    }

    #[test]
    fn parallel_relations_are_distinct_paths_but_duplicate_edges_are_not() {
        let graph = QueryGraph::build(
            [item(1), item(2)],
            [
                edge("implements", 1, 2),
                edge("verifies", 1, 2),
                edge("verifies", 1, 2),
            ],
        );
        let result = graph
            .trace(&item(1), TraceDirection::Outgoing, None, 1)
            .unwrap();

        assert_eq!(graph.edge_count(), 2);
        assert_eq!(result.paths().len(), 2);
        assert_eq!(
            result
                .paths()
                .iter()
                .map(|path| path.edges()[0].relation())
                .collect::<Vec<_>>(),
            ["implements", "verifies"]
        );
    }

    #[test]
    fn empty_missing_and_zero_depth_requests_are_structured() {
        let focus = item(1);
        assert_eq!(
            QueryGraph::default().trace(&focus, TraceDirection::Outgoing, None, 1),
            Err(TraceError::MissingFocus {
                focus: focus.clone()
            })
        );
        let isolated = QueryGraph::build([focus.clone()], []);
        let result = isolated
            .trace(&focus, TraceDirection::Bidirectional, None, 1)
            .unwrap();
        assert_eq!(result.nodes(), std::slice::from_ref(&focus));
        assert!(result.paths().is_empty());
        assert_eq!(
            isolated.trace(&focus, TraceDirection::Outgoing, None, 0),
            Err(TraceError::ZeroDepth)
        );
    }

    #[test]
    fn normalized_fixture_seam_traces_external_and_derived_endpoints() {
        let focus = item(1);
        let source = NodeRef::source_span(
            span("src/lib.rs", "fn traced() {}", 3, 9),
            Some("traced".to_owned()),
        );
        let external = NodeRef::external("linear://CON-17");
        let graph = QueryGraph::from_canonical(
            [&mid(1)],
            [],
            [
                ProjectionEdge::new("mentions", source.clone(), focus.clone()),
                ProjectionEdge::new("delivered_by", focus.clone(), external.clone()),
            ],
        );
        let result = graph
            .trace(&focus, TraceDirection::Bidirectional, None, 3)
            .unwrap();

        assert_eq!(result.paths().len(), 2);
        assert!(result.nodes().contains(&source));
        assert!(result.nodes().contains(&external));
        assert_eq!(
            result
                .paths()
                .iter()
                .map(|path| path.edges()[0].traversal())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([TraversalDirection::Incoming, TraversalDirection::Outgoing,])
        );
    }
}
