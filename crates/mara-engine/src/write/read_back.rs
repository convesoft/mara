//! The read-back of an edit (DES-ITEM-WRITER, ADR-READ-BACK-EVERY-EDIT): the parse of the new
//! contents against the parse the edit was computed from. Only the bytes that the splices
//! replace or insert may read differently. Every item, metadata row, top-level block and
//! reference (link, image, footnote reference) whose own bytes the edit leaves alone must read as
//! before apart from offsets, and nothing new may read as one outside the changed bytes. Link
//! reference and footnote definitions are document-wide, so this compares the whole document,
//! with no exemption for an item or narrative that a splice merely touches.

use std::collections::HashMap;
use std::ops::Range;

use super::{Expect, Misread, RowChange, Splice};
use crate::document::{Item, Reference, ReferenceKind, Row, front_matter};
use crate::snapshot::ParsedFile;

/// Checks that `new`, the parse of `old` with `splices` applied, reads as
/// [`super::ReadBack::edit`] requires.
pub(super) fn check(old: &ParsedFile, new: &ParsedFile, splices: &[Splice]) -> Result<(), Misread> {
    Reading::new(old, new, splices).check()
}

/// An edit's old and new parse, with where each splice's text lands.
struct Reading<'r> {
    old: &'r ParsedFile,
    new: &'r ParsedFile,
    splices: &'r [Splice],
    /// Where the text of each splice starts in the new contents.
    starts: Vec<usize>,
    /// Where the text of each splice ends in the new contents.
    ends: Vec<usize>,
    /// The shift of every offset after the first `n` splices, for each `n`.
    shifts: Vec<isize>,
    /// Where each line of the old contents starts.
    lines: Vec<usize>,
    /// The spans of the old items that the edit removes, in order.
    removed: Vec<Range<usize>>,
}

type Checked<T = ()> = Result<T, Misread>;

impl<'r> Reading<'r> {
    fn new(old: &'r ParsedFile, new: &'r ParsedFile, splices: &'r [Splice]) -> Reading<'r> {
        let mut shifts = vec![0isize];
        let mut starts = Vec::with_capacity(splices.len());
        let mut ends = Vec::with_capacity(splices.len());
        for splice in splices {
            let shift = shifts[shifts.len() - 1];
            let start = splice.range.start.saturating_add_signed(shift);
            starts.push(start);
            ends.push(start + splice.text.len());
            shifts.push(shift + splice.text.len() as isize - splice.range.len() as isize);
        }
        let bytes = &old.contents;
        let mut lines = vec![0];
        for (at, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' || (byte == b'\r' && bytes.get(at + 1) != Some(&b'\n')) {
                lines.push(at + 1);
            }
        }
        Reading {
            old,
            new,
            splices,
            starts,
            ends,
            shifts,
            lines,
            removed: Vec::new(),
        }
    }

    fn check(mut self) -> Checked {
        if markdown_start(&self.old.contents) != markdown_start(&self.new.contents) {
            return misread(
                1,
                "the edit would change how the front matter at the start of the document reads: \
                 end its closing `---` line with a line break"
                    .to_owned(),
            );
        }
        self.insertion_points()?;
        self.removals()?;
        let inserted = self.inserted()?;
        self.items(&inserted)?;
        self.rows()?;
        self.references()?;
        self.blocks()?;
        self.problems(&inserted)
    }

    /// The old line of the old `offset`.
    fn line(&self, offset: usize) -> usize {
        self.lines.partition_point(|&start| start <= offset)
    }

    /// The new offset of the old `offset`, which no splice replaces; text inserted at it comes
    /// before it.
    fn map(&self, offset: usize) -> usize {
        let before = self
            .splices
            .partition_point(|splice| splice.range.end <= offset);
        offset.saturating_add_signed(self.shifts[before])
    }

    /// The old offset of the new `offset`, which no splice writes; right after written text, it
    /// is the old offset right after what that text replaced.
    fn unmap(&self, offset: usize) -> usize {
        let before = self.ends.partition_point(|&end| end <= offset);
        offset.saturating_add_signed(-self.shifts[before])
    }

