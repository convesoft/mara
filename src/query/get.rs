use super::{page::*, *};

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
pub struct ItemGetResult {
    pub summary: ItemSummary,
    pub source: ItemSource,
    pub body: String,
    pub body_range: TextRange,
    pub metadata: Vec<MetadataFragment>,
    pub metadata_range: EntryRange,
    pub outgoing_relations: Vec<RelationSummary>,
    pub outgoing_relations_range: EntryRange,
    pub incoming_relations: Vec<RelationSummary>,
    pub incoming_relations_range: EntryRange,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Position {
    body: usize,
    metadata: usize,
    value: usize,
    relations: usize,
}

impl Position {
    fn complete(self, item: &ResolvedItem) -> bool {
        self.body == item.body.len()
            && self.metadata == item.metadata.len()
            && self.value == 0
            && self.relations == item.outgoing_relations.len() + item.incoming_relations.len()
    }

    fn cursor(self, fingerprint: &str) -> String {
        format!(
            "g1-{fingerprint}-{:016x}-{:016x}-{:016x}-{:016x}",
            self.body, self.metadata, self.value, self.relations
        )
    }

    fn read(
        cursor: Option<&str>,
        fingerprint: &str,
        item: &ResolvedItem,
    ) -> Result<Self, QueryError> {
        let Some(cursor) = cursor else {
            return Ok(Self::default());
        };
        let invalid = || {
            page_error(
                "invalid or stale get cursor; source or request changed; restart from the first page",
            )
        };
        if cursor.len() != 87 {
            return Err(invalid());
        }
        let mut parts = cursor.split('-');
        if parts.next() != Some("g1") || parts.next() != Some(fingerprint) {
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
            body: number()?,
            metadata: number()?,
            value: number()?,
            relations: number()?,
        };
        if parts.next().is_some()
            || !item.body.is_char_boundary(position.body)
            || position.metadata > item.metadata.len()
            || position.relations > item.outgoing_relations.len() + item.incoming_relations.len()
            || (position.body < item.body.len()
                && (position.metadata != 0 || position.value != 0 || position.relations != 0))
            || (position.metadata < item.metadata.len()
                && (position.relations != 0
                    || !item.metadata[position.metadata]
                        .value
                        .is_char_boundary(position.value)
                    || (position.value != 0
                        && position.value == item.metadata[position.metadata].value.len())))
            || (position.metadata == item.metadata.len() && position.value != 0)
            || position == Self::default()
            || position.complete(item)
        {
            return Err(invalid());
        }
        Ok(position)
    }
}

impl ItemGetResult {
    fn update(&mut self, start: Position, next: Position, item: &ResolvedItem, fingerprint: &str) {
        self.body_range = TextRange::new(start.body, next.body, item.body.len());
        self.metadata_range = EntryRange::new(
            start.metadata,
            self.metadata
                .last()
                .map_or(start.metadata, |entry| entry.index + 1),
            item.metadata.len(),
        );
        self.metadata_range.partial |= self.metadata.iter().any(|entry| entry.range.partial);
        let outgoing = item.outgoing_relations.len();
        self.outgoing_relations_range = EntryRange::new(
            start.relations.min(outgoing),
            next.relations.min(outgoing),
            outgoing,
        );
        self.incoming_relations_range = EntryRange::new(
            start.relations.saturating_sub(outgoing),
            next.relations.saturating_sub(outgoing),
            item.incoming_relations.len(),
        );
        self.has_more = !next.complete(item);
        self.next_cursor = self.has_more.then(|| next.cursor(fingerprint));
    }

    fn fits(&self) -> Result<bool, QueryError> {
        Ok(serde_json::to_vec(self)
            .map_err(|_| page_error("could not serialize item get page"))?
            .len()
            <= PAGE_BYTES)
    }
}

/// Bounded transport retrieval. Mutations continue to use the complete internal
/// `get_item` model so a retrieval page can never become replacement source data.
pub fn get_item_page(
    corpus: &Corpus,
    schema: &Schema,
    id: &str,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Result<ItemGetResult, QueryError> {
    let limit = page_limit(limit)?;
    let fingerprint = fingerprint(corpus, schema, &("get", id, limit))?;
    let item = get_item(corpus, id)?;
    let start = Position::read(cursor, &fingerprint, &item)?;
    let mut next = start;
    let mut summary = item.summary.clone();
    truncate_title(&mut summary);
    let mut result = ItemGetResult {
        summary,
        source: item.source.clone(),
        body: String::new(),
        body_range: TextRange::new(start.body, start.body, item.body.len()),
        metadata: Vec::new(),
        metadata_range: EntryRange::new(start.metadata, start.metadata, item.metadata.len()),
        outgoing_relations: Vec::new(),
        outgoing_relations_range: EntryRange::new(0, 0, item.outgoing_relations.len()),
        incoming_relations: Vec::new(),
        incoming_relations_range: EntryRange::new(0, 0, item.incoming_relations.len()),
        has_more: false,
        next_cursor: None,
    };
    result.update(start, next, &item, &fingerprint);
    if !result.fits()? {
        return Err(page_error(
            "item header cannot fit the 65536-byte get budget; shorten oversized identity/location fields in the source",
        ));
    }

    if next.body < item.body.len() {
        let end = fitting_end(&item.body, next.body, |end| {
            result.body = item.body[start.body..end].to_owned();
            next.body = end;
            result.update(start, next, &item, &fingerprint);
            result.fits()
        })?
        .unwrap_or(start.body);
        next.body = end;
        result.body = item.body[start.body..end].to_owned();
    }
    if next.body == item.body.len() {
        while next.metadata < item.metadata.len() {
            let before = next;
            let entry = &item.metadata[next.metadata];
            result.metadata.push(MetadataFragment {
                index: next.metadata,
                key: entry.key.clone(),
                value: String::new(),
                range: TextRange::new(next.value, next.value, entry.value.len()),
            });
            let end = fitting_end(&entry.value, before.value, |end| {
                let fragment = result.metadata.last_mut().expect("inserted fragment");
                fragment.value = entry.value[before.value..end].to_owned();
                fragment.range = TextRange::new(before.value, end, entry.value.len());
                next = if end == entry.value.len() {
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
                fragment.value = entry.value[before.value..end].to_owned();
                fragment.range = TextRange::new(before.value, end, entry.value.len());
                if end == entry.value.len() {
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
    if next.metadata == item.metadata.len() && next.body == item.body.len() {
        for relation in item
            .outgoing_relations
            .iter()
            .chain(&item.incoming_relations)
            .skip(next.relations)
            .take(limit)
        {
            let outgoing = next.relations < item.outgoing_relations.len();
            let mut relation = relation.clone();
            truncate_title(&mut relation.item);
            if outgoing {
                result.outgoing_relations.push(relation);
            } else {
                result.incoming_relations.push(relation);
            }
            next.relations += 1;
            result.update(start, next, &item, &fingerprint);
            if !result.fits()? {
                next.relations -= 1;
                if outgoing {
                    result.outgoing_relations.pop();
                } else {
                    result.incoming_relations.pop();
                }
                break;
            }
        }
    }
    result.update(start, next, &item, &fingerprint);
    if next == start && result.has_more {
        return Err(page_error(
            "next fragment or relation cannot fit the 65536-byte get budget; shorten oversized identity/location fields, metadata keys, or relation names in the source",
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
