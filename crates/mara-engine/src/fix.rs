//! The repairs of `project fix` (REQ-ONE-FIX-OPERATION in `docs/authoring.mara.md`,
//! DES-FIX-REPAIRS in `docs/authoring-requests.mara.md`): from a snapshot, the splices that repair each document and the
//! problems they repair. The host supplies new MIDs, reads the splices back and writes them, so
//! this module does no I/O (DES-ENGINE-BOUNDARY).

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use crate::document::{Anchorless, Document, Item, ReferenceKind, Row, is_mid, label_name, lines};
use crate::json::Object;
use crate::read::DIAGNOSTIC_BYTES;
use crate::snapshot::{Edge, EdgeSource, Snapshot};
use crate::validation::{is_id, resolve};
use crate::write::{Expect, Splice, anchor, heading, line_ending, link, relative, set_value};

/// The version of the JSON form of a fix result.
pub const FORMAT_VERSION: u64 = 1;

/// One problem that a fix repairs, where it was before the fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repair {
    /// The code of the problem (DES-SCHEMA-CHECKS, DES-REGIONS, DES-ITEM-STRUCTURE).
    pub code: &'static str,
    /// The document, relative to the project root.
    pub path: String,
    pub line: usize,
    /// What the fix writes.
    pub message: String,
}

/// The repairs of one document.
#[derive(Debug)]
pub struct DocumentFix {
    /// Its index among the snapshot's documents.
    pub document: usize,
    /// In byte order and apart, for [`crate::write::ReadBack::edit`].
    pub splices: Vec<Splice>,
    /// In line order.
    pub repairs: Vec<Repair>,
}

/// The repairs of every document of `snapshot` that has any, in path order. `new_mid` gives a
/// new MID for each anchor that fix adds; one that an item of the snapshot or an earlier anchor
/// already has is drawn again.
pub fn plan<E>(
    snapshot: &Snapshot,
    new_mid: &mut dyn FnMut() -> Result<String, E>,
) -> Result<Vec<DocumentFix>, E> {
    let planner = Planner::new(snapshot, new_mid)?;
    let fixes = (0..snapshot.documents().len())
        .filter_map(|document| planner.document(document))
        .collect();
    Ok(fixes)
}

/// The JSON form of a fix result: `format_version`, the first `repairs` in path and line order
/// that fit in [`DIAGNOSTIC_BYTES`], and `repairs_total`.
pub fn to_json(repairs: &[Repair]) -> String {
    let records = records(repairs);
    let mut out = String::new();
    let mut object = Object::new(&mut out);
    object.number("format_version", FORMAT_VERSION);
    object.array("repairs", records.iter().map(String::as_str));
    object.number("repairs_total", repairs.len() as u64);
    object.end();
    out
}

/// How many of `repairs`, from the first, a result shows.
pub fn shown(repairs: &[Repair]) -> usize {
    records(repairs).len()
}

/// The JSON forms of the repairs that a result shows: `code`, `path`, `line` and `message`.
fn records(repairs: &[Repair]) -> Vec<String> {
    let mut used = 0;
    let mut records = Vec::new();
    for repair in repairs {
        let mut record = String::new();
        let mut object = Object::new(&mut record);
        object.string("code", repair.code);
        object.string("path", &repair.path);
        object.number("line", repair.line as u64);
        object.string("message", &repair.message);
        object.end();
        let cost = record.len() + usize::from(!records.is_empty());
        if used + cost > DIAGNOSTIC_BYTES {
            break;
        }
        used += cost;
        records.push(record);
    }
    records
}

/// What the repairs of a snapshot read of its items more than once, and the MIDs of the
/// anchors that fix adds.
struct Planner<'s> {
    snapshot: &'s Snapshot,
    /// The schema name of each item's first `Type` value.
    kinds: Vec<String>,
    /// Each item's first `ID` value.
    ids: Vec<Option<&'s str>>,
    /// The new MID of each region that lacks only its anchor, by document, in source order.
    anchors: Vec<Vec<String>>,
    /// The document and new MID of each such region, by its first `ID` value, so that a bare ID
    /// can name it in the same fix.
    anchored: HashMap<&'s str, Vec<(&'s str, String)>>,
}

