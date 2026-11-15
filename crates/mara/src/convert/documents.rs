//! The documents of a 0.3.0 project in the item format (DES-CONVERSION-MAPPING in
//! `docs/migration.mara.md`): each `:::mara` block becomes an item written by the item writer,
//! each relation one row on its source item, each `[[…]]` mention a mention link. Narrative and
//! items already in the item format, as a stopped conversion leaves them, keep their bytes.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;

use mara_engine::diagnostic::Diagnostic;
use mara_engine::document::{self, Document, is_mid};
use mara_engine::work::WorkCounts;
use mara_engine::write::{self, label};

use super::report::Entry;
use super::scan::{self, Block, Scan, Token};
use super::schema::{Role, STATUS, Vocabulary};

/// A document of the project as read.
#[derive(Debug)]
pub struct Source {
    pub path: String,
    pub text: String,
    /// What the item format reads in it: the items of a document already converted.
    pub parsed: Document,
    /// Its `:::mara` blocks.
    pub scan: Scan,
}

impl Source {
    /// Whether the document is already in the item format, as a stopped conversion leaves it:
    /// it holds items and no block. A rerun keeps it as it is, but for the rows that other
    /// documents' assertions give its items, and reports nothing of it: its lines are no longer
    /// those of the 0.3.0 document that the report names.
    fn converted(&self) -> bool {
        self.scan.blocks.is_empty() && !self.parsed.items.is_empty()
    }
}

/// Why the documents cannot be converted.
#[derive(Debug)]
pub enum Refusal {
    /// Items without a status, and no status named for the run.
    MissingStatus(Vec<Diagnostic>),
    /// A converted document that would not read back as written.
    Unreadable(Box<Diagnostic>),
}

/// Where an item is: a block of a source, or an item of a source's [`Source::parsed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Holder {
    Block(usize, usize),
    Item(usize, usize),
}

/// An item that a relation or mention can name.
struct Target<'s> {
    id: &'s str,
    mid: String,
    source: usize,
    holder: Holder,
}

/// A row: the schema name of its key and its cell.
type Row = (String, String);

struct Conversion<'s> {
    sources: &'s [Source],
    vocabulary: &'s Vocabulary,
    targets: Vec<Target<'s>>,
    holders: HashMap<Holder, usize>,
    by_id: HashMap<&'s str, Vec<usize>>,
    by_mid: HashMap<String, Vec<usize>>,
    /// Each relation written, as (holder MID, relation, target MID).
    edges: HashSet<(String, String, String)>,
    /// Rows that other items' assertions give an item, in the order found.
    incoming: HashMap<Holder, Vec<Row>>,
    entries: Vec<Entry>,
}

/// Converts `sources`, in path order. A block without a MID takes the MID that an entry of
/// `recorded`, the report file of an earlier run, gave it, or else the next of `mids`; an item
/// without a status gets `status`, a schema name.
pub fn convert(
    sources: &[Source],
    vocabulary: &Vocabulary,
    status: Option<&str>,
    mids: Vec<String>,
    recorded: &[Entry],
    work: &mut WorkCounts,
) -> Result<(Vec<Option<String>>, Vec<Entry>), Refusal> {
    let mut conversion = Conversion::new(sources, vocabulary, mids, recorded);
    let mut missing = Vec::new();
    let mut items = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let mut blocks = Vec::with_capacity(source.scan.blocks.len());
        for (number, block) in source.scan.blocks.iter().enumerate() {
            let item = conversion.block(index, number, block);
            if !item.status && status.is_none() {
                let message = format!(
                    "item `{}` has no status: name the status such items get with `--status`",
                    block.id
                );
                missing.push(Diagnostic::error(
                    "missing-key",
                    &source.path,
                    block.line,
                    message,
                ));
            }
            blocks.push(item);
        }
        for line in source.scan.malformed.iter().filter(|_| !source.converted()) {
            conversion.entry(
                index,
                *line,
                "this line starts with `:::mara` but is no opening line `:::mara <flavour> <id>`, \
                 so its block stays as text"
                    .to_owned(),
            );
        }
        items.push(blocks);
    }
    if !missing.is_empty() {
        return Err(Refusal::MissingStatus(missing));
    }
    let mut contents = Vec::with_capacity(sources.len());
    for (index, blocks) in items.into_iter().enumerate() {
        contents.push(conversion.render(index, blocks, status, work)?);
    }
    let mut entries = conversion.entries;
    entries.sort();
    entries.dedup();
    Ok((contents, entries))
}