    /// The old line of the new `offset`: that of the splice whose text holds it, or of the old
    /// byte it was.
    fn old_line(&self, offset: usize) -> usize {
        if self.inside_written(offset) {
            let after = self.starts.partition_point(|&start| start <= offset);
            return self.line(self.splices[after - 1].range.start);
        }
        self.line(self.unmap(offset))
    }

    /// Whether a splice changes the old bytes `span`: replaces any of them or inserts strictly
    /// inside them.
    fn changed(&self, span: &Range<usize>) -> bool {
        let first = self
            .splices
            .partition_point(|splice| splice.range.end < span.start);
        self.splices[first..]
            .iter()
            .take_while(|splice| splice.range.start <= span.end)
            .any(|splice| crosses(&splice.range, span))
    }

    /// Whether a splice changes the old bytes `span` or replaces or inserts text right at either
    /// end of them.
    fn changed_at(&self, span: &Range<usize>) -> bool {
        let first = self
            .splices
            .partition_point(|splice| splice.range.end < span.start);
        self.splices
            .get(first)
            .is_some_and(|splice| splice.range.start <= span.end)
    }

    /// Whether the new byte `offset` is one that a splice writes.
    fn inside_written(&self, offset: usize) -> bool {
        let after = self.starts.partition_point(|&start| start <= offset);
        after > 0 && offset < self.ends[after - 1]
    }

    /// Whether the new bytes `span` hold text that a splice writes, or a place inside them where
    /// a splice deleted text.
    fn written(&self, span: &Range<usize>) -> bool {
        let first = self.ends.partition_point(|&end| end < span.start);
        (first..self.splices.len())
            .take_while(|&index| self.starts[index] <= span.end)
            .any(|index| crosses(&(self.starts[index]..self.ends[index]), span))
    }

    /// An inserted item goes between blocks: not inside an item, nor inside a block or link
    /// reference definition, which it would split.
    fn insertion_points(&self) -> Checked {
        let items = &self.old.document.items;
        let document = &self.old.document;
        let mut blocks: Vec<Range<usize>> = document.blocks.clone();
        blocks.extend(document.definitions.iter().cloned());
        blocks.sort_by_key(|block| block.start);
        let blocks = &blocks;
        let mut ends = Vec::with_capacity(blocks.len());
        for block in blocks {
            let previous = ends.last().copied().unwrap_or(0);
            ends.push(block.end.max(previous));
        }
        for splice in self.splices {
            let Expect::Item { .. } = splice.expect else {
                continue;
            };
            let at = splice.range.start;
            if let Some(item) = holder(items, at).filter(|item| item.span.start < at) {
                let message = format!("the edit would write an item into {}", named(item));
                return misread(self.line(at), message);
            }
            let before = blocks.partition_point(|block| block.start < at);
            if before > 0 && at < ends[before - 1] {
                let block = blocks[..before]
                    .iter()
                    .rfind(|block| at < block.end)
                    .expect("a block ends after the insertion point");
                let message = format!(
                    "the item written here would split the block that starts at line {}, such as \
                     a paragraph, list, table, quote or code block: write it before or after that \
                     block",
                    self.line(block.start)
                );
                return misread(self.line(at), message);
            }
        }
        Ok(())
    }

    /// Each removal takes exactly one item and, around it, only blank lines, spaces and
    /// separators.
    fn removals(&mut self) -> Checked {
        let items = &self.old.document.items;
        for splice in self.splices {
            if splice.expect != Expect::Removal {
                continue;
            }
            let range = &splice.range;
            let first = items.partition_point(|item| item.span.end <= range.start);
            let overlapping: Vec<&Item> = items[first..]
                .iter()
                .take_while(|item| item.span.start < range.end)
                .collect();
            let line = self.line(range.start);
            let [item] = overlapping[..] else {
                let count = overlapping.len();
                return misread(
                    line,
                    format!("the removal would take {count} items instead of one"),
                );
            };
            if item.span.start < range.start || range.end < item.span.end {
                return misread(line, format!("the removal would cut {}", named(item)));
            }
            let around = [range.start..item.span.start, item.span.end..range.end];
            if around
                .iter()
                .any(|part| !spacing(&self.old.contents[part.clone()]))
            {
                let message = "the removal would take narrative text with it".to_owned();
                return misread(line, message);
            }
            self.removed.push(item.span.clone());
        }
        Ok(())
    }

