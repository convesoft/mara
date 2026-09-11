use super::{page::*, *};
use crate::{
    DiscoveryNode, DiscoveryNodeKind, DiscoveryNodeSummary, MarkdownBlock, MarkdownBlockKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SearchHit {
    pub node: DiscoveryNodeSummary,
    pub excerpt: SearchExcerpt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SearchResult {
    pub format_version: u8,
    pub results: Vec<SearchHit>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// Unified discovery over owning items, section headings and outermost blocks.
pub fn search(
    corpus: &Corpus,
    schema: &Schema,
    query: &str,
    filters: &ItemFilters,
) -> Result<SearchResult, QueryError> {
    let limit = page_limit(filters.limit)?;
    let fingerprint = fingerprint(
        corpus,
        schema,
        &(
            "discovery-search-v1",
            query,
            &filters.flavours,
            &filters.fields,
            &filters.relations,
            &filters.paths,
            &filters.ids,
            limit,
        ),
    )?;
    let start = cursor_position(filters.cursor.as_deref(), &fingerprint)?;
    let mut item_filters = filters.clone();
    item_filters.relations = filters
        .relations
        .iter()
        .map(|name| {
            if let Some(name) = name.strip_prefix("schema:") {
                return Ok(name.to_owned());
            }
            if matches!(name.as_str(), "contains" | "mentions")
                && schema.relations().contains_key(name)
            {
                return Err(QueryError::AmbiguousRelationName { name: name.clone() });
            }
            Ok(name.clone())
        })
        .collect::<Result<_, _>>()?;
    let items = filtered_items(corpus, schema, &item_filters, None)?;
    let item_only = !filters.ids.is_empty()
        || !filters.flavours.is_empty()
        || !filters.fields.is_empty()
        || !filters.relations.is_empty();
    let paths = normalized_paths(&filters.paths)?;
    let terms = keyword_terms(query);
    let graph = corpus.discovery();
    let sources = corpus
        .documents()
        .iter()
        .map(|d| (d.path(), d.source()))
        .collect::<BTreeMap<_, _>>();
    let mut matches = Vec::new();
    for node in graph.nodes() {
        if !is_search_unit(node)
            || (!paths.is_empty() && !paths.iter().any(|p| node.source().path().starts_with(p)))
        {
            continue;
        }
        let source = sources[node.source().path()];
        let mut fields = Vec::new();
        match node.kind() {
            DiscoveryNodeKind::Item(item) => {
                if !items.iter().any(|selected| std::ptr::eq(*selected, item)) {
                    continue;
                }
                fields.push((item.id(), 3, false));
                fields.push((item.body(), 1, true));
                for entry in item.metadata() {
                    fields.push((entry.key(), 1, true));
                    fields.push((
                        entry.value(),
                        if entry.key() == "title" { 3 } else { 1 },
                        entry.key() != "mid",
                    ));
                }
                add_headings(item.body_blocks(), &mut fields);
            }
            _ if item_only => continue,
            DiscoveryNodeKind::Section { heading } => {
                fields.push((heading.heading_text().unwrap_or_default(), 3, true))
            }
            DiscoveryNodeKind::MarkdownBlock(block) => {
                let span = block.source().span();
                fields.push((&source[span.start_byte()..span.end_byte()], 1, true));
                add_headings(std::slice::from_ref(block), &mut fields);
            }
            DiscoveryNodeKind::Document(_) => unreachable!(),
        }
        if let Some(rank) = rank_fields(fields, &terms) {
            matches.push((rank, node));
        }
    }
    matches.sort_by(|(a_rank, a), (b_rank, b)| {
        b_rank
            .cmp(a_rank)
            .then_with(|| a.source().path().cmp(b.source().path()))
            .then_with(|| {
                a.source()
                    .span()
                    .start_byte()
                    .cmp(&b.source().span().start_byte())
            })
    });
    if filters.cursor.is_some() && (start == 0 || start >= matches.len()) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let mut page = SearchResult {
        format_version: 1,
        results: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    for (_, node) in matches.iter().skip(start).take(limit) {
        page.results.push(SearchHit {
            node: node.summary(),
            excerpt: excerpt(*node, sources[node.source().path()], &terms),
        });
        (page.has_more, page.next_cursor) =
            continuation(start, page.results.len(), matches.len(), &fingerprint);
        if serde_json::to_vec(&page)
            .map_err(|_| page_error("could not serialize search page"))?
            .len()
            > PAGE_BYTES
        {
            page.results.pop();
            if page.results.is_empty() {
                return Err(page_error(
                    "a search result cannot fit the 65536-byte page budget; shorten oversized identity/location fields in the source",
                ));
            }
            (page.has_more, page.next_cursor) =
                continuation(start, page.results.len(), matches.len(), &fingerprint);
            break;
        }
    }
    Ok(page)
}

fn is_search_unit(node: DiscoveryNode<'_, '_>) -> bool {
    if matches!(node.kind(), DiscoveryNodeKind::Document(_)) {
        return false;
    }
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if matches!(
            parent.kind(),
            DiscoveryNodeKind::Item(_) | DiscoveryNodeKind::MarkdownBlock(_)
        ) {
            return false;
        }
        ancestor = parent.parent();
    }
    true
}

fn add_headings<'a>(blocks: &'a [MarkdownBlock], fields: &mut Vec<(&'a str, usize, bool)>) {
    for block in blocks {
        if let MarkdownBlockKind::Heading { .. } = block.kind() {
            fields.push((block.heading_text().unwrap_or_default(), 3, true));
        }
        add_headings(block.children(), fields);
    }
}

fn matching_heading_offset(blocks: &[MarkdownBlock], terms: &BTreeSet<String>) -> Option<usize> {
    for block in blocks {
        if let Some(text) = block.heading_text()
            && let Some((start, _)) = matching_spans(text, terms, true).first()
            && let Some(offset) = block.heading_source_offset(*start)
        {
            return Some(offset);
        }
        if let Some(offset) = matching_heading_offset(block.children(), terms) {
            return Some(offset);
        }
    }
    None
}

fn excerpt(node: DiscoveryNode<'_, '_>, source: &str, terms: &BTreeSet<String>) -> SearchExcerpt {
    if let DiscoveryNodeKind::Item(item) = node.kind()
        && let Some(excerpt) = excerpts(source, item, terms).into_iter().next()
    {
        return excerpt;
    }
    let span = match node.kind() {
        DiscoveryNodeKind::Section { heading } => heading.source().span(),
        _ => node.source().span(),
    };
    let value = &source[span.start_byte()..span.end_byte()];
    let matched = if let Some((start, _)) = matching_spans(value, terms, true).first() {
        *start
    } else {
        // Ranking also searches decoded headings. Their words may not occur
        // literally in Markdown (entities or inline formatting). Map the
        // decoded word back to its original source before centering context.
        let blocks = match node.kind() {
            DiscoveryNodeKind::Item(item) => item.body_blocks(),
            DiscoveryNodeKind::MarkdownBlock(block) => std::slice::from_ref(block),
            DiscoveryNodeKind::Section { heading } => std::slice::from_ref(heading),
            DiscoveryNodeKind::Document(_) => &[],
        };
        matching_heading_offset(blocks, terms).map_or(0, |offset| offset - span.start_byte())
    };
    let start = value[..matched]
        .char_indices()
        .rev()
        .nth(59)
        .map_or(0, |(offset, _)| offset);
    let end = value[start..]
        .char_indices()
        .nth(240)
        .map_or(value.len(), |(offset, _)| start + offset);
    let start_byte = span.start_byte() + start;
    let end_byte = span.start_byte() + end;
    SearchExcerpt {
        text: source[start_byte..end_byte].to_owned(),
        start_byte,
        end_byte,
        start_line: line_at(source, start_byte),
        end_line: line_at(source, end_byte.saturating_sub(1).max(start_byte)),
        partial: start_byte > node.source().span().start_byte()
            || end_byte < node.source().span().end_byte(),
    }
}