/// A block converted, before it is written.
struct Item {
    mid: String,
    title: String,
    /// Its rows after `Type` and `ID`, in order.
    rows: Vec<Row>,
    /// Whether a row holds its status.
    status: bool,
    body: String,
}

impl<'s> Conversion<'s> {
    fn new(
        sources: &'s [Source],
        vocabulary: &'s Vocabulary,
        mids: Vec<String>,
        recorded: &[Entry],
    ) -> Self {
        let mut mids = mids.into_iter();
        // The MIDs an earlier run gave blocks without one, by document and ID, so that a rerun
        // gives each the same and links to it keep resolving (ADR-CONVERSION-RERUN).
        let mut given: HashMap<(&str, &str), VecDeque<&str>> = HashMap::new();
        for entry in recorded {
            if let Some((id, mid)) = given_mid(&entry.message) {
                let key = (entry.path.as_str(), id);
                given.entry(key).or_default().push_back(mid);
            }
        }
        let mut conversion = Conversion {
            sources,
            vocabulary,
            targets: Vec::new(),
            holders: HashMap::new(),
            by_id: HashMap::new(),
            by_mid: HashMap::new(),
            edges: HashSet::new(),
            incoming: HashMap::new(),
            entries: Vec::new(),
        };
        for (index, source) in sources.iter().enumerate() {
            for (number, block) in source.scan.blocks.iter().enumerate() {
                let mid = match block.metadata.iter().find(|meta| meta.key == "mid") {
                    Some(meta) if is_mid(&meta.value) => meta.value.clone(),
                    found => {
                        let key = (source.path.as_str(), block.id.as_str());
                        let mid = match given.get_mut(&key).and_then(VecDeque::pop_front) {
                            Some(mid) => mid.to_owned(),
                            None => mids.next().expect("a new MID for each block without one"),
                        };
                        let had = match found {
                            Some(meta) => format!("has `{}`, which is no MID", meta.value),
                            None => "has no MID".to_owned(),
                        };
                        let message = format!(
                            "item `{}` {had}: it gets the MID `{mid}`{GIVEN_END}",
                            block.id
                        );
                        conversion.entry(index, block.line, message);
                        mid
                    }
                };
                conversion.add(&block.id, mid, index, Holder::Block(index, number));
            }
            for (number, item) in source.parsed.items.iter().enumerate() {
                let Some(id) = item.value("id") else { continue };
                for row in item.rows.iter().filter(|row| row.link.is_some()) {
                    let fragment = row.link.as_ref().and_then(|link| link.fragment.clone());
                    if let Some(target) = fragment {
                        let edge = (item.mid.clone(), row.name.clone(), target);
                        conversion.edges.insert(edge);
                    }
                }
                conversion.add(id, item.mid.clone(), index, Holder::Item(index, number));
            }
        }
        conversion
    }

    fn add(&mut self, id: &'s str, mid: String, source: usize, holder: Holder) {
        let at = self.targets.len();
        self.holders.insert(holder, at);
        self.by_id.entry(id).or_default().push(at);
        self.by_mid.entry(mid.clone()).or_default().push(at);
        self.targets.push(Target {
            id,
            mid,
            source,
            holder,
        });
    }

    fn entry(&mut self, source: usize, line: usize, message: String) {
        self.entries
            .push(Entry::new(&self.sources[source].path, line, message));
    }