/// A change to a document's bytes: the splice's extent, and the replacements in it, apart and in
/// order, with the problems it repairs.
struct Edit {
    range: Range<usize>,
    replacements: Vec<(Range<usize>, String)>,
    expect: Expect,
    repairs: Vec<Repair>,
}

impl Edit {
    /// An edit that replaces `range` by `text`.
    fn replace(range: Range<usize>, text: String, repair: Repair) -> Edit {
        Edit {
            replacements: vec![(range.clone(), text)],
            range,
            expect: Expect::Bytes,
            repairs: vec![repair],
        }
    }

    /// The edit of a splice from the item writer.
    fn splice(splice: Splice, repairs: Vec<Repair>) -> Edit {
        Edit {
            replacements: vec![(splice.range.clone(), splice.text)],
            range: splice.range,
            expect: splice.expect,
            repairs,
        }
    }
}

/// One document being repaired.
struct Page<'s> {
    path: &'s str,
    contents: &'s str,
    parsed: &'s Document,
    eol: &'static str,
    edits: Vec<Edit>,
    /// The edit of each reference definition that fix rewrites, by where it starts.
    definitions: HashMap<usize, usize>,
    /// The extent of each reference definition and the links and images that use it, by where
    /// it starts, made on first use.
    users: Option<HashMap<usize, Range<usize>>>,
}

impl Page<'_> {
    fn repair(&self, code: &'static str, line: usize, message: String) -> Repair {
        Repair {
            code,
            path: self.path.to_owned(),
            line,
            message,
        }
    }
}

