use std::hash::{DefaultHasher, Hash, Hasher};

use super::*;

pub(crate) const PAGE_BYTES: usize = 65_536;
const TITLE_CHARS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ItemCollectionResult {
    pub items: Vec<ItemSummary>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

pub(crate) fn filtered_page(
    corpus: &Corpus,
    schema: &Schema,
    filters: &ItemFilters,
) -> Result<ItemCollectionResult, QueryError> {
    let limit = page_limit(filters.limit)?;
    let request = (
        "items-subtrees-v1",
        &filters.flavours,
        &filters.fields,
        &filters.relations,
        &filters.paths,
        &Vec::<String>::new(),
        false,
        None::<&str>,
        limit,
    );
    let fingerprint = fingerprint(corpus, schema, &request)?;
    let start = cursor_position(filters.cursor.as_deref(), &fingerprint)?;
    let matches = filtered_items(corpus, schema, filters)?;
    if filters.cursor.is_some() && (start == 0 || start >= matches.len()) {
        return Err(page_error(
            "invalid continuation position; restart from the first page",
        ));
    }
    let mut page = ItemCollectionResult {
        items: Vec::new(),
        has_more: false,
        next_cursor: None,
    };
    for item in matches.iter().skip(start).take(limit) {
        let mut summary = ItemSummary::from(*item);
        truncate_title(&mut summary);
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

pub(crate) fn page_limit(limit: Option<usize>) -> Result<usize, QueryError> {
    let limit = limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(page_error("page limit must be 1 through 100"));
    }
    Ok(limit)
}

pub(crate) fn truncate_title(summary: &mut ItemSummary) {
    if let Some((end, _)) = summary.title.char_indices().nth(TITLE_CHARS) {
        summary.title.truncate(end);
        summary.title_truncated = true;
    }
}

pub(crate) fn page_error(message: &str) -> QueryError {
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

pub(crate) fn continuation(
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

pub(crate) fn fingerprint(
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
    for file in corpus.code().files() {
        file.path.hash(&mut hash);
        file.source.hash(&mut hash);
    }
    // Explicit file-only endpoints need no adapter, so they are absent from
    // the code index. Their existence and contents can change related results.
    for path in corpus.file_only_code_paths() {
        path.hash(&mut hash);
        corpus.code().file_only_bytes(&path).hash(&mut hash);
    }
    for path in corpus.code().assets() {
        path.hash(&mut hash);
        std::fs::read(corpus.code().root().join(path))
            .unwrap_or_default()
            .hash(&mut hash);
    }
    Ok(format!("{:016x}", hash.finish()))
}

pub(crate) fn cursor_position(
    cursor: Option<&str>,
    fingerprint: &str,
) -> Result<usize, QueryError> {
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