    /// The one item that the ID or MID `handle` names.
    fn resolve(&self, handle: &str) -> Option<usize> {
        let found = match is_mid(handle) {
            true => self.by_mid.get(handle),
            false => self.by_id.get(handle),
        };
        match found.map(Vec::as_slice) {
            Some([one]) => Some(*one),
            _ => None,
        }
    }

    /// The link from the document of `from` to the item `to`.
    fn link(&self, from: usize, to: usize) -> String {
        let target = &self.targets[to];
        write::link(
            target.id,
            &target.mid,
            &self.sources[from].path,
            &self.sources[target.source].path,
        )
    }

    /// Records the relation `relation` from the item `source` to the item `target`, once: the
    /// row for the source item, or `None` when it is already written.
    fn edge(&mut self, source: usize, relation: &str, target: usize) -> Option<Row> {
        let key = (
            self.targets[source].mid.clone(),
            relation.to_owned(),
            self.targets[target].mid.clone(),
        );
        if !self.edges.insert(key) {
            return None;
        }
        let from = self.targets[source].source;
        Some((relation.to_owned(), self.link(from, target)))
    }

    /// Gives the item `holder` a row from another item's assertion.
    fn incoming(&mut self, holder: usize, row: Row) {
        let holder = self.targets[holder].holder;
        self.incoming.entry(holder).or_default().push(row);
    }

    /// The metadata rows and body of a block, and the rows its assertions give other items.
    fn block(&mut self, source: usize, number: usize, block: &Block) -> Item {
        let me = self.holders[&Holder::Block(source, number)];
        let mid = self.targets[me].mid.clone();
        let mut rows = Vec::new();
        let (mut title, mut seen_mid, mut status) = (None, false, false);
        for meta in &block.metadata {
            let (key, value) = (meta.key.as_str(), meta.value.as_str());
            match key {
                "mid" if !seen_mid => {
                    seen_mid = true;
                    continue;
                }
                "title" if title.is_none() && !value.is_empty() => {
                    title = Some(value.to_owned());
                    continue;
                }
                _ => {}
            }
            let shown = format!("`:{key}: {value}`");
            if value.is_empty() {
                let message = format!(
                    "`:{key}:` of item `{}` is left out: it has no value",
                    block.id
                );
                self.entry(source, meta.line, message);
                continue;
            }
            if key == STATUS {
                status = true;
                let cell = match self.vocabulary.status(value) {
                    Some(name) => label(name),
                    None => cell(value),
                };
                rows.push((STATUS.to_owned(), cell));
                continue;
            }
            // A field the flavour declares is no relation, whatever other flavours declare.
            let role = match self.vocabulary.field(&block.flavour, key) {
                Some(_) => Role::None,
                None => self.vocabulary.role(key),
            };
            match role {
                Role::Forward(relation) => {
                    if is_address(value) {
                        self.entry(source, meta.line, not_an_item(&shown));
                    } else if let Some(target) = self.resolve(value) {
                        rows.extend(self.edge(me, &relation.name, target));
                    } else {
                        rows.push((key.to_owned(), cell(value)));
                    }
                }
                Role::Inverse(relation) => {
                    if is_address(value) {
                        self.entry(source, meta.line, not_an_item(&shown));
                    } else if let Some(target) = self.resolve(value) {
                        if let Some(row) = self.edge(target, &relation.name, me) {
                            self.incoming(target, row);
                        }
                    } else {
                        let message = format!(
                            "{shown} is left out: `{value}` names no item, which would hold the \
                             row of relation `{}`",
                            relation.name
                        );
                        self.entry(source, meta.line, message);
                    }
                }
                Role::Dropped(why) => {
                    let message = format!("{shown} is left out: relation `{key}` {why}");
                    self.entry(source, meta.line, message);
                }
                Role::None => {
                    let named = self
                        .vocabulary
                        .field(&block.flavour, key)
                        .and_then(|field| field.values.as_ref())
                        .and_then(|values| values.iter().find(|(written, _)| written == value))
                        .map(|(_, name)| label(name));
                    rows.push((key.to_owned(), named.unwrap_or_else(|| cell(value))));
                }
            }
        }
        let title = title.unwrap_or_else(|| {
            let message = format!("item `{}` has no title: its heading is its ID", block.id);
            self.entry(source, block.line, message);
            block.id.clone()
        });
        let text = &self.sources[source].text;
        let body = self.body(source, me, &text[block.body.clone()], block);
        if body.trim().is_empty() {
            let message = format!(
                "item `{}` has an empty body, which the item format does not allow: complete it",
                block.id
            );
            self.entry(source, block.line, message);
        }
        Item {
            mid,
            title,
            rows,
            status,
            body,
        }
    }