impl<'s> Planner<'s> {
    /// Reads what the repairs need of every item, and draws the MID of every anchor that fix
    /// adds before it plans any document, so that a bare ID can name an item that gets one.
    fn new<E>(
        snapshot: &'s Snapshot,
        new_mid: &mut dyn FnMut() -> Result<String, E>,
    ) -> Result<Planner<'s>, E> {
        let count = snapshot.items().len();
        let (mut kinds, mut ids) = (Vec::with_capacity(count), Vec::with_capacity(count));
        for index in 0..count {
            let rows = &snapshot.item(index).rows;
            kinds.push(kind(rows));
            ids.push(first(rows, "id").map(|row| row.value.as_str()));
        }
        let mut used = HashSet::new();
        let mut anchors = Vec::with_capacity(snapshot.documents().len());
        let mut anchored: HashMap<&'s str, Vec<(&'s str, String)>> = HashMap::new();
        for document in snapshot.documents() {
            let mut mids = Vec::new();
            for region in &document.file.document.anchorless {
                let mid = loop {
                    let mid = new_mid()?;
                    if snapshot.items_with_mid(&mid).is_empty() && used.insert(mid.clone()) {
                        break mid;
                    }
                };
                if let Some(id) = first(&region.rows, "id") {
                    let place = (document.path.as_str(), mid.clone());
                    anchored.entry(id.value.as_str()).or_default().push(place);
                }
                mids.push(mid);
            }
            anchors.push(mids);
        }
        Ok(Planner {
            snapshot,
            kinds,
            ids,
            anchors,
            anchored,
        })
    }

    fn document(&self, index: usize) -> Option<DocumentFix> {
        let snapshot = self.snapshot;
        let document = &snapshot.documents()[index];
        // A document that is not UTF-8 has no items, links or regions to repair.
        let contents = std::str::from_utf8(&document.file.contents).ok()?;
        let mut page = Page {
            path: &document.path,
            contents,
            parsed: &document.file.document,
            eol: line_ending(contents),
            edits: Vec::new(),
            definitions: HashMap::new(),
            users: None,
        };
        let mut flagged: HashMap<&str, HashSet<usize>> = HashMap::new();
        for diagnostic in page.parsed.diagnostics("") {
            flagged
                .entry(diagnostic.code)
                .or_default()
                .insert(diagnostic.line);
        }
        let flagged =
            |code: &str, line: usize| flagged.get(code).is_some_and(|l| l.contains(&line));
        // An item that gets its anchor is repaired as fully as one that has it.
        for (region, mid) in page.parsed.anchorless.iter().zip(&self.anchors[index]) {
            self.anchor(&mut page, region, mid, &flagged);
            self.relations(&mut page, &kind(&region.rows), &region.rows);
        }
        for item in document.items.clone() {
            self.formatting(&mut page, snapshot.item(item), &flagged);
            self.relations(&mut page, &self.kinds[item], &snapshot.item(item).rows);
            for edge in snapshot.outgoing(item) {
                self.mention(&mut page, edge);
            }
        }
        for edge in snapshot.narrative(index) {
            self.mention(&mut page, edge);
        }
        if page.edits.is_empty() {
            return None;
        }
        let (splices, mut repairs) = merge(contents, page.edits);
        repairs.sort_by_key(|repair| repair.line);
        Some(DocumentFix {
            document: index,
            splices,
            repairs,
        })
    }

    /// The anchor of `mid`, and a blank line, before the heading of `region`, which lacks only
    /// its anchor, in a splice that holds the whole region, which becomes an item
    /// (DES-ITEM-WRITER). Also an ATX heading for a setext heading of one line.
    fn anchor(
        &self,
        page: &mut Page<'_>,
        region: &Anchorless,
        mid: &str,
        flagged: &dyn Fn(&str, usize) -> bool,
    ) {
        let eol = page.eol;
        let message =
            format!("added an anchor with the new MID {mid} before the heading, so the item loads");
        let repair = page.repair("incomplete-item", region.line, message);
        let start = region.span.start;
        page.edits.push(Edit {
            range: region.span.clone(),
            replacements: vec![(start..start, format!("{}{eol}{eol}", anchor(mid)))],
            expect: Expect::Bytes,
            repairs: vec![repair],
        });
        let base = (region.heading.start, region.line);
        self.setext(page, mid, region.heading.clone(), base, flagged);
    }

    /// The separator before an anchor that lacks it, the blank line after an anchor that lacks
    /// only that, and an ATX heading for a setext heading of one line in the heading's place
    /// (DES-REGIONS, DES-MID-ANCHOR). `flagged` tells whether the parser reported a code at a
    /// line.
    fn formatting(&self, page: &mut Page<'_>, item: &Item, flagged: &dyn Fn(&str, usize) -> bool) {
        let (contents, eol) = (page.contents, page.eol);
        let start = item.span.start;
        let line_start = contents[..start].rfind(['\n', '\r']).map_or(0, |at| at + 1);
        if flagged("missing-separator", item.line) {
            let text = match previous_line_blank(contents, line_start) {
                true => format!("---{eol}{eol}"),
                false => format!("{eol}---{eol}{eol}"),
            };
            let message = format!("added a `---` line before the anchor of MID {}", item.mid);
            let repair = page.repair("missing-separator", item.line, message);
            page.edits
                .push(Edit::replace(line_start..line_start, text, repair));
        }
        let anchor_end = start + anchor(&item.mid).len();
        // The paragraph of the anchor ends with it, or with the text after it when merged.
        let paragraph_end = item.lead.end.max(anchor_end);
        let Some(head_start) = line_end(contents, paragraph_end) else {
            return;
        };
        let exact = item.lead.start == start;
        if exact
            && let Some(next) = lines(&contents[head_start..]).next()
            && !is_blank(next)
        {
            let message = format!("added a blank line after the anchor of MID {}", item.mid);
            let repair = page.repair("merged-anchor", item.line, message);
            page.edits.push(Edit::replace(
                head_start..head_start,
                eol.to_owned(),
                repair,
            ));
        }
        // The heading lies between the anchor and the table.
        let head = head_start..item.table.start.max(head_start);
        self.setext(page, &item.mid, head, (line_start, item.line), flagged);
    }

    /// An ATX heading for the setext heading of the item with MID `mid`, when the lines of
    /// `head` that are not blank are one line of text and its underline, and the parser reported
    /// the underline. `base` is a line start before them and its line.
    fn setext(
        &self,
        page: &mut Page<'_>,
        mid: &str,
        head: Range<usize>,
        base: (usize, usize),
        flagged: &dyn Fn(&str, usize) -> bool,
    ) {
        let contents = page.contents;
        let mut offset = head.start;
        let mut written = Vec::new();
        for line in lines(&contents[head]) {
            if !is_blank(line) {
                written.push(offset..offset + line.trim_end_matches(['\n', '\r']).len());
            }
            offset += line.len();
        }
        let [text, underline] = &written[..] else {
            return;
        };
        let underline_line = base.1 + line_count(&contents[base.0..underline.start]);
        let Some(level) = setext_level(&contents[underline.clone()]) else {
            return;
        };
        if !flagged("setext-heading", underline_line) {
            return;
        }
        let title = contents[text.clone()].trim_matches([' ', '\t']);
        let Ok(atx) = heading(level, title) else {
            return;
        };
        let message = format!("wrote the heading of the item with MID {mid} as `{atx}`");
        let repair = page.repair("setext-heading", underline_line, message);
        page.edits
            .push(Edit::replace(text.start..underline.end, atx, repair));
    }

    /// The relation link, as the writer writes it, for each relation value among `rows`, of an
    /// item of type `kind`, that is a bare ID of one item or a link to one item whose path or
    /// text is stale. A bare ID may name an item that gets its anchor in this fix.
    fn relations(&self, page: &mut Page<'_>, kind: &str, rows: &[Row]) {
        let snapshot = self.snapshot;
        let Some(schema) = snapshot.schema() else {
            return;
        };
        let from = page.path;
        for row in rows {
            if schema.relation_from(kind, &row.name).is_none() {
                continue;
            }
            let Some(old) = &row.link else {
                if !is_id(&row.value) {
                    continue;
                }
                let Some((to, mid)) = self.by_id(&row.value) else {
                    continue;
                };
                let value = link(&row.value, mid, from, to);
                let message = format!("wrote the `{}` value as the link `{value}`", row.key);
                let repair = page.repair("bare-id-value", row.line, message);
                page.edits
                    .push(Edit::splice(set_value(row, &value), vec![repair]));
                continue;
            };
            let Some(mid) = old.fragment.as_deref().filter(|fragment| is_mid(fragment)) else {
                continue;
            };
            let &[target] = snapshot.items_with_mid(mid) else {
                continue;
            };
            let to = self.path(target);
            let id = self.ids[target];
            let mut repairs = Vec::new();
            if resolve(from, &old.path).as_deref() != Some(to) {
                let message = path_message(mid, &relative(from, to), to);
                repairs.push(page.repair("stale-link-path", old.line, message));
            }
            if let Some(id) = id.filter(|id| old.text != *id) {
                let message =
                    format!("wrote the link text `{id}`, the ID of the item with MID {mid}");
                repairs.push(page.repair("stale-link-text", old.line, message));
            }
            if repairs.is_empty() {
                continue;
            }
            // The canonical relation link: the target's ID, or the text when it has none.
            let value = link(id.unwrap_or(&old.text), mid, from, to);
            page.edits
                .push(Edit::splice(set_value(row, &value), repairs));
        }
    }

    /// The path of the mention of `edge` when it is stale and the MID is that of one item. A
    /// relation's edge is left to [`Planner::relations`].
    fn mention(&self, page: &mut Page<'_>, edge: &Edge) {
        if let EdgeSource::Row(_) = edge.source {
            return;
        }
        let snapshot = self.snapshot;
        let old = snapshot.link(edge);
        let mid = old.fragment.as_deref().expect("an edge links to a MID");
        let &[target] = snapshot.items_with_mid(mid) else {
            return;
        };
        let (from, to) = (page.path, self.path(target));
        if resolve(from, &old.path).as_deref() == Some(to) {
            return;
        }
        let path = relative(from, to);
        let references = &page.parsed.references;
        let at = references.partition_point(|reference| reference.span.start < old.span.start);
        let Some(reference) = references
            .get(at)
            .filter(|reference| reference.span.start == old.span.start)
            .filter(|reference| reference.kind == ReferenceKind::Link)
        else {
            return;
        };
        let destination = &reference.destination;
        if !destination
            .strip_prefix(old.path.as_str())
            .is_some_and(|rest| rest.starts_with('#'))
        {
            return;
        }
        let repair = page.repair("stale-link-path", old.line, path_message(mid, &path, to));
        let contents = page.contents;
        let Some(definition) = reference.definition.clone() else {
            let Some(written) = inline_destination(&contents[old.span.clone()], destination) else {
                return;
            };
            let start = old.span.start + written.start;
            let range = start..start + old.path.len();
            page.edits.push(Edit::replace(range, path, repair));
            return;
        };
        // A reference link: the path in its definition, in one splice with every link and image
        // that uses the definition, so that they read as the edit means (DES-ITEM-WRITER).
        if let Some(&edit) = page.definitions.get(&definition.start) {
            page.edits[edit].repairs.push(repair);
            return;
        }
        let Some(written) = definition_destination(&contents[definition.clone()], destination)
        else {
            return;
        };
        let start = definition.start + written.start;
        let users = page.users.get_or_insert_with(|| users(page.parsed));
        let range = users
            .get(&definition.start)
            .cloned()
            .unwrap_or(definition.clone());
        page.definitions.insert(definition.start, page.edits.len());
        page.edits.push(Edit {
            range,
            replacements: vec![(start..start + old.path.len(), path)],
            expect: Expect::Bytes,
            repairs: vec![repair],
        });
    }

    /// The document and MID of the one item with the ID `id`, among the items of the snapshot
    /// and those that get their anchor in this fix.
    fn by_id(&self, id: &str) -> Option<(&str, &str)> {
        let snapshot = self.snapshot;
        let anchored = self.anchored.get(id).map_or(&[][..], Vec::as_slice);
        match (snapshot.items_with_id(id), anchored) {
            (&[item], []) => Some((self.path(item), &snapshot.item(item).mid)),
            ([], [(path, mid)]) => Some((path, mid)),
            _ => None,
        }
    }

    /// The path of the document of the snapshot item `index`.
    fn path(&self, index: usize) -> &'s str {
        let snapshot = self.snapshot;
        &snapshot.documents()[snapshot.items()[index].document].path
    }
}