    /// Each inserted item reads as its own text: it starts where it was put, ends within its
    /// text, and each of its references that a definition decides uses one inside it. Bytes after
    /// its last block, such as definitions and trailing spaces, stay. Returns the index and lines
    /// of each.
    fn inserted(&self) -> Checked<Vec<(usize, usize, usize)>> {
        let items = &self.new.document.items;
        let references = &self.new.document.references;
        let mut inserted = Vec::new();
        for (index, splice) in self.splices.iter().enumerate() {
            let Expect::Item { at, len } = splice.expect else {
                continue;
            };
            let (start, end) = (self.starts[index] + at, self.starts[index] + at + len);
            let line = self.line(splice.range.start);
            let Some(found) = starting(items, start, |item| item.span.start) else {
                return misread(
                    line,
                    "the item written here would not be read where it is written: a block that \
                     starts before it, such as an unclosed code block, HTML block or comment, \
                     would take it in"
                        .to_owned(),
                );
            };
            let item = &items[found];
            if item.span.end > end {
                return misread(
                    line,
                    "the item written here would run on past its own text and take in what \
                     follows it, for example through an unclosed code block in its body"
                        .to_owned(),
                );
            }
            let first = references.partition_point(|reference| reference.span.start < start);
            let own = references[first..]
                .iter()
                .take_while(|reference| reference.span.start < end);
            for reference in own {
                let Some(definition) = &reference.definition else {
                    continue;
                };
                if start <= definition.start && definition.end <= end {
                    continue;
                }
                let message = format!(
                    "the item written here would not read as written: `{}` in it would use the \
                     definition `[{}]` at line {}, outside the item; give it a label that the \
                     item defines and that no earlier text does",
                    source(&self.new.contents, &reference.span),
                    label(&self.new.contents, definition),
                    self.old_line(definition.start)
                );
                return misread(line, message);
            }
            inserted.push((found, item.line, item.end_line));
        }
        Ok(inserted)
    }

    /// Each old item that the edit does not remove, and whose anchor it leaves alone, is read
    /// where its anchor moved: with its title unless the edit changes its heading, ending where
    /// its end moved unless the edit writes there, and with its rows as their changes make them.
    /// Every other new item is inserted or has an anchor that the edit writes.
    fn items(&self, inserted: &[(usize, usize, usize)]) -> Checked {
        let old_items = &self.old.document.items;
        let new_items = &self.new.document.items;
        let mut accounted = vec![false; new_items.len()];
        for &(index, _, _) in inserted {
            accounted[index] = true;
        }
        let mut removed = self.removed.iter().peekable();
        for item in old_items {
            if removed.next_if(|span| **span == item.span).is_some() {
                continue;
            }
            let named = named(item);
            if self.changed(&anchor(item)) {
                continue;
            }
            let at = self.map(item.span.start);
            let found = starting(new_items, at, |item| item.span.start)
                .filter(|&index| new_items[index].mid == item.mid);
            let Some(index) = found else {
                let message = format!(
                    "{named} would no longer read as an item: the edit changes text that it \
                     depends on"
                );
                return misread(item.line, message);
            };
            accounted[index] = true;
            let new_item = &new_items[index];
            if !self.changed(&(item.span.start..item.table.start)) && new_item.title != item.title {
                let message = format!(
                    "{named} would read differently: its title would read `{}`",
                    new_item.title
                );
                return misread(item.line, message);
            }
            let end = item.span.end;
            if !self.changed_at(&(end..end)) && new_item.span.end != self.map(end) {
                let message = format!(
                    "{named} would end elsewhere: text after it would join it, or text in it would \
                     leave it"
                );
                return misread(item.line, message);
            }
            let first = self
                .splices
                .partition_point(|splice| splice.range.end < item.span.start);
            let inner: Vec<&Splice> = self.splices[first..]
                .iter()
                .take_while(|splice| splice.range.start <= item.span.end)
                .filter(|splice| matches!(splice.expect, Expect::Row(_)))
                .collect();
            if let Some(rows) = expected_rows(item, &inner)
                && rows != sorted_rows(new_item)
            {
                let message = format!(
                    "the metadata rows of {named} would not read back as the edit writes them"
                );
                return misread(item.line, message);
            }
        }
        for (item, _) in new_items.iter().zip(&accounted).filter(|(_, done)| !**done) {
            if !self.written(&anchor(item)) {
                let message = format!(
                    "the edit would make other text read as the item `{}` with MID {}",
                    item.title, item.mid
                );
                return misread(self.old_line(item.span.start), message);
            }
        }
        Ok(())
    }