    /// The body of the item `me` from the block `block`, whose body text is `text`: mentions as
    /// links, thematic breaks as `<hr>`, setext headings as ATX headings, without the blank lines
    /// around it. Inline relations give rows.
    fn body(&mut self, source: usize, me: usize, text: &str, block: &Block) -> String {
        let markdown = scan::markdown(text);
        let first_line = block.line + 1 + block.metadata.len();
        let line_at = |offset: usize| first_line + text[..offset].matches('\n').count();
        let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
        for token in scan::tokens(text, &markdown.raw) {
            let line = line_at(token.range.start);
            if let Some(link) = self.token(source, Some(me), &token, line) {
                replacements.push((token.range.clone(), link));
            }
        }
        for range in &markdown.rules {
            if starts_line(text, range.start) {
                let rule = &text[range.clone()];
                let marks = rule.trim_end_matches(['\n', '\r']);
                let mut new = format!("<hr>{}", &rule[marks.len()..]);
                let next = &text[range.end..];
                if !next.is_empty() && !next.lines().next().unwrap_or_default().trim().is_empty() {
                    new.push('\n');
                }
                replacements.push((range.clone(), new));
            }
        }
        for (range, level) in &markdown.setext {
            if !starts_line(text, range.start) {
                continue;
            }
            let heading = &text[range.clone()];
            let content_end = heading.trim_end_matches(['\n', '\r']).len();
            let underline = heading[..content_end].rfind(['\n', '\r']).unwrap_or(0);
            let inner: Vec<(Range<usize>, String)> = replacements
                .iter()
                .filter(|(at, _)| range.start <= at.start && at.end <= range.start + underline)
                .map(|(at, new)| (at.start - range.start..at.end - range.start, new.clone()))
                .collect();
            replacements.retain(|(at, _)| !(range.start <= at.start && at.end <= range.end));
            let content = splice(&heading[..underline], &inner);
            let words: Vec<&str> = content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect();
            let ending = &heading[content_end..];
            let new = format!("{} {}{ending}", "#".repeat(*level), words.join(" "));
            replacements.push((range.clone(), new));
        }
        replacements.sort_by_key(|(range, _)| range.start);
        let converted = splice(text, &replacements);
        trim_blank_lines(&converted).to_owned()
    }