/// The first of `rows` whose key matches the schema name `name`.
fn first<'r>(rows: &'r [Row], name: &str) -> Option<&'r Row> {
    rows.iter().find(|row| row.name == name)
}

/// The schema name of the first `Type` value among `rows`.
fn kind(rows: &[Row]) -> String {
    first(rows, "type").map_or_else(String::new, |row| label_name(&row.value))
}

/// What fix says when it writes `path` for a link to MID `mid` in the document `to`.
fn path_message(mid: &str, path: &str, to: &str) -> String {
    match path {
        "" => format!("wrote `#{mid}` without a path, since MID {mid} is in this file"),
        path => format!("wrote the path `{path}`, since MID {mid} is in {to}"),
    }
}

/// The extent of each reference definition of `document` together with the links and images
/// that use it, by where the definition starts.
fn users(document: &Document) -> HashMap<usize, Range<usize>> {
    let mut users: HashMap<usize, Range<usize>> = HashMap::new();
    for reference in &document.references {
        if let Some(definition) = &reference.definition {
            let extent = users
                .entry(definition.start)
                .or_insert_with(|| definition.clone());
            extent.start = extent.start.min(reference.span.start);
            extent.end = extent.end.max(reference.span.end);
        }
    }
    users
}

/// The splices of `edits` on `contents`, in byte order: edits that overlap or touch become one
/// splice of their bytes. Also the repairs they make. An edit whose replacements would overlap
/// those of an earlier one is left out with its repairs.
fn merge(contents: &str, mut edits: Vec<Edit>) -> (Vec<Splice>, Vec<Repair>) {
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    let mut merged: Vec<Edit> = Vec::with_capacity(edits.len());
    for mut edit in edits {
        let Some(last) = merged
            .last_mut()
            .filter(|last| edit.range.start <= last.range.end)
        else {
            merged.push(edit);
            continue;
        };
        let mut replacements = last.replacements.clone();
        replacements.append(&mut edit.replacements);
        replacements.sort_by_key(|(range, _)| (range.start, range.end));
        let apart = replacements.windows(2).all(|pair| {
            let (a, b) = (&pair[0].0, &pair[1].0);
            a.end <= b.start && !(a.is_empty() && b.is_empty() && a.start == b.start)
        });
        if !apart {
            continue;
        }
        last.range = last.range.start..last.range.end.max(edit.range.end);
        last.replacements = replacements;
        last.expect = Expect::Bytes;
        last.repairs.append(&mut edit.repairs);
    }
    let mut splices = Vec::with_capacity(merged.len());
    let mut repairs = Vec::new();
    for edit in merged {
        let mut text = String::new();
        let mut at = edit.range.start;
        for (range, replacement) in &edit.replacements {
            text.push_str(&contents[at..range.start]);
            text.push_str(replacement);
            at = range.end;
        }
        text.push_str(&contents[at..edit.range.end]);
        splices.push(Splice {
            range: edit.range,
            text,
            expect: edit.expect,
        });
        repairs.extend(edit.repairs);
    }
    (splices, repairs)
}