    /// Every metadata row whose bytes the edit leaves alone reads as before, in the same item,
    /// and no other row appears outside the text the edit writes.
    fn rows(&self) -> Checked {
        let old_rows = rows_of(&self.old.document.items);
        let new_rows = rows_of(&self.new.document.items);
        for &(item, row) in &old_rows {
            if self.changed(&row.span) {
                continue;
            }
            let at = self.map(row.span.start);
            let found =
                starting(&new_rows, at, |(_, row)| row.span.start).map(|index| new_rows[index]);
            if let Some((new_item, new_row)) = found
                && new_item.mid == item.mid
                && row_reading(new_row) == row_reading(row)
            {
                continue;
            }
            let link = |document: &'r ParsedFile, row: Option<&Row>| {
                let span = &row?.link.as_ref()?.span;
                let references = &document.document.references;
                starting(references, span.start, |reference| reference.span.start)
                    .map(|index| &references[index])
            };
            let after = found.map(|(_, row)| row);
            let (before, after_link) = (link(self.old, Some(row)), link(self.new, after));
            let what = match (found, before == after_link) {
                (Some((new_item, _)), _) if new_item.mid != item.mid => {
                    format!("would move to {}", named(new_item))
                }
                (Some((_, new_row)), _) if link_reading(new_row) == link_reading(row) => {
                    format!("would read `{}` instead of `{}`", new_row.value, row.value)
                }
                _ => change(before, after_link),
            };
            let message = format!(
                "{} would read differently: its `{}` row {what}{}",
                named(item),
                row.key,
                self.cause(before, after_link)
            );
            return misread(row.line, message);
        }
        for &(item, row) in &new_rows {
            if self.written(&row.span) {
                continue;
            }
            let at = self.unmap(row.span.start);
            if starting(&old_rows, at, |(_, row)| row.span.start).is_none() {
                let message = format!(
                    "the edit would make other text read as the `{}` row of {}",
                    row.key,
                    named(item)
                );
                return misread(self.old_line(row.span.start), message);
            }
        }
        Ok(())
    }

    /// Every link, image and footnote reference whose bytes the edit leaves alone reads as
    /// before, in the same item or narrative, and no other appears outside the text the edit
    /// writes.
    fn references(&self) -> Checked {
        let old_references = &self.old.document.references;
        let new_references = &self.new.document.references;
        for reference in old_references {
            if self.changed(&reference.span) {
                continue;
            }
            let at = self.map(reference.span.start);
            let found = starting(new_references, at, |reference| reference.span.start)
                .map(|index| &new_references[index]);
            let old_owner = holder(&self.old.document.items, reference.span.start);
            let new_owner = holder(&self.new.document.items, at);
            let same_owner = old_owner.map(|item| &item.mid) == new_owner.map(|item| &item.mid);
            if let Some(after) = found
                && same_owner
                && self.same_reading(reference, after)
            {
                continue;
            }
            let what = match found {
                Some(after) if self.same_reading(reference, after) => match new_owner {
                    Some(item) => format!("would become part of {}", named(item)),
                    None => "would leave the item and become narrative".to_owned(),
                },
                _ => change(Some(reference), found),
            };
            let message = format!(
                "{} would read differently: `{}` in it {what}{}",
                place(old_owner),
                source(&self.old.contents, &reference.span),
                self.cause(Some(reference), found)
            );
            return misread(self.line(reference.span.start), message);
        }
        for reference in new_references {
            if self.written(&reference.span) {
                continue;
            }
            let at = self.unmap(reference.span.start);
            if starting(old_references, at, |reference| reference.span.start).is_some() {
                continue;
            }
            let owner = holder(&self.new.document.items, reference.span.start);
            let message = format!(
                "{} would read differently: `{}` in it {}{}",
                place(owner),
                source(&self.new.contents, &reference.span),
                change(None, Some(reference)),
                self.cause(None, Some(reference))
            );
            return misread(self.old_line(reference.span.start), message);
        }
        Ok(())
    }

    /// Whether two references read alike: of one kind, leading to the same destination with the
    /// same title, and, for a footnote reference, to the same footnote definition.
    fn same_reading(&self, before: &Reference, after: &Reference) -> bool {
        let same_footnote = match (&before.definition, &after.definition) {
            _ if before.kind != ReferenceKind::Footnote => true,
            (None, None) => true,
            (Some(old), Some(new)) => {
                !self.inside_written(new.start) && self.unmap(new.start) == old.start
            }
            _ => false,
        };
        before.kind == after.kind
            && before.destination == after.destination
            && before.title == after.title
            && same_footnote
    }

    /// Every top-level block whose bytes the edit leaves alone still starts one where it moved, in the same item or narrative, and keeps its extent
    /// unless the edit writes right at one of its ends. No block starts where none did, away from
    /// the text the edit writes.
    fn blocks(&self) -> Checked {
        let old_blocks = &self.old.document.blocks;
        let new_blocks = &self.new.document.blocks;
        let owner = |document: &'r ParsedFile, at: usize| {
            holder(&document.document.items, at).map(|item| item.mid.as_str())
        };
        for block in old_blocks {
            if self.changed(block) {
                continue;
            }
            let start = self.map(block.start);
            let first = new_blocks.partition_point(|new| new.start < start);
            let mut starting_here = new_blocks[first..]
                .iter()
                .take_while(|new| new.start == start);
            let kept = match self.changed_at(block) {
                true => starting_here.next().is_some(),
                false => {
                    let end = self.map(block.end);
                    starting_here.any(|new| new.end == end)
                }
            };
            if kept && owner(self.old, block.start) == owner(self.new, start) {
                continue;
            }
            let message = "the text here would read differently: the block it is in, such as a \
                           paragraph, list, table or code block, would join another, grow, shrink \
                           or move into another item or narrative"
                .to_owned();
            return misread(self.line(block.start), message);
        }
        // A new block that holds no written text starts where an old block started: one whose
        // extent the loop above compared, or the rest of one that the edit changed.
        for block in new_blocks {
            if self.written(block) {
                continue;
            }
            let at = self.unmap(block.start);
            if starting(old_blocks, at, |block| block.start).is_none() {
                let message = "the edit would make this text read as a block of its own, for \
                               example by splitting a block or a link reference definition"
                    .to_owned();
                return misread(self.old_line(block.start), message);
            }
        }
        Ok(())
    }

    /// No parse problem appears outside an inserted item.
    fn problems(&self, inserted: &[(usize, usize, usize)]) -> Checked {
        let removed: Vec<(usize, usize)> = self
            .removed
            .iter()
            .map(|span| (self.line(span.start), self.line(span.end.saturating_sub(1))))
            .collect();
        let inserted: Vec<(usize, usize)> = inserted
            .iter()
            .map(|&(_, first, last)| (first, last))
            .collect();
        let mut problems: HashMap<&str, usize> = HashMap::new();
        for problem in self.old.document.diagnostics("") {
            if !within(problem.line, &removed) {
                *problems.entry(problem.code).or_default() += 1;
            }
        }
        let mut added = Vec::new();
        for problem in self.new.document.diagnostics("") {
            if within(problem.line, &inserted) {
                continue;
            }
            match problems.get_mut(problem.code) {
                Some(count) if *count > 0 => *count -= 1,
                _ => added.push(problem),
            }
        }
        // A missing separator is the cause of the other problems it brings, such as an empty
        // body of the item before it, so it is the one named.
        let named = match added.iter().position(|p| p.code == "missing-separator") {
            Some(index) => added.into_iter().nth(index),
            None => added.into_iter().next(),
        };
        let Some(problem) = named else {
            return Ok(());
        };
        let message = format!(
            "the edit would add a problem, `{}`: {}",
            problem.code, problem.message
        );
        let line = self
            .splices
            .first()
            .map_or(1, |splice| self.line(splice.range.start));
        misread(line, message)
    }

    /// Why a reference reads differently: the definition that decides it, when the edit removes,
    /// changes, adds or shadows one.
    fn cause(&self, before: Option<&Reference>, after: Option<&Reference>) -> String {
        let written = |reference: Option<&Reference>| {
            reference
                .and_then(|reference| reference.definition.clone())
                .filter(|definition| self.written(definition))
        };
        if let Some(definition) = before.and_then(|reference| reference.definition.clone()) {
            let label = label(&self.old.contents, &definition);
            let line = self.line(definition.start);
            let removed = self
                .removed
                .iter()
                .any(|span| span.start <= definition.start && definition.end <= span.end);
            if removed {
                return format!(
                    ", since the definition `[{label}]` it uses, at line {line}, is in the item \
                     being removed"
                );
            }
            if self.changed(&definition) {
                return format!(
                    ", since the edit changes the definition `[{label}]` at line {line} that it uses"
                );
            }
            if written(after).is_some() {
                return format!(
                    ", since a definition `[{label}]` that the edit writes would come first and \
                     shadow the one at line {line}"
                );
            }
            return String::new();
        }
        match written(after) {
            Some(definition) => format!(
                ", since a definition `[{}]` that the edit writes or changes would decide it",
                label(&self.new.contents, &definition)
            ),
            None => String::new(),
        }
    }
}