    /// What the token `token` at `line` of the source `source` becomes: a mention link, or
    /// `None` when it stays as text. In the item `me`, a typed token gives its relation's row;
    /// outside an item it is only a mention.
    fn token(
        &mut self,
        source: usize,
        me: Option<usize>,
        token: &Token,
        line: usize,
    ) -> Option<String> {
        let shown = format!("`[[{}]]`", token.inner);
        let Some(name) = token.relation else {
            return match self.resolve(token.target) {
                Some(target) => Some(self.link(source, target)),
                None => {
                    let message = format!("{shown} names no item, so it stays as text");
                    self.entry(source, line, message);
                    None
                }
            };
        };
        let relation = match self.vocabulary.role(name) {
            Role::Forward(relation) => (relation.name.clone(), true),
            Role::Inverse(relation) => (relation.name.clone(), false),
            Role::Dropped(why) => {
                self.entry(
                    source,
                    line,
                    format!("{shown} stays as text: relation `{name}` {why}"),
                );
                return None;
            }
            Role::None => {
                let message =
                    format!("{shown} names no relation of the schema, so it stays as text");
                self.entry(source, line, message);
                return None;
            }
        };
        if is_address(token.target) {
            let message = format!(
                "{shown} stays as text: its target is not an item, and a relation leads to an item; \
                 a code marker in that file can link it (DES-CODE-MARKERS)"
            );
            self.entry(source, line, message);
            return None;
        }
        let Some(target) = self.resolve(token.target) else {
            let message = format!("{shown} names no item, so it stays as text");
            self.entry(source, line, message);
            return None;
        };
        match (me, relation) {
            (Some(me), (relation, true)) => {
                if let Some(row) = self.edge(me, &relation, target) {
                    self.incoming(me, row);
                }
            }
            (Some(me), (relation, false)) => {
                if let Some(row) = self.edge(target, &relation, me) {
                    self.incoming(target, row);
                }
            }
            (None, _) => {
                let message = format!(
                    "{shown} in narrative outside an item becomes a mention link: a relation is a \
                     row of its source item"
                );
                self.entry(source, line, message);
            }
        }
        Some(self.link(source, target))
    }

    /// The new contents of the source `index`, `None` when they are its contents, read back to
    /// check that every item reads as written (ADR-READ-BACK-EVERY-EDIT).
    fn render(
        &mut self,
        index: usize,
        items: Vec<Item>,
        status: Option<&str>,
        work: &mut WorkCounts,
    ) -> Result<Option<String>, Refusal> {
        let sources = self.sources;
        let source = &sources[index];
        let text = source.text.as_str();
        let eol = write::line_ending(text);
        let blocks = &source.scan.blocks;
        let mut splices: Vec<(Range<usize>, String)> = Vec::new();
        // Each expected item with where it is: its MID, rows, whether its body is filled, and
        // the line of the block it comes from.
        let mut expected: Vec<(usize, Expected)> = Vec::new();
        let mut items = items.into_iter();
        let mut cursor = 0;
        let mut level = None;
        let mut number = 0;
        while number < blocks.len() {
            let first = number;
            while number + 1 < blocks.len()
                && text[blocks[number].range.end..blocks[number + 1].range.start]
                    .trim()
                    .is_empty()
            {
                number += 1;
            }
            let group = blocks[first].range.start..blocks[number].range.end;
            level = self.narrative(index, cursor..group.start, level, &mut splices);
            let level = level.map_or(2, |level: usize| (level + 1).min(6));
            let mut texts = Vec::new();
            for (block_number, block) in blocks.iter().enumerate().take(number + 1).skip(first) {
                let item = items.next().expect("an item per block");
                let holder = Holder::Block(index, block_number);
                let mut rows = vec![
                    ("type".to_owned(), label(&block.flavour)),
                    ("id".to_owned(), cell(&block.id)),
                ];
                if !item.status
                    && let Some(status) = status
                {
                    rows.push((STATUS.to_owned(), label(status)));
                }
                rows.extend(item.rows);
                rows.extend(self.incoming.remove(&holder).unwrap_or_default());
                let written: Vec<String> = rows
                    .iter()
                    .map(|(name, cell)| write::row(name, cell))
                    .collect();
                let text = write::item(&item.mid, level, &item.title, &written, &item.body, eol)
                    .or_else(|_| {
                        write::item(&item.mid, level, &block.id, &written, &item.body, eol)
                    })
                    .expect("an ID is a title that can be written");
                texts.push(text);
                expected.push((
                    block.range.start,
                    Expected {
                        mid: item.mid,
                        rows,
                        body: Some(!item.body.trim().is_empty()),
                        line: block.line,
                        id: block.id.clone(),
                    },
                ));
            }
            let joined = texts.join(&format!("{eol}{eol}---{eol}{eol}"));
            let without = format!("{}{}", &text[..group.start], &text[group.end..]);
            let placed = write::insert(&without, group.start, &joined);
            splices.push((group.clone(), placed.text));
            cursor = group.end;
            number += 1;
        }
        if !source.converted() {
            self.narrative(index, cursor..text.len(), level, &mut splices);
        }
        for (number, item) in source.parsed.items.iter().enumerate() {
            let mut rows: Vec<Row> = item
                .rows
                .iter()
                .map(|row| (row.name.clone(), row.value.clone()))
                .collect();
            let added = self
                .incoming
                .remove(&Holder::Item(index, number))
                .unwrap_or_default();
            if !added.is_empty() {
                let mut text_added = String::new();
                for (name, cell) in &added {
                    text_added.push_str(&write::add_row(text, item, name, cell).text);
                }
                splices.push((item.table.end..item.table.end, text_added));
                rows.extend(added.into_iter().map(|(name, cell)| (name, read(&cell))));
            }
            let id = item.value("id").unwrap_or_default().to_owned();
            let line = item.line;
            expected.push((
                item.span.start,
                Expected {
                    mid: item.mid.clone(),
                    rows: rows
                        .into_iter()
                        .map(|(name, value)| (name, escape(&value)))
                        .collect(),
                    body: None,
                    line,
                    id,
                },
            ));
        }
        splices.sort_by_key(|(range, _)| range.start);
        let new = splice(text, &splices);
        if new == text {
            return Ok(None);
        }
        expected.sort_by_key(|(at, _)| *at);
        let expected: Vec<Expected> = expected.into_iter().map(|(_, item)| item).collect();
        check(&source.path, &new, &expected, work)?;
        Ok(Some(new))
    }

