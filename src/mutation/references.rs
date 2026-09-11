//! Compare resolved links through source correspondence, never snapshot handles
//! or generated anchor names (both can change during an otherwise safe edit).

use std::{collections::BTreeMap, ops::Range, path::PathBuf};

use similar::{ChangeTag, TextDiff};

use crate::{
    ConnectionKind, Corpus, DiscoveryNode, DiscoveryNodeKind, Error, ReferenceKind,
    RelationDirection, SourceLocation,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Item(String),
    Narrative(PathBuf),
}

struct Part {
    local: Range<usize>,
    path: PathBuf,
    start: usize,
}

#[derive(Default)]
struct Source {
    text: String,
    parts: Vec<Part>,
}

impl Source {
    fn push(&mut self, path: PathBuf, start: usize, text: &str) {
        let local = self.text.len()..self.text.len() + text.len();
        self.text.push_str(text);
        self.parts.push(Part { local, path, start });
    }

    fn local(&self, path: &std::path::Path, byte: usize) -> Option<usize> {
        self.parts.iter().find_map(|part| {
            (part.path == path && (part.start..part.start + part.local.len()).contains(&byte))
                .then(|| part.local.start + byte - part.start)
        })
    }

    fn physical(&self, byte: usize) -> Option<(PathBuf, usize)> {
        self.parts.iter().find_map(|part| {
            part.local
                .contains(&byte)
                .then(|| (part.path.clone(), part.start + byte - part.local.start))
        })
    }

    fn content(&self, source: &SourceLocation) -> String {
        let mut text = String::new();
        for part in &self.parts {
            if part.path != source.path() {
                continue;
            }
            let start = part.start.max(source.span().start_byte());
            let end = (part.start + part.local.len()).min(source.span().end_byte());
            if start < end {
                let local = part.local.start + start - part.start;
                text.push_str(&self.text[local..local + end - start]);
            }
        }
        text
    }
}

fn sources(corpus: &Corpus) -> BTreeMap<Scope, Source> {
    let mut sources = BTreeMap::<Scope, Source>::new();
    for document in corpus.documents() {
        let path = document.path().to_path_buf();
        let narrative = Scope::Narrative(path.clone());
        let mut cursor = 0;
        for item in document.items() {
            let span = item.source().span();
            sources.entry(narrative.clone()).or_default().push(
                path.clone(),
                cursor,
                &document.source()[cursor..span.start_byte()],
            );
            sources
                .entry(Scope::Item(item.mid().unwrap_or(item.id()).to_owned()))
                .or_default()
                .push(
                    path.clone(),
                    span.start_byte(),
                    &document.source()[span.start_byte()..span.end_byte()],
                );
            cursor = span.end_byte();
        }
        sources
            .entry(narrative)
            .or_default()
            .push(path, cursor, &document.source()[cursor..]);
    }
    sources
}

struct Correspondence {
    before: Source,
    after: Source,
    equal: Vec<(Range<usize>, usize)>,
}

impl Correspondence {
    fn new(before: Source, after: Source) -> Self {
        let mut equal: Vec<(Range<usize>, usize)> = Vec::new();
        let (mut old, mut new) = (0, 0);
        for change in TextDiff::from_chars(&before.text, &after.text).iter_all_changes() {
            let len = change.value().len();
            match change.tag() {
                ChangeTag::Equal => {
                    if let Some((range, start)) = equal.last_mut()
                        && range.end == old
                        && *start + range.len() == new
                    {
                        range.end += len;
                    } else {
                        equal.push((old..old + len, new));
                    }
                    old += len;
                    new += len;
                }
                ChangeTag::Delete => old += len,
                ChangeTag::Insert => new += len,
            }
        }
        Self {
            before,
            after,
            equal,
        }
    }

    fn point(&self, path: &std::path::Path, byte: usize) -> Option<(PathBuf, usize)> {
        let old = self.before.local(path, byte)?;
        let new = self
            .equal
            .iter()
            .find_map(|(range, start)| range.contains(&old).then(|| start + old - range.start))?;
        self.after.physical(new)
    }
}

