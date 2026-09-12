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
    let old_source = match (old.kind(), new.kind()) {
        (DiscoveryNodeKind::Item(a), DiscoveryNodeKind::Item(b)) => {
            return a.mid().unwrap_or(a.id()) == b.mid().unwrap_or(b.id());
        }
        (DiscoveryNodeKind::Document(a), DiscoveryNodeKind::Document(b)) => {
            return a.path() == b.path();
        }
        (DiscoveryNodeKind::Section { heading: a }, DiscoveryNodeKind::Section { .. }) => {
            a.source()
        }
        (DiscoveryNodeKind::MarkdownBlock(a), DiscoveryNodeKind::MarkdownBlock(b))
            if a.kind() == b.kind() =>
        {
            a.source()
        }
        _ => return false,
    };
    // A text diff can align an identical heading with a newly inserted duplicate.
    // Its surviving content must belong to the same destination too. Compare only
    // the heading/block's owning scope: moving a contained item out of a narrative
    // section does not replace that section's identity.
    let Some(map) = maps.iter().find(|map| {
        map.before
            .local(old_source.path(), old_source.span().start_byte())
            .is_some()
    }) else {
        return false;
    };
    if map
        .after
        .local(new.source().path(), new.source().span().start_byte())
        .is_none()
    {
        return false;
    }
    let old_content = map.before.content(old.source());
    let new_content = map.after.content(new.source());
    // An intact, unique structural node survives a reorder even when a character
    // diff represents all of it as deletion/insertion. Require uniqueness in both
    // snapshots so identical blocks or duplicate headings do not confer identity.
    let matches_content = |node: DiscoveryNode<'_, '_>, source: &Source, content: &str| {
        let same_kind = match (old.kind(), node.kind()) {
            (DiscoveryNodeKind::Section { .. }, DiscoveryNodeKind::Section { .. }) => true,
            (DiscoveryNodeKind::MarkdownBlock(a), DiscoveryNodeKind::MarkdownBlock(b)) => {
                a.kind() == b.kind()
            }
            _ => false,
        };
        same_kind
            && source
                .local(node.source().path(), node.source().span().start_byte())
                .is_some()
            && source.content(node.source()).trim() == content.trim()
    };
    let mut retained = new_graph
        .nodes()
        .filter(|node| matches_content(*node, &map.after, &old_content));
    let first_retained = retained.next();
    if let Some(candidate) = first_retained
        && retained.next().is_none()
        && old_graph
            .nodes()
            .filter(|node| matches_content(*node, &map.before, &old_content))
            .take(2)
            .count()
            == 1
    {
        return candidate.source() == new.source();
    }
    // An anchored block may be rewritten completely. Once an intact original
    // elsewhere (including duplicate matches) has been ruled out, the same slot
    // in a corresponding container identifies that edited block without requiring
    // any shared characters.
    if matches!(old.kind(), DiscoveryNodeKind::MarkdownBlock(_))
        && first_retained.is_none()
        && let (Some(old_parent), Some(new_parent)) = (old.parent(), new.parent())
        && same_destination(old_parent, new_parent, maps, old_graph, new_graph)
    {
        // A surviving sibling can occupy the deleted target's slot. Its intact
        // content establishes a different origin, even when the old target has
        // no characters left for the diff to track.
        if old_graph.nodes().any(|node| {
            node.source() != old.source() && matches_content(node, &map.before, &new_content)
        }) {
            return false;
        }
        let old_slot = old_parent
            .children()
            .iter()
            .position(|node| node.source() == old.source());
        let new_slot = new_parent
            .children()
            .iter()
            .position(|node| node.source() == new.source());
        if old_slot.is_some() && old_slot == new_slot {
            return true;
        }
    }
    let mut correspondence_source = old.source();
    let mut destination_source = new.source();
    if let DiscoveryNodeKind::Section { heading } = old.kind() {
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
        // Like link usages, a moved paragraph can be absent from the diff's
        // equal ranges. A uniquely retained direct block identifies its section
        // more reliably than equal heading punctuation or a shared trailing dot.
        for child in old.children() {
            let DiscoveryNodeKind::MarkdownBlock(block) = child.kind() else {
                continue;
            };
            let content = map.before.content(child.source());
            let mut retained = new_graph.nodes().filter(|node| {
                matches!(node.kind(), DiscoveryNodeKind::MarkdownBlock(candidate) if candidate.kind() == block.kind())
                    && map.after.local(node.source().path(), node.source().span().start_byte()).is_some()
                    && map.after.content(node.source()).trim() == content.trim()
            });
            if let Some(candidate) = retained.next()
                && retained.next().is_none()
                && (candidate.source().path() != new.source().path()
                    || candidate.source().span().start_byte() < new.source().span().start_byte()
                    || candidate.source().span().end_byte() > new.source().span().end_byte())
            {
                return false;
            }
        }
        // Unique heading text still needs source correspondence, and an intact
        // direct block surviving elsewhere rules out a replacement section above.
        // Compare the heading itself so promotion/demotion of descendants can
        // change section extent without changing its destination identity.
        if old_sections.len() == 1
            && new_count == 1
            && same_heading(new, &map.after)
            && let DiscoveryNodeKind::Section { heading: candidate } = new.kind()
        {
            correspondence_source = heading.source();
            destination_source = candidate.source();
        }
    }
    // Edited blocks need retained content, not a retained first byte. All mapped
    // non-whitespace content must remain inside the same candidate destination.
    let mut retained_content = false;
    for part in &map.before.parts {
        if part.path != correspondence_source.path() {
            continue;
        }
        let start = part.start.max(correspondence_source.span().start_byte());
        let end = (part.start + part.local.len()).min(correspondence_source.span().end_byte());
        if start >= end {
            continue;
        }
        let local = part.local.start + start - part.start;
        for (offset, character) in map.before.text[local..local + end - start].char_indices() {
            if character.is_whitespace() {
                continue;
            }
            if let Some((path, byte)) = map.point(&part.path, start + offset) {
                if path != destination_source.path()
                    || !(destination_source.span().start_byte()
                        ..destination_source.span().end_byte())
                        .contains(&byte)
                {
                    return false;
                }
                retained_content = true;
            }
        }
    }
    retained_content
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
    edited_body: Option<&SourceLocation>,
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
        let raw = &document.source()[location.1..location.2];
        let expected = if let Some((old, new)) = rename
            && reference.is_some_and(|reference| {
                reference.kind() == ReferenceKind::Item && reference.target() == old
            }) {
            format!("[[{new}]]")
        } else {
            raw.to_owned()
        };
        let mapped = point(&maps, source, location.1).and_then(|(path, start)| {
            let (end_path, last) = point(&maps, source, location.2 - 1)?;
            let document = after.documents().iter().find(|doc| doc.path() == path)?;
            (path == end_path && document.source().get(start..last + 1) == Some(expected.as_str()))
                .then_some((path, start, last + 1))
        });
        // Diff equality is only a hint: relocation may be represented entirely
        // as deletion/insertion. Match parsed occurrences in the surviving item
        // or narrative scope before treating an unmapped usage as removed.
        let surviving = reference.and_then(|original| {
            let map = maps
                .iter()
                .find(|map| map.before.local(source.path(), location.1).is_some())?;
            after
                .documents()
                .iter()
                .flat_map(|doc| {
                    doc.references().iter().filter(|candidate| {
                        let span = candidate.source().span();
                        candidate.kind() == original.kind()
                            && map.after.local(doc.path(), span.start_byte()).is_some()
                            && doc.source().get(span.start_byte()..span.end_byte())
                                == Some(expected.as_str())
                    })
                })
                .min_by_key(|candidate| {
                    let span = candidate.source().span();
                    let key = (
                        candidate.source().path().to_path_buf(),
                        span.start_byte(),
                        span.end_byte(),
                    );
                    (Some(&key) != mapped.as_ref(), key)
                })
        });
        if let (Some(original), Some(candidate)) = (reference, surviving)
            && original.kind() == ReferenceKind::MarkdownLink
            && edited_body.map(SourceLocation::path) == Some(source.path())
            && candidate.source().path() == source.path()
            && original.target() != candidate.target()
        {
            // Equal link usage with a changed parsed destination in a body-update
            // document means its reference definition was edited. Candidate
            // validation owns the new destination. Moves must not get this exemption.
            continue;
        }
        let Some((path, start, end)) = surviving
            .map(|candidate| {
                let span = candidate.source().span();
                (
                    candidate.source().path().to_path_buf(),
                    span.start_byte(),
                    span.end_byte(),
                )
            })
            .or_else(|| {
                // Literal-context edits inside the explicitly replaced body can
                // remove a parsed reference while retaining all its raw bytes.
                // Outside that body, disappearance still needs protection: a
                // mutation must not silently hide an untouched reference.
                let explicitly_replaced = reference.is_some()
                    && edited_body.is_some_and(|body| {
                        body.path() == source.path()
                            && body.span().start_byte() <= location.1
                            && body.span().end_byte() >= location.2
                    });
                if explicitly_replaced { None } else { mapped }
            })
        else {
            continue; // Explicitly edited or removed reference.
        };
        if new_connections
            .get(&(path, start, end))
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