    /// Converts the mentions of the narrative at `range` of the source `index` into splices, and
    /// returns the level of its last heading, or `level` when it has none.
    fn narrative(
        &mut self,
        index: usize,
        range: Range<usize>,
        level: Option<usize>,
        splices: &mut Vec<(Range<usize>, String)>,
    ) -> Option<usize> {
        let sources = self.sources;
        let text = &sources[index].text[range.clone()];
        let markdown = scan::markdown(text);
        let first_line = 1 + sources[index].text[..range.start].matches('\n').count();
        for token in scan::tokens(text, &markdown.raw) {
            let line = first_line + text[..token.range.start].matches('\n').count();
            if let Some(link) = self.token(index, None, &token, line) {
                let at = range.start + token.range.start..range.start + token.range.end;
                splices.push((at, link));
            }
        }
        markdown.headings.last().copied().or(level)
    }
}

/// An item that a converted document must read back as.
struct Expected {
    mid: String,
    /// Each row's schema name and cell.
    rows: Vec<Row>,
    /// Whether its body holds anything; `None` for an item that was already in the format.
    body: Option<bool>,
    /// The 0.3.0 line it comes from.
    line: usize,
    id: String,
}

/// Parses the converted document `new` and refuses it unless its items are exactly `expected`,
/// with their rows, in order.
fn check(
    path: &str,
    new: &str,
    expected: &[Expected],
    work: &mut WorkCounts,
) -> Result<(), Refusal> {
    let parsed = document::parse(new.as_bytes(), work);
    let refuse = |item: &Expected, why: String| {
        let message = format!(
            "item `{}` would not read back as converted: {why}; nothing was written. Look for \
             an unclosed code block or HTML block in or before it",
            item.id
        );
        Err(Refusal::Unreadable(Box::new(Diagnostic::error(
            write::UNREADABLE_EDIT,
            path,
            item.line,
            message,
        ))))
    };
    for (at, item) in expected.iter().enumerate() {
        let Some(read) = parsed.items.get(at) else {
            return refuse(item, "it would not be read as an item".to_owned());
        };
        if read.mid != item.mid {
            let why = format!("the item read in its place has the MID {}", read.mid);
            return refuse(item, why);
        }
        let rows: Vec<(&str, String)> = read
            .rows
            .iter()
            .map(|row| (row.name.as_str(), escape(&row.value)))
            .collect();
        let want: Vec<(&str, String)> = item
            .rows
            .iter()
            .map(|(name, cell)| (name.as_str(), cell.clone()))
            .collect();
        if rows != want {
            return refuse(item, "its metadata rows would read differently".to_owned());
        }
        if item
            .body
            .is_some_and(|filled| filled == read.body.is_empty())
        {
            return refuse(item, "its body would read differently".to_owned());
        }
    }
    if let Some(extra) = parsed.items.get(expected.len()) {
        let item = Expected {
            mid: String::new(),
            rows: Vec::new(),
            body: None,
            line: expected.last().map_or(1, |last| last.line),
            id: extra.value("id").unwrap_or_default().to_owned(),
        };
        return refuse(
            &item,
            "the document would read it as one more item".to_owned(),
        );
    }
    Ok(())
}