fn point(
    maps: &[Correspondence],
    source: &SourceLocation,
    byte: usize,
) -> Option<(PathBuf, usize)> {
    maps.iter().find_map(|map| map.point(source.path(), byte))
}

fn same_destination(
    old: DiscoveryNode<'_, '_>,
    new: DiscoveryNode<'_, '_>,
    maps: &[Correspondence],
    old_graph: &crate::DiscoveryGraph<'_>,
    new_graph: &crate::DiscoveryGraph<'_>,
) -> bool {
    let (old_source, new_source) = match (old.kind(), new.kind()) {
        (DiscoveryNodeKind::Item(a), DiscoveryNodeKind::Item(b)) => {
            return a.mid().unwrap_or(a.id()) == b.mid().unwrap_or(b.id());
        }
        (DiscoveryNodeKind::Document(a), DiscoveryNodeKind::Document(b)) => {
            return a.path() == b.path();
        }
        (DiscoveryNodeKind::Section { heading: a }, DiscoveryNodeKind::Section { heading: b }) => {
            (a.source(), b.source())
        }
        (DiscoveryNodeKind::MarkdownBlock(a), DiscoveryNodeKind::MarkdownBlock(b))
            if a.kind() == b.kind() =>
        {
            (a.source(), b.source())
        }
        _ => return false,
    };
    if point(maps, old_source, old_source.span().start_byte())
        != Some((
            new_source.path().to_path_buf(),
            new_source.span().start_byte(),
        ))
    {
        return false;
    }
    // A text diff can align an identical heading with a newly inserted duplicate.
    // Its surviving content must belong to the same destination too. Compare only
    // the heading/block's owning scope: moving a contained item out of a narrative
    // section does not replace that section's identity.
    let map = maps
        .iter()
        .find(|map| {
            map.before
                .local(old_source.path(), old_source.span().start_byte())
                .is_some()
        })
        .unwrap();
    if let DiscoveryNodeKind::Section { heading } = old.kind() {
        let old_content = map.before.content(old.source());
        let new_content = map.after.content(new.source());
        let same_heading = |node: DiscoveryNode<'_, '_>, source: &Source| {
            matches!(node.kind(), DiscoveryNodeKind::Section { heading: candidate }
                if candidate.heading_text() == heading.heading_text())
                && source
                    .local(node.source().path(), node.source().span().start_byte())
                    .is_some()
        };
        let old_sections = old_graph
            .nodes()
            .filter(|node| same_heading(*node, &map.before))
            .collect::<Vec<_>>();
        let new_count = new_graph
            .nodes()
            .filter(|node| same_heading(*node, &map.after))
            .count();
        // Repeated headings have no persisted identity. An unchanged complete
        // section is stronger evidence than a diff's choice of equal '#' bytes.
        if (old_sections.len() > 1 || new_count > 1)
            && old_content.trim() != new_content.trim()
            && (old_sections.len() != new_count
                || old_sections.iter().any(|node| {
                    node.source() != old.source()
                        && map.before.content(node.source()).trim() == new_content.trim()
                }))
        {
            return false;
        }
    }
    for part in &map.before.parts {
        if part.path != old.source().path() {
            continue;
        }
        let start = part.start.max(old.source().span().start_byte());
        let end = (part.start + part.local.len()).min(old.source().span().end_byte());
        if start >= end {
            continue;
        }
        let local = part.local.start + start - part.start;
        for (offset, character) in map.before.text[local..local + end - start].char_indices() {
            if character.is_whitespace() {
                continue;
            }
            if let Some((path, byte)) = map.point(&part.path, start + offset)
                && (path != new.source().path()
                    || !(new.source().span().start_byte()..new.source().span().end_byte())
                        .contains(&byte))
            {
                return false;
            }
        }
    }
    true
}

