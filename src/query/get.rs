use super::{page::*, *};
use crate::{DiscoveryNodeKind, DiscoveryNodeSummary, MetadataEntry};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TextRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub total_bytes: usize,
    pub partial: bool,
}

impl TextRange {
    fn new(start: usize, end: usize, total: usize) -> Self {
        Self {
            start_byte: start,
            end_byte: end,
            total_bytes: total,
            partial: start != 0 || end != total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EntryRange {
    pub start_index: usize,
    pub end_index: usize,
    pub total: usize,
    pub partial: bool,
}

impl EntryRange {
    fn new(start: usize, end: usize, total: usize) -> Self {
        Self {
            start_index: start,
            end_index: end,
            total,
            partial: start != 0 || end != total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MetadataFragment {
    pub index: usize,
    pub key: String,
    pub value: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GetResult {
    pub format_version: u8,
    pub node: DiscoveryNodeSummary,
    pub content: String,
    pub content_range: TextRange,
    pub metadata: Vec<MetadataFragment>,
    pub metadata_range: EntryRange,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Position {
    content: usize,
    metadata: usize,
    value: usize,
}

impl Position {
    fn complete(self, item: &ReadContent<'_>) -> bool {
        self.content == item.content.len()
            && self.metadata == item.metadata.len()
            && self.value == 0
    }

    fn cursor(self, fingerprint: &str) -> String {
        format!(
            "g2-{fingerprint}-{:016x}-{:016x}-{:016x}",
            self.content, self.metadata, self.value
        )
    }

    fn read(
        cursor: Option<&str>,
        fingerprint: &str,
        item: &ReadContent<'_>,
    ) -> Result<Self, QueryError> {
        let Some(cursor) = cursor else {
            return Ok(Self::default());
        };
        let invalid = || {
            page_error(
                "invalid or stale get cursor; source or request changed; restart from the first page",
            )
        };
        if cursor.len() != 70 {
            return Err(invalid());
        }
        let mut parts = cursor.split('-');
        if parts.next() != Some("g2") || parts.next() != Some(fingerprint) {
            return Err(invalid());
        }
        let mut number = || {
            let part = parts.next().ok_or_else(invalid)?;
            if part.len() != 16 || !part.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(invalid());
            }
            usize::from_str_radix(part, 16).map_err(|_| invalid())
        };
        let position = Self {
            content: number()?,
            metadata: number()?,
            value: number()?,
        };
        if parts.next().is_some()
            || !item.content.is_char_boundary(position.content)
            || position.metadata > item.metadata.len()
            || (position.content < item.content.len()
                && (position.metadata != 0 || position.value != 0))
            || (position.metadata < item.metadata.len()
                && (!item.metadata[position.metadata]
                    .value()
                    .is_char_boundary(position.value)
                    || (position.value != 0
                        && position.value == item.metadata[position.metadata].value().len())))
            || (position.metadata == item.metadata.len() && position.value != 0)
            || position == Self::default()
            || position.complete(item)
        {
            return Err(invalid());
        }
        Ok(position)
    }
}

impl GetResult {
    fn update(
        &mut self,
        start: Position,
        next: Position,
        item: &ReadContent<'_>,
        fingerprint: &str,
    ) {
        self.content_range = TextRange::new(start.content, next.content, item.content.len());
        self.metadata_range = EntryRange::new(
            start.metadata,
            self.metadata
                .last()
                .map_or(start.metadata, |entry| entry.index + 1),
            item.metadata.len(),
        );
        self.metadata_range.partial |= self.metadata.iter().any(|entry| entry.range.partial);
        self.has_more = !next.complete(item);
        self.next_cursor = self.has_more.then(|| next.cursor(fingerprint));
    }

    fn fits(&self) -> Result<bool, QueryError> {
        Ok(serde_json::to_vec(self)
            .map_err(|_| page_error("could not serialize get page"))?
            .len()
            <= PAGE_BYTES)
    }
}

struct ReadContent<'a> {
    content: &'a str,
    metadata: &'a [MetadataEntry],
}

/// Read consecutive source content and metadata without expanding neighbours.
pub fn get(
    corpus: &Corpus,
    schema: &Schema,
    reference: &str,
    cursor: Option<&str>,
) -> Result<GetResult, QueryError> {
    let graph = corpus.discovery();
    let node = graph.resolve(reference)?;
    let item = match node.kind() {
        DiscoveryNodeKind::Item(item) => ReadContent {
            content: item.body(),
            metadata: item.metadata(),
        },
        _ => {
            let source = node.source();
            let document = corpus
                .documents()
                .iter()
                .find(|document| document.path() == source.path())
                .expect("discovery node belongs to a loaded document");
            ReadContent {
                content: &document.source()[source.span().start_byte()..source.span().end_byte()],
                metadata: &[],
            }
        }
    };
    let fingerprint = fingerprint(corpus, schema, &("discovery-get-v1", reference))?;
    let start = Position::read(cursor, &fingerprint, &item)?;
    let mut next = start;
    let mut result = GetResult {
        format_version: 1,
        node: node.summary(),
        content: String::new(),
        content_range: TextRange::new(start.content, start.content, item.content.len()),
        metadata: Vec::new(),
        metadata_range: EntryRange::new(start.metadata, start.metadata, item.metadata.len()),
        has_more: false,
        next_cursor: None,
    };
    result.update(start, next, &item, &fingerprint);
    if !result.fits()? {
        return Err(page_error(
            "node header cannot fit the 65536-byte get budget; shorten oversized identity/location fields in the source",
        ));
    }

    if next.content < item.content.len() {
        let end = fitting_end(item.content, next.content, |end| {
            result.content = item.content[start.content..end].to_owned();
            next.content = end;
            result.update(start, next, &item, &fingerprint);
            result.fits()
        })?
        .unwrap_or(start.content);
        next.content = end;
        result.content = item.content[start.content..end].to_owned();
    }
    if next.content == item.content.len() {
        while next.metadata < item.metadata.len() {
            let before = next;
            let entry = &item.metadata[next.metadata];
            result.metadata.push(MetadataFragment {
                index: next.metadata,
                key: entry.key().to_owned(),
                value: String::new(),
                range: TextRange::new(next.value, next.value, entry.value().len()),
            });
            let end = fitting_end(entry.value(), before.value, |end| {
                let fragment = result.metadata.last_mut().expect("inserted fragment");
                fragment.value = entry.value()[before.value..end].to_owned();
                fragment.range = TextRange::new(before.value, end, entry.value().len());
                next = if end == entry.value().len() {
                    Position {
                        metadata: before.metadata + 1,
                        value: 0,
                        ..before
                    }
                } else {
                    Position {
                        value: end,
                        ..before
                    }
                };
                result.update(start, next, &item, &fingerprint);
                result.fits()
            })?;
            if let Some(end) = end {
                let fragment = result.metadata.last_mut().expect("inserted fragment");
                fragment.value = entry.value()[before.value..end].to_owned();
                fragment.range = TextRange::new(before.value, end, entry.value().len());
                if end == entry.value().len() {
                    next = Position {
                        metadata: before.metadata + 1,
                        value: 0,
                        ..before
                    };
                } else {
                    next = Position {
                        value: end,
                        ..before
                    };
                    break;
                }
            } else {
                result.metadata.pop();
                next = before;
                break;
            }
        }
    }
    result.update(start, next, &item, &fingerprint);
    if next == start && result.has_more {
        return Err(page_error(
            "next content or metadata fragment cannot fit the 65536-byte get budget; shorten oversized identity/location fields or metadata keys in the source",
        ));
    }
    Ok(result)
}

// Test the whole remaining value first: finishing the last field can remove the
// cursor overhead. Otherwise find a fitting prefix on UTF-8 scalar boundaries.
fn fitting_end(
    text: &str,
    start: usize,
    mut fits: impl FnMut(usize) -> Result<bool, QueryError>,
) -> Result<Option<usize>, QueryError> {
    let mut high = text.floor_char_boundary(start.saturating_add(PAGE_BYTES).min(text.len()));
    if fits(high)? {
        return Ok(Some(high));
    }
    let mut low = start;
    while low < high {
        let middle = text.ceil_char_boundary(low + (high - low).div_ceil(2));
        if fits(middle)? {
            low = middle;
        } else {
            high = text.floor_char_boundary(middle - 1);
        }
    }
    Ok((low > start).then_some(low))
}