/// The end of the report entry of a MID that the conversion gives an item.
const GIVEN_END: &str = ", which a rerun gives it again";

/// The ID and MID of a report entry that gives an item a MID.
fn given_mid(message: &str) -> Option<(&str, &str)> {
    let rest = message.strip_prefix("item `")?.strip_suffix(GIVEN_END)?;
    let (id, rest) = rest.split_once('`')?;
    let mid = rest
        .rsplit_once(": it gets the MID `")?
        .1
        .strip_suffix('`')?;
    is_mid(mid).then_some((id, mid))
}

/// Whether the block needs a new MID: it has no `:mid:` line, or its first holds no MID.
pub fn needs_mid(block: &Block) -> bool {
    block
        .metadata
        .iter()
        .find(|meta| meta.key == "mid")
        .is_none_or(|meta| !is_mid(&meta.value))
}

/// A value as a cell: a value that a cell cannot hold as it is, which validation reports, is
/// written with its `|` escaped.
fn cell(value: &str) -> String {
    write::cell(value).unwrap_or_else(|_| escape(value))
}

/// `value` with `|` as `\|`, as a cell writes it.
fn escape(value: &str) -> String {
    value.replace('|', "\\|")
}

/// A cell as the parser reads its value.
fn read(cell: &str) -> String {
    cell.replace("\\|", "|")
}

/// Whether a 0.3.0 relation target is a code reference or an external address, not an item.
fn is_address(target: &str) -> bool {
    target.starts_with("code:") || target.starts_with("external:")
}

fn not_an_item(shown: &str) -> String {
    format!(
        "{shown} is left out: its target is not an item, and a relation leads to an item; a code \
         marker in that file can link it (DES-CODE-MARKERS)"
    )
}

/// Whether byte `at` of `text` starts a line, after at most three spaces.
fn starts_line(text: &str, at: usize) -> bool {
    let before = &text[..at];
    let line = before
        .rfind(['\n', '\r'])
        .map_or(before, |end| &before[end + 1..]);
    line.len() <= 3 && line.bytes().all(|byte| byte == b' ')
}

/// `text` with each replacement applied; they are in order and do not overlap.
fn splice(text: &str, replacements: &[(Range<usize>, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, new) in replacements {
        if range.start < at {
            continue;
        }
        out.push_str(&text[at..range.start]);
        out.push_str(new);
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// `text` without the blank lines at its start and end.
fn trim_blank_lines(text: &str) -> &str {
    let mut start = 0;
    for line in scan::lines(text) {
        if !text[line.start..line.text_end].trim().is_empty() {
            break;
        }
        start = line.end;
    }
    let rest = &text[start..];
    let mut end = rest.len();
    for line in scan::lines(rest).iter().rev() {
        if !rest[line.start..line.text_end].trim().is_empty() {
            end = line.text_end;
            break;
        }
        end = line.start;
    }
    &rest[..end]
}