fn connections<'graph, 'corpus>(
    graph: &'graph crate::DiscoveryGraph<'corpus>,
) -> BTreeMap<(PathBuf, usize, usize), DiscoveryNode<'graph, 'corpus>> {
    graph
        .nodes()
        .flat_map(|node| node.connections(RelationDirection::Outgoing))
        .filter(|edge| {
            matches!(
                edge.kind,
                ConnectionKind::Mentions | ConnectionKind::Schema(_)
            )
        })
        .map(|edge| {
            (
                (
                    edge.source.path().to_path_buf(),
                    edge.source.span().start_byte(),
                    edge.source.span().end_byte(),
                ),
                edge.neighbour,
            )
        })
        .collect()
}

/// Every byte retained in a surviving link must still resolve to its original
/// destination. Rename may change an ID token, but not its resolved identity.
pub(super) fn preflight(
    before: &Corpus,
    after: &Corpus,
    rename: Option<(&str, &str)>,
) -> Result<(), Error> {
    let mut new_sources = sources(after);
    let maps = sources(before)
        .into_iter()
        .filter_map(|(scope, before)| {
            new_sources
                .remove(&scope)
                .map(|after| Correspondence::new(before, after))
        })
        .collect::<Vec<_>>();
    let old_graph = before.discovery();
    let new_graph = after.discovery();
    let new_connections = connections(&new_graph);
    let mut impacts = BTreeMap::new();
    for (location, target) in connections(&old_graph) {
        let document = before
            .documents()
            .iter()
            .find(|doc| doc.path() == location.0)
            .unwrap();
        let reference = document.references().iter().find(|reference| {
            reference.kind() != ReferenceKind::Anchor
                && reference.source().span().start_byte() == location.1
                && reference.source().span().end_byte() == location.2
        });
        let (source, written_target, relation_name) = if let Some(reference) = reference {
            (reference.source(), reference.target(), None)
        } else {
            let relation = document
                .items()
                .iter()
                .flat_map(|item| item.relations())
                .find(|relation| {
                    relation.source().span().start_byte() == location.1
                        && relation.source().span().end_byte() == location.2
                })
                .expect("schema edge has relation evidence");
            (relation.source(), relation.target(), Some(relation.name()))
        };
        let Some((path, start)) = point(&maps, source, location.1) else {
            continue;
        };
        let Some((end_path, last)) = point(&maps, source, location.2 - 1) else {
            continue;
        };
        if path != end_path {
            continue;
        }
        let candidate_doc = after
            .documents()
            .iter()
            .find(|doc| doc.path() == path)
            .unwrap();
        let raw = &document.source()[location.1..location.2];
        let expected = if let Some((old, new)) = rename
            && reference.is_some_and(|reference| {
                reference.kind() == ReferenceKind::Item && reference.target() == old
            }) {
            format!("[[{new}]]")
        } else {
            raw.to_owned()
        };
        if candidate_doc.source().get(start..last + 1) != Some(expected.as_str()) {
            continue; // Explicitly edited or removed reference; candidate validation owns it.
        }
        if new_connections
            .get(&(path, start, last + 1))
            .is_none_or(|candidate| {
                !same_destination(target, *candidate, &maps, &old_graph, &new_graph)
            })
        {
            let owner = document.items().iter().find(|item| {
                item.source().span().start_byte() <= location.1
                    && item.source().span().end_byte() >= location.2
            });
            let context = owner.map_or_else(String::new, |item| {
                relation_name.map_or_else(
                    || format!(" (item '{}' mention)", item.id()),
                    |name| format!(" (item '{}' relation '{name}')", item.id()),
                )
            });
            impacts.insert(
                location,
                format!(
                    "{}:{} (bytes {}..{}): untouched link '{}' would break or change destination{context}",
                    source.path().display(),
                    source.span().start_line(),
                    source.span().start_byte(),
                    source.span().end_byte(),
                    written_target,
                ),
            );
        }
    }
    if impacts.is_empty() {
        Ok(())
    } else {
        super::invalid(format!(
            "mutation would change surviving references; resolve link impacts before retrying:\n{}",
            impacts.into_values().collect::<Vec<_>>().join("\n")
        ))
    }
}