fn misread<T>(line: usize, message: String) -> Checked<T> {
    Err(Misread { line, message })
}

/// Whether a splice of `range` changes the bytes `span`: replaces any of them, or inserts
/// strictly inside.
fn crosses(range: &Range<usize>, span: &Range<usize>) -> bool {
    (range.start < span.end && span.start < range.end)
        || (range.is_empty() && span.start < range.start && range.start < span.end)
}

/// Whether `bytes` hold only blank lines, spaces, tabs and thematic breaks.
fn spacing(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    text.split(['\n', '\r']).all(|line| {
        let marks: Vec<char> = line.chars().filter(|c| !matches!(c, ' ' | '\t')).collect();
        marks.is_empty()
            || (marks.len() >= 3
                && matches!(marks[0], '-' | '*' | '_')
                && marks.iter().all(|&c| c == marks[0]))
    })
}

/// The index of the element of `elements`, in source order, that starts at byte `at`.
fn starting<T>(elements: &[T], at: usize, start: impl Fn(&T) -> usize) -> Option<usize> {
    let index = elements.partition_point(|element| start(element) < at);
    (index < elements.len() && start(&elements[index]) == at).then_some(index)
}

/// The item of `items`, in source order, whose bytes hold byte `at`.
fn holder(items: &[Item], at: usize) -> Option<&Item> {
    let after = items.partition_point(|item| item.span.start <= at);
    items[..after].last().filter(|item| at < item.span.end)
}

