use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;

pub(super) const PAGE_BYTES: usize = 65_536;
const TITLE_CHARS: usize = 256;
const EXCERPT_CHARS: usize = 240;
const EXCERPT_COUNT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ItemCollectionResult {
    pub items: Vec<ItemSummary>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RelatedItemsResult {
    pub items: Vec<RelatedItem>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SearchExcerpt {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub partial: bool,
}

pub(super) fn filtered_page(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
    query: Option<&str>,
) -> Result<ItemCollectionResult, QueryError> {
    let limit = page_limit(filters.limit)?;
    let request = (
        "items",
        &filters.flavours,
        &filters.fields,
        &filters.relations,
        &filters.paths,
        &filters.ids,
        filters.excerpts,
        query,
        limit,
    );
    let fingerprint = fingerprint(corpus, schema, &request)?;
    let start = cursor_position(filters.cursor.as_deref(), &fingerprint)?;
    let matches = filtered_items(corpus, schema, filters, query)?;
    if filters.cursor.is_some() && (start == 0 || start >= matches.len()) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let terms = query.map(keyword_terms).unwrap_or_default();
    let mut page = ItemCollectionResult {
        items: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    for item in matches.iter().skip(start).take(limit) {
        let mut summary = ItemSummary::from(*item);
        truncate_title(&mut summary);
        if filters.excerpts {
            let document = corpus
                .documents()
                .iter()
                .find(|document| document.path() == item.source().path())
                .expect("matched item belongs to a corpus document");
            summary.excerpts = Some(excerpts(document.source(), item, &terms));
        }
        page.items.push(summary);
        set_continuation(&mut page, start, matches.len(), &fingerprint);
        if serde_json::to_vec(&page)
            .map_err(|_| page_error("could not serialize search/list page"))?
            .len()
            > PAGE_BYTES
        {
            page.items.pop();
            if page.items.is_empty() {
                return Err(page_error(
                    "an item cannot fit the 65536-byte page budget; shorten oversized identity/location fields in the source",
                ));
            }
            set_continuation(&mut page, start, matches.len(), &fingerprint);
            break;
        }
    }
    Ok(page)
}

pub(super) fn related_page(
    corpus: &Corpus,
    schema: &Schema,
    id: &str,
    filters: &RelatedFilters,
) -> Result<RelatedItemsResult, QueryError> {
    let limit = page_limit(filters.limit)?;
    let request = (
        "related",
        id,
        filters.direction,
        &filters.relations,
        &filters.flavours,
        limit,
    );
    let fingerprint = fingerprint(corpus, schema, &request)?;
    let start = cursor_position(filters.cursor.as_deref(), &fingerprint)?;
    let matches = related_matches(corpus, schema, id, filters)?;
    let total = matches.len();
    if filters.cursor.is_some() && (start == 0 || start >= total) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let mut page = RelatedItemsResult {
        items: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    for mut entry in matches.into_iter().skip(start).take(limit) {
        truncate_title(&mut entry.item);
        page.items.push(entry);
        (page.has_more, page.next_cursor) =
            continuation(start, page.items.len(), total, &fingerprint);
        if serde_json::to_vec(&page)
            .map_err(|_| page_error("could not serialize related page"))?
            .len()
            > PAGE_BYTES
        {
            page.items.pop();
            if page.items.is_empty() {
                return Err(page_error(
                    "a relation entry cannot fit the 65536-byte page budget; shorten oversized identity/location fields or relation names in the source",
                ));
            }
            (page.has_more, page.next_cursor) =
                continuation(start, page.items.len(), total, &fingerprint);
            break;
        }
    }
    Ok(page)
}

pub(super) fn page_limit(limit: Option<usize>) -> Result<usize, QueryError> {
    let limit = limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(page_error("page limit must be 1 through 100"));
    }
    Ok(limit)
}

pub(super) fn truncate_title(summary: &mut ItemSummary) {
    if let Some((end, _)) = summary.title.char_indices().nth(TITLE_CHARS) {
        summary.title.truncate(end);
        summary.title_truncated = true;
    }
}

pub(super) fn page_error(message: &str) -> QueryError {
    QueryError::InvalidPage {
        message: message.to_owned(),
    }
}

fn set_continuation(
    page: &mut ItemCollectionResult,
    start: usize,
    total: usize,
    fingerprint: &str,
) {
    (page.has_more, page.next_cursor) = continuation(start, page.items.len(), total, fingerprint);
}

fn continuation(
    start: usize,
    count: usize,
    total: usize,
    fingerprint: &str,
) -> (bool, Option<String>) {
    let next = start + count;
    let has_more = next < total;
    (
        has_more,
        has_more.then(|| format!("1-{fingerprint}-{next:016x}")),
    )
}

pub(super) fn fingerprint(
    corpus: &Corpus,
    schema: &Schema,
    request: &impl Serialize,
) -> Result<String, QueryError> {
    // Deterministic across processes of this build. This is change detection,
    // not an authentication token; cursors confer no access to stored state.
    let mut hash = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hash);
    serde_json::to_vec(&(request, schema))
        .map_err(|_| page_error("could not fingerprint retrieval request"))?
        .hash(&mut hash);
    for document in corpus.documents() {
        document.path().hash(&mut hash);
        document.source().hash(&mut hash);
    }
    Ok(format!("{:016x}", hash.finish()))
}

fn cursor_position(cursor: Option<&str>, fingerprint: &str) -> Result<usize, QueryError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let invalid = || {
        page_error(
            "invalid or stale cursor; source or request changed; restart from the first page",
        )
    };
    if cursor.len() != 35 {
        return Err(invalid());
    }
    let mut parts = cursor.split('-');
    if parts.next() != Some("1") || parts.next() != Some(fingerprint) {
        return Err(invalid());
    }
    let position = parts.next().ok_or_else(invalid)?;
    if position.len() != 16
        || !position.bytes().all(|b| b.is_ascii_hexdigit())
        || parts.next().is_some()
    {
        return Err(invalid());
    }
    usize::from_str_radix(position, 16).map_err(|_| invalid())
}

fn excerpts(source: &str, item: &Item, terms: &BTreeSet<String>) -> Vec<SearchExcerpt> {
    if terms.is_empty() {
        return Vec::new();
    }
    let mut values = Vec::new();
    let opener_start = item.source().span().start_byte();
    let opener = &source[opener_start..item.source().span().end_byte()];
    // The ID is the last token on the opening line. Metadata values are trimmed
    // scalars; locate their original bytes rather than reconstructing a line.
    let id_offset = opener
        .lines()
        .next()
        .expect("item opener")
        .rfind(item.id())
        .expect("item ID in opener");
    values.push((opener_start + id_offset, item.id()));
    for entry in item.metadata() {
        let start = entry.source().span().start_byte();
        let line = &source[start..entry.source().span().end_byte()];
        values.push((start + 1, entry.key()));
        let prefix = entry.key().len() + 2;
        let after_key = &line[prefix..];
        let whitespace = after_key.len() - after_key.trim_start().len();
        values.push((start + prefix + whitespace, entry.value()));
    }
    values.push((item.body_source().span().start_byte(), item.body()));
    let mut fragments = Vec::new();
    for (base, value) in values {
        let mut covered_until = 0;
        for (start, _) in matching_spans(value, terms) {
            if start < covered_until {
                continue;
            }
            let context_start = value[..start]
                .char_indices()
                .rev()
                .nth(59)
                .map_or(0, |(offset, _)| offset);
            let context_start = context_start.max(covered_until);
            let end = value[context_start..]
                .char_indices()
                .nth(EXCERPT_CHARS)
                .map_or(value.len(), |(offset, _)| context_start + offset);
            covered_until = end;
            let start_byte = base + context_start;
            let end_byte = base + end;
            fragments.push(SearchExcerpt {
                text: source[start_byte..end_byte].to_owned(),
                start_byte,
                end_byte,
                start_line: line_at(source, start_byte),
                end_line: line_at(source, end_byte - 1),
                partial: true,
            });
            if fragments.len() == EXCERPT_COUNT {
                return fragments;
            }
        }
    }
    fragments
}

fn line_at(source: &str, byte: usize) -> usize {
    source.as_bytes()[..byte]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

fn matching_spans(value: &str, terms: &BTreeSet<String>) -> Vec<(usize, usize)> {
    // Normalize whole grapheme clusters so composition and case-fold expansion
    // retain a mapping to their original source bytes (e.g. cafe + accent, ß).
    let mut canonical = String::new();
    let mut mapping = Vec::new();
    for (start, grapheme) in value.grapheme_indices(true) {
        let normalized_start = canonical.len();
        canonical.push_str(&canonical_text(grapheme));
        mapping.push((
            normalized_start,
            canonical.len(),
            start,
            start + grapheme.len(),
        ));
    }
    canonical
        .unicode_word_indices()
        .filter(|(_, word)| terms.iter().any(|term| word_matches(term, word)))
        .map(|(start, word)| {
            let first = mapping.partition_point(|entry| entry.1 <= start);
            let last = mapping.partition_point(|entry| entry.0 < start + word.len()) - 1;
            (mapping[first].2, mapping[last].3)
        })
        .collect()
}