/// Where the destination of the inline link `source` is written, when it is written exactly as
/// `destination`, without escapes or character references, and only one place in `source` can
/// be it. `None` otherwise.
pub(crate) fn inline_destination(source: &str, destination: &str) -> Option<Range<usize>> {
    if !source.ends_with(')') || destination.is_empty() {
        return None;
    }
    let mut found = None;
    for (at, _) in source.match_indices("](") {
        let Some((written, end)) = destination_at(source, at + 2, destination) else {
            continue;
        };
        let after = &source[end..];
        // A title or the closing parenthesis follows the destination.
        if after == ")" || after.starts_with([' ', '\t', '\n', '\r']) {
            if found.is_some() {
                return None;
            }
            found = Some(written);
        }
    }
    found
}

/// Where the destination of the link reference definition `source` is written, when it is
/// written exactly as `destination`. `None` otherwise.
pub(crate) fn definition_destination(source: &str, destination: &str) -> Option<Range<usize>> {
    let bytes = source.as_bytes();
    let mut at = source.find('[')? + 1;
    // A label holds no unescaped bracket.
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b']' => break,
            b'[' => return None,
            _ => at += 1,
        }
    }
    if source.get(at..at + 2) != Some("]:") || destination.is_empty() {
        return None;
    }
    let (written, end) = destination_at(source, at + 2, destination)?;
    let after = &source[end..];
    (after.is_empty() || after.starts_with([' ', '\t', '\n', '\r'])).then_some(written)
}

