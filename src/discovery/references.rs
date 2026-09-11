use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

use super::*;
use crate::{DocumentReference, ReferenceKind, corpus::diagnostic};

type Anchors = BTreeMap<(PathBuf, String), Vec<(NodeIndex, SourceLocation)>>;

impl<'corpus> DiscoveryGraph<'corpus> {
    pub(super) fn add_references(&mut self, corpus: &'corpus Corpus) {
        let mut anchors: Anchors = BTreeMap::new();
        let mut generated: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
        let mut documents = BTreeMap::new();
        let mut items: BTreeMap<&str, Vec<NodeIndex>> = BTreeMap::new();
        for index in self.graph.node_indices() {
            let data = &self.graph[index];
            match data.kind {
                DiscoveryNodeKind::Document(document) => {
                    documents.insert(document.path(), index);
                }
                DiscoveryNodeKind::Item(item) => {
                    items.entry(item.id()).or_default().push(index);
                    // Match validation's identity index even for recoverable malformed metadata.
                    let mids = item
                        .metadata()
                        .iter()
                        .filter(|entry| entry.key() == "mid" && crate::is_mid(entry.value()))
                        .map(|entry| entry.value())
                        .collect::<BTreeSet<_>>();
                    for mid in mids {
                        items.entry(mid).or_default().push(index);
                    }
                }
                DiscoveryNodeKind::Section { heading } => {
                    let base = heading_anchor(heading.heading_text().unwrap_or_default());
                    let used = generated.entry(data.source.path().to_owned()).or_default();
                    let mut anchor = base.clone();
                    let mut number = 0;
                    while !used.insert(anchor.clone()) {
                        number += 1;
                        anchor = format!("{base}-{number}");
                    }
                    anchors
                        .entry((data.source.path().to_owned(), anchor))
                        .or_default()
                        .push((index, heading.source().clone()));
                }
                _ => {}
            }
        }
        for document in corpus.documents() {
            for reference in document
                .references()
                .iter()
                .filter(|r| r.kind() == ReferenceKind::Anchor)
            {
                if let Some(target) = self.anchor_node(reference, document) {
                    anchors
                        .entry((document.path().to_owned(), reference.target().to_owned()))
                        .or_default()
                        .push((target, reference.source().clone()));
                }
            }
        }
        for ((_, name), targets) in anchors.iter().filter(|(_, targets)| targets.len() > 1) {
            for (_, source) in targets {
                diagnostic(
                    &mut self.diagnostics,
                    source,
                    format!("ambiguous anchor '{name}'"),
                );
            }
        }
        for document in corpus.documents() {
            for reference in document
                .references()
                .iter()
                .filter(|r| r.kind() != ReferenceKind::Anchor)
            {
                let Some(source_node) = self.reference_node(reference.source(), true) else {
                    continue;
                };
                let resolution = match reference.kind() {
                    ReferenceKind::Item => resolve_items(&items, reference.target()),
                    ReferenceKind::MarkdownLink => {
                        resolve_link(document.path(), reference.target(), &documents, &anchors)
                    }
                    ReferenceKind::Anchor => unreachable!(),
                };
                match resolution {
                    Resolution::One(target) => {
                        self.graph.add_edge(
                            source_node,
                            target,
                            EdgeData {
                                kind: EdgeKind::Mentions,
                                source: reference.source().clone(),
                            },
                        );
                    }
                    Resolution::Missing(kind) if corpus.is_complete() => diagnostic(
                        &mut self.diagnostics,
                        reference.source(),
                        format!("{kind} '{}'", reference.target()),
                    ),
                    Resolution::Ambiguous(kind) => diagnostic(
                        &mut self.diagnostics,
                        reference.source(),
                        format!("{kind} '{}'", reference.target()),
                    ),
                    _ => {}
                }
            }
        }
    }