/// The bytes of an item's anchor.
fn anchor(item: &Item) -> Range<usize> {
    item.span.start..item.span.start + format!("<a id=\"{}\"></a>", item.mid).len()
}

/// Every metadata row of `items`, with its item, in source order.
fn rows_of(items: &[Item]) -> Vec<(&Item, &Row)> {
    items
        .iter()
        .flat_map(|item| item.rows.iter().map(move |row| (item, row)))
        .collect()
}

/// Whether `line` lies in one of `spans`, in order and apart, of first and last lines.
fn within(line: usize, spans: &[(usize, usize)]) -> bool {
    let after = spans.partition_point(|&(first, _)| first <= line);
    spans[..after].last().is_some_and(|&(_, last)| line <= last)
}

fn named(item: &Item) -> String {
    format!("the item `{}` with MID {}", item.title, item.mid)
}

fn place(owner: Option<&Item>) -> String {
    match owner {
        Some(item) => named(item),
        None => "the narrative".to_owned(),
    }
}

fn source(contents: &[u8], span: &Range<usize>) -> String {
    String::from_utf8_lossy(&contents[span.clone()]).into_owned()
}

/// What a link reads as, apart from where it is: its text, path and fragment.
type LinkReading<'l> = Option<(&'l str, &'l str, Option<&'l str>)>;

