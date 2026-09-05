use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;

const PAGE_BYTES: usize = 65_536;
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
    let limit = filters.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(page_error("page limit must be 1 through 100"));
    }
    let fingerprint = fingerprint(corpus, schema, filters, query, limit)?;
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
        if let Some((end, _)) = summary.title.char_indices().nth(TITLE_CHARS) {
            summary.title.truncate(end);
            summary.title_truncated = true;
        }
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

fn page_error(message: &str) -> QueryError {
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
    let next = start + page.items.len();
    page.has_more = next < total;
    page.next_cursor = page
        .has_more
        .then(|| format!("1-{fingerprint}-{next:016x}"));
}

fn fingerprint(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
    query: Option<&str>,
    limit: usize,
) -> Result<String, QueryError> {
    // Deterministic across processes of this build. This is change detection,
    // not an authentication token; cursors confer no access to stored state.
    let mut hash = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hash);
    let request = (
        &filters.flavours,
        &filters.fields,
        &filters.relations,
        &filters.paths,
        &filters.ids,
        filters.excerpts,
        query,
        limit,
        schema,
    );
    serde_json::to_vec(&request)
        .map_err(|_| page_error("could not fingerprint search/list request"))?
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
        .filter(|(_, word)| terms.contains(*word))
        .map(|(start, word)| {
            let first = mapping.partition_point(|entry| entry.1 <= start);
            let last = mapping.partition_point(|entry| entry.0 < start + word.len()) - 1;
            (mapping[first].2, mapping[last].3)
        })
        .collect()
}