/// The bytes of `destination` written from `at` after spaces and tabs and at most one line
/// ending, bare or in angle brackets, and where what follows it starts. `None` when the text
/// there is not `destination`.
fn destination_at(source: &str, at: usize, destination: &str) -> Option<(Range<usize>, usize)> {
    let mut start = at + spaces(&source[at..]);
    let rest = &source[start..];
    let ending = match rest {
        _ if rest.starts_with("\r\n") => 2,
        _ if rest.starts_with(['\n', '\r']) => 1,
        _ => 0,
    };
    if ending > 0 {
        start += ending;
        start += spaces(&source[start..]);
    }
    let rest = &source[start..];
    if let Some(inner) = rest.strip_prefix('<') {
        let closed = inner
            .strip_prefix(destination)
            .is_some_and(|after| after.starts_with('>'));
        let end = start + 1 + destination.len();
        return closed.then_some((start + 1..end, end + 1));
    }
    rest.starts_with(destination)
        .then_some((start..start + destination.len(), start + destination.len()))
}

/// The length of the spaces and tabs that `text` starts with.
fn spaces(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

/// Whether `line`, with its line ending, holds only spaces and tabs.
fn is_blank(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\n', '\r']).is_empty()
}

/// Where the line that holds byte `at` ends, after its line ending: `None` when it has none.
fn line_end(contents: &str, at: usize) -> Option<usize> {
    let rest = &contents[at..];
    let line = lines(rest).next()?;
    line.ends_with(['\n', '\r']).then_some(at + line.len())
}

/// Whether the line before the one that starts at `start` holds only spaces and tabs.
fn previous_line_blank(contents: &str, start: usize) -> bool {
    let before = &contents[..start];
    let before = before
        .strip_suffix("\r\n")
        .or_else(|| before.strip_suffix(['\n', '\r']))
        .unwrap_or(before);
    let line = &before[before.rfind(['\n', '\r']).map_or(0, |at| at + 1)..];
    is_blank(line)
}

/// The line endings in `text`: LF, CRLF or a lone CR each.
fn line_count(text: &str) -> usize {
    lines(text)
        .filter(|line| line.ends_with(['\n', '\r']))
        .count()
}

/// The level of the setext heading whose underline is `line`: 1 for `=`, 2 for `-`.
fn setext_level(line: &str) -> Option<usize> {
    let unindented = line.trim_start_matches(' ');
    if line.len() - unindented.len() > 3 {
        return None;
    }
    let marks = unindented.trim_end_matches([' ', '\t']);
    match marks.as_bytes().first() {
        Some(b'=') if marks.bytes().all(|byte| byte == b'=') => Some(1),
        Some(b'-') if marks.bytes().all(|byte| byte == b'-') => Some(2),
        _ => None,
    }
}