/// What a row reads as, apart from where it is: its key, value, text and link.
type RowReading<'r> = (&'r str, &'r str, &'r str, LinkReading<'r>);

fn link_reading(row: &Row) -> LinkReading<'_> {
    let link = row.link.as_ref()?;
    Some((
        link.text.as_str(),
        link.path.as_str(),
        link.fragment.as_deref(),
    ))
}

fn row_reading(row: &Row) -> RowReading<'_> {
    (&row.key, &row.value, &row.text, link_reading(row))
}

/// How a reference changes.
fn change(before: Option<&Reference>, after: Option<&Reference>) -> String {
    let noun = |reference: &Reference| match reference.kind {
        ReferenceKind::Link => "a link",
        ReferenceKind::Image => "an image",
        ReferenceKind::Footnote => "a footnote reference",
    };
    match (before, after) {
        (Some(before), Some(after)) if before.kind == after.kind => match before.kind {
            ReferenceKind::Footnote => "would refer to another footnote".to_owned(),
            _ if before.destination == after.destination => {
                format!("would change its title to `{}`", after.title)
            }
            _ => format!(
                "would lead to `{}` instead of `{}`",
                after.destination, before.destination
            ),
        },
        (Some(before), Some(after)) => {
            format!("would become {} instead of {}", noun(after), noun(before))
        }
        (Some(before), None) => format!("would no longer be {}", noun(before)),
        (None, Some(after)) => format!("would become {} to `{}`", noun(after), after.destination),
        (None, None) => "would read differently".to_owned(),
    }
}

/// The label of the definition at `definition`.
fn label(contents: &[u8], definition: &Range<usize>) -> String {
    let text = String::from_utf8_lossy(&contents[definition.clone()]);
    let label = text
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("]:"))
        .map_or(text.as_ref(), |(label, _)| label);
    label.to_owned()
}

/// The metadata rows, as (key, value) and sorted, that `item` reads with once the row changes
/// among `splices` are made; `None` when they change no row.
fn expected_rows(item: &Item, splices: &[&Splice]) -> Option<Vec<(String, String)>> {
    if splices.is_empty() {
        return None;
    }
    let mut rows: Vec<Option<(String, String)>> = item
        .rows
        .iter()
        .map(|row| Some((row.key.clone(), row.value.clone())))
        .collect();
    let index = |at: usize| starting(&item.rows, at, |row| row.span.start);
    for splice in splices {
        let Expect::Row(change) = &splice.expect else {
            continue;
        };
        match change {
            RowChange::Set { at, value } => {
                if let Some(Some((_, old))) = index(*at).map(|index| &mut rows[index]) {
                    old.clone_from(value);
                }
            }
            RowChange::Add { key, value } => rows.push(Some((key.clone(), value.clone()))),
            RowChange::Remove { at } => {
                if let Some(index) = index(*at) {
                    rows[index] = None;
                }
            }
        }
    }
    let mut rows: Vec<(String, String)> = rows.into_iter().flatten().collect();
    rows.sort();
    Some(rows)
}

fn sorted_rows(item: &Item) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = item
        .rows
        .iter()
        .map(|row| (row.key.clone(), row.value.clone()))
        .collect();
    rows.sort();
    rows
}

/// The bytes before the first Markdown line, a byte order mark and front matter, without the
/// line ending of the front matter's closing line, which an edit may add.
fn markdown_start(contents: &[u8]) -> &[u8] {
    let start = match std::str::from_utf8(contents) {
        Ok(text) => front_matter(text).0,
        Err(_) => 0,
    };
    contents[..start].trim_ascii_end()
}