    fn parent_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.graph
            .edges_directed(node, Direction::Incoming)
            .find(|edge| matches!(edge.weight().kind, EdgeKind::Contains))
            .map(|edge| edge.source())
    }

    fn containing_node(&self, source: &SourceLocation) -> Option<NodeIndex> {
        self.graph.node_indices().rfind(|&index| {
            let candidate = &self.graph[index].source;
            candidate.path() == source.path()
                && candidate.span().start_byte() <= source.span().start_byte()
                && candidate.span().end_byte() >= source.span().end_byte()
        })
    }

    /// Preserve destination structure inside an item; ownership only groups link sources.
    fn reference_node(&self, source: &SourceLocation, item_owner: bool) -> Option<NodeIndex> {
        let mut node = self.containing_node(source)?;
        let mut ancestor = node;
        while let Some(parent) = self.parent_index(ancestor) {
            match self.graph[parent].kind {
                DiscoveryNodeKind::Item(_) if item_owner => return Some(parent),
                DiscoveryNodeKind::MarkdownBlock(_) => node = parent,
                _ => {}
            }
            ancestor = parent;
        }
        Some(node)
    }

    fn anchor_node(&self, anchor: &DocumentReference, document: &Document) -> Option<NodeIndex> {
        let node = self.containing_node(anchor.source())?;
        let default = self.reference_node(anchor.source(), false)?;
        let span = self.graph[node].source.span();
        let anchor_span = anchor.source().span();
        // HTML blocks can contain several declarations. Placement belongs to
        // this declaration's line, not all earlier content in the shared block.
        let line_start = document.source()[..anchor_span.start_byte()]
            .rfind('\n')
            .map_or(0, |offset| offset + 1)
            .max(span.start_byte());
        let standalone = document.source()[line_start..anchor_span.start_byte()]
            .chars()
            .all(|c| c.is_whitespace() || c == '>')
            && document.source()[anchor_span.end_byte()..span.end_byte()]
                .trim()
                .is_empty();
        if !standalone {
            return Some(default);
        }
        // Adjacent source blocks only. Never jump past an item, container, or section end.
        let next = self.graph.node_indices().find(|&index| {
            self.graph[index].source.path() == document.path()
                && self.graph[index].source.span().start_byte() >= span.end_byte()
                && index != node
        });
        if let Some(next) = next {
            let gap =
                &document.source()[span.end_byte()..self.graph[next].source.span().start_byte()];
            let same_scope = self.markdown_parent(node) == self.markdown_parent(next);
            let same_section = self.parent_index(node) == self.parent_index(next);
            // Quote prefixes between parsed siblings are structural separators.
            if gap.chars().all(|c| c.is_whitespace() || c == '>')
                && same_scope
                && (matches!(self.graph[next].kind, DiscoveryNodeKind::Section { .. })
                    || (same_section
                        && matches!(self.graph[next].kind, DiscoveryNodeKind::MarkdownBlock(_))))
            {
                return if matches!(self.graph[next].kind, DiscoveryNodeKind::Section { .. }) {
                    Some(next)
                } else {
                    self.reference_node(&self.graph[next].source, false)
                };
            }
        }
        Some(default)
    }

    fn markdown_parent(&self, node: NodeIndex) -> Option<NodeIndex> {
        let mut parent = self.parent_index(node);
        while let Some(index) = parent {
            if !matches!(self.graph[index].kind, DiscoveryNodeKind::Section { .. }) {
                break;
            }
            parent = self.parent_index(index);
        }
        parent
    }
}

enum Resolution {
    One(NodeIndex),
    Missing(&'static str),
    Ambiguous(&'static str),
    SourceOnly,
}

fn resolve_items(items: &BTreeMap<&str, Vec<NodeIndex>>, target: &str) -> Resolution {
    match items.get(target).map(Vec::as_slice) {
        Some([target]) => Resolution::One(*target),
        Some(_) => Resolution::Ambiguous("mention references ambiguous item"),
        None => Resolution::Missing("mention references missing item"),
    }
}

fn resolve_link(
    source: &Path,
    target: &str,
    documents: &BTreeMap<&Path, NodeIndex>,
    anchors: &Anchors,
) -> Resolution {
    // URI schemes and network-path references stay source content. Do not read the network.
    if target.starts_with("//")
        || target.split_once(':').is_some_and(|(scheme, _)| {
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
    {
        return Resolution::SourceOnly;
    }
    let (path, fragment) = target
        .split_once('#')
        .map_or((target, None), |(p, f)| (p, Some(f)));
    let path = percent_decode(path);
    // Only canonical Mara documents have a target contract; code and other assets remain links.
    if !path.is_empty() && !path.ends_with(".mara.md") {
        return Resolution::SourceOnly;
    }
    let destination = if path.is_empty() {
        source.to_owned()
    } else {
        let base = if path.starts_with('/') {
            Path::new("")
        } else {
            source.parent().unwrap_or(Path::new(""))
        };
        let Some(path) = normalize(&base.join(path.trim_start_matches('/'))) else {
            return Resolution::Missing("link references missing internal document");
        };
        path
    };
    let Some(&document) = documents.get(destination.as_path()) else {
        return Resolution::Missing("link references missing internal document");
    };
    let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
        return Resolution::One(document);
    };
    match anchors
        .get(&(destination, percent_decode(fragment)))
        .map(Vec::as_slice)
    {
        Some([(target, _)]) => Resolution::One(*target),
        Some(_) => Resolution::Ambiguous("link references ambiguous anchor"),
        None => Resolution::Missing("link references missing internal anchor"),
    }
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir if result.pop() => {}
            _ => return None,
        }
    }
    Some(result)
}

fn percent_decode(text: &str) -> String {
    let mut bytes = Vec::new();
    let mut input = text.as_bytes().iter().copied().peekable();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let mut lookahead = input.clone();
            if let (Some(a), Some(b)) = (lookahead.next(), lookahead.next())
                && let (Some(a), Some(b)) = ((a as char).to_digit(16), (b as char).to_digit(16))
            {
                bytes.push((a * 16 + b) as u8);
                input = lookahead;
                continue;
            }
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

fn heading_anchor(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c == ' ' {
                Some('-')
            } else if c.is_alphanumeric()
                || matches!(c, '-' | '_')
                || unicode_normalization::char::is_combining_mark(c)
            {
                Some(c)
            } else {
                None
            }
        })
        .collect()
}
