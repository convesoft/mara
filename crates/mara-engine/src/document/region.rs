//! The event pass: top-level blocks grouped into regions, each read as an item or as narrative.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Tag, TagEnd};

use super::html::{HtmlBlock, HtmlLine};
use super::{Document, Item, LineIndex, Link, Problem, Row, is_mid, label_name, lines};

/// Reads the events of one document in order.
pub(super) struct Reader<'t> {
    text: &'t str,
    lines: LineIndex,
    /// Open block tags: 0 between top-level blocks.
    depth: usize,
    block: Option<OpenBlock>,
    /// The finished top-level blocks of the current region.
    region: Vec<Block>,
    /// A thematic break inside the open top-level block ends the region after that block.
    break_pending: bool,
    document: Document,
}

/// A finished top-level block.
struct Block {
    kind: Kind,
    /// Without trailing blank lines.
    span: Range<usize>,
    /// Every link in the block, in source order.
    links: Vec<Link>,
    /// The MID anchors in the block, with their offsets, unless the block is one.
    anchors: Vec<(String, usize)>,
}

enum Kind {
    /// A paragraph that is exactly `<a id="<MID>"></a>`.
    Anchor(String),
    /// A paragraph of inline HTML only, without a MID anchor.
    Html,
    /// Any other paragraph.
    Paragraph,
    Heading(String),
    /// A table among the first blocks of its region, where a metadata table can be.
    Table(Vec<TableRow>),
    Other,
}

struct TableRow {
    span: Range<usize>,
    /// The cells written in the source, up to the table's column count.
    cells: Vec<Cell>,
    /// The source has more cells than the table has columns; the parser drops them.
    excess: bool,
}

impl TableRow {
    /// Key and value: the first two cells, when both have text (DES-METADATA-TABLE).
    fn key_value(&self) -> Option<(&Cell, &Cell)> {
        match self.cells.as_slice() {
            [key, value, ..] if !key.text.is_empty() && !value.text.is_empty() => {
                Some((key, value))
            }
            _ => None,
        }
    }

    fn is_type_row(&self) -> bool {
        self.key_value()
            .is_some_and(|(key, _)| label_name(&key.text) == "type")
    }
}

struct Cell {
    /// Without surrounding spaces.
    span: Range<usize>,
    /// As written, with `\|` read as `|`.
    text: String,
    /// The indices of the cell's links in [`Block::links`].
    links: Range<usize>,
}

struct OpenBlock {
    kind: OpenKind,
    span: Range<usize>,
    links: Vec<Link>,
    open_links: Vec<OpenLink>,
    /// The MID anchors `<a id="<MID>"></a>` in the block, with their offsets.
    anchors: Vec<(String, usize)>,
    /// The HTML block being read, if any.
    html: HtmlBlock,
}

/// A link whose end has not been seen.
struct OpenLink {
    text: String,
    destination: String,
    span: Range<usize>,
    collapsed: bool,
}

enum OpenKind {
    Paragraph {
        html: String,
        html_only: bool,
    },
    Heading(String),
    Table {
        rows: Vec<TableRow>,
        row: Option<TableRow>,
        /// The open cell's span and the index of its first link.
        cell: Option<(Range<usize>, usize)>,
    },
    Other,
}

impl OpenBlock {
    /// Text that is not inline HTML, in an inline context.
    fn not_html(&mut self) {
        if let OpenKind::Paragraph { html_only, .. } = &mut self.kind {
            *html_only = false;
        }
    }

    fn text(&mut self, text: &str) {
        self.not_html();
        if let OpenKind::Heading(title) = &mut self.kind {
            title.push_str(text);
        }
        if let Some(link) = self.open_links.last_mut() {
            link.text.push_str(text);
        }
    }

    fn finish(self) -> Block {
        let kind = match self.kind {
            OpenKind::Paragraph { html, html_only } => {
                let exact = html
                    .strip_prefix("<a id=\"")
                    .and_then(|rest| rest.strip_suffix("\"></a>"))
                    .filter(|mid| html_only && is_mid(mid));
                match exact {
                    Some(mid) => Kind::Anchor(mid.to_owned()),
                    None if html_only && self.anchors.is_empty() && !html.is_empty() => Kind::Html,
                    None => Kind::Paragraph,
                }
            }
            OpenKind::Heading(title) => Kind::Heading(title),
            OpenKind::Table { rows, .. } => Kind::Table(rows),
            OpenKind::Other => Kind::Other,
        };
        let anchors = match kind {
            Kind::Anchor(_) => Vec::new(),
            _ => self.anchors,
        };
        Block {
            kind,
            span: self.span,
            links: self.links,
            anchors,
        }
    }
}

/// Where the parts of an item are among the first blocks of a region (DES-ITEM-STRUCTURE).
struct Head {
    anchor: Option<Anchor>,
    heading: Option<usize>,
    /// The metadata table: the table after the heading's place, or in it.
    table: Option<usize>,
}

/// The MID anchor that starts a region: a paragraph of its own, or the one anchor merged into
/// the region's first paragraph.
struct Anchor {
    mid: String,
    offset: usize,
    /// The anchor is a paragraph of its own.
    exact: bool,
}

impl Head {
    fn of(blocks: &[Block]) -> Head {
        let kind = |index: usize| blocks.get(index).map(|block| &block.kind);
        let anchor = match blocks.first() {
            Some(Block {
                kind: Kind::Anchor(mid),
                span,
                ..
            }) => Some(Anchor {
                mid: mid.clone(),
                offset: span.start,
                exact: true,
            }),
            Some(Block {
                kind: Kind::Paragraph,
                anchors,
                ..
            }) if anchors.len() == 1 => Some(Anchor {
                mid: anchors[0].0.clone(),
                offset: anchors[0].1,
                exact: false,
            }),
            _ => None,
        };
        // The heading's place: the first block when it is a heading, else the block after an
        // anchor or a paragraph of inline HTML.
        let first = match kind(0) {
            Some(Kind::Anchor(_) | Kind::Html) => 1,
            Some(Kind::Paragraph) => usize::from(anchor.is_some()),
            _ => 0,
        };
        let heading = matches!(kind(first), Some(Kind::Heading(_))).then_some(first);
        // Another block in the heading's place leaves the table after it in its place.
        let at = first + usize::from(!matches!(kind(first), Some(Kind::Table(_))));
        let table = matches!(kind(at), Some(Kind::Table(_))).then_some(at);
        Head {
            anchor,
            heading,
            table,
        }
    }

    /// Whether the region is meant as an item: it starts with a MID anchor, or, after an
    /// optional paragraph of inline HTML, with a heading and a table with a `Type` row.
    fn is_item(&self, blocks: &[Block]) -> bool {
        let type_row = self.table.is_some_and(|table| match &blocks[table].kind {
            Kind::Table(rows) => rows.iter().any(TableRow::is_type_row),
            _ => false,
        });
        self.anchor.is_some() || (self.heading.is_some() && type_row)
    }
}

impl<'t> Reader<'t> {
    pub(super) fn new(text: &'t str, lines: LineIndex) -> Reader<'t> {
        Reader {
            text,
            lines,
            depth: 0,
            block: None,
            region: Vec::new(),
            break_pending: false,
            document: Document::default(),
        }
    }

    pub(super) fn event(&mut self, event: Event<'_>, span: Range<usize>) {
        match event {
            Event::Start(tag) => {
                if self.depth == 0 {
                    if self.break_pending {
                        self.end_region(Some(span.start));
                    }
                    self.block = Some(self.open(&tag, span.clone()));
                }
                self.depth += 1;
                self.start(tag, span);
            }
            Event::End(tag) => {
                self.end(tag, span);
                self.depth = self.depth.saturating_sub(1);
                if self.depth == 0
                    && let Some(block) = self.block.take()
                {
                    self.push(block);
                }
            }
            Event::Rule if self.depth == 0 => self.end_region(Some(span.start)),
            Event::Rule => self.break_pending = true,
            Event::Text(text) | Event::Code(text) => {
                if let Some(block) = &mut self.block {
                    block.text(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(block) = &mut self.block {
                    block.text(" ");
                }
            }
            Event::InlineHtml(html) => {
                if let Some(block) = &mut self.block {
                    // Only the full spelling `<a id="<MID>"></a>` is an anchor (DES-MID-ANCHOR).
                    let mid = html
                        .strip_prefix("<a id=\"")
                        .and_then(|rest| rest.strip_suffix("\">"))
                        .filter(|mid| is_mid(mid) && self.text[span.end..].starts_with("</a>"));
                    if let Some(mid) = mid {
                        block.anchors.push((mid.to_owned(), span.start));
                    }
                    if let OpenKind::Paragraph { html: all, .. } = &mut block.kind {
                        all.push_str(&html);
                    }
                }
            }
            Event::Html(html) => {
                let mut end_tags = Vec::new();
                if let Some(block) = &mut self.block {
                    let mut offset = span.start;
                    for line in lines(&html) {
                        match block.html.line(line) {
                            HtmlLine::Anchor(mid) => block.anchors.push((mid.to_owned(), offset)),
                            HtmlLine::EndTagCase(end) => end_tags.push((end, offset)),
                            HtmlLine::Other => {}
                        }
                        offset += line.len();
                    }
                }
                for (end, offset) in end_tags {
                    let message = format!(
                        "write this end tag in lowercase, `{end}`: Mara's Markdown parser ends \
                         the HTML block only there, so it reads what follows as HTML, unlike \
                         GitHub"
                    );
                    self.problem("html-end-tag-case", offset, message);
                }
            }
            _ => {
                if let Some(block) = &mut self.block {
                    block.not_html();
                }
            }
        }
    }

    pub(super) fn finish(mut self) -> Document {
        if let Some(block) = self.block.take() {
            self.push(block);
        }
        self.end_region(None);
        self.document.problems.sort_by_key(|problem| problem.line);
        self.document
    }

    fn open(&self, tag: &Tag<'_>, span: Range<usize>) -> OpenBlock {
        let kind = match tag {
            Tag::Paragraph => OpenKind::Paragraph {
                html: String::new(),
                html_only: true,
            },
            Tag::Heading { .. } => OpenKind::Heading(String::new()),
            Tag::Table(_) if self.region.len() <= 2 => OpenKind::Table {
                rows: Vec::new(),
                row: None,
                cell: None,
            },
            _ => OpenKind::Other,
        };
        OpenBlock {
            kind,
            span,
            links: Vec::new(),
            open_links: Vec::new(),
            anchors: Vec::new(),
            html: HtmlBlock::default(),
        }
    }

    fn start(&mut self, tag: Tag<'_>, span: Range<usize>) {
        let nested = self.depth > 1;
        let Some(block) = &mut self.block else {
            return;
        };
        if nested {
            block.not_html();
        }
        match (tag, &mut block.kind) {
            (
                Tag::Link {
                    link_type,
                    dest_url,
                    ..
                },
                _,
            ) => block.open_links.push(OpenLink {
                text: String::new(),
                destination: dest_url.into_string(),
                span,
                collapsed: link_type == LinkType::Collapsed,
            }),
            (Tag::HtmlBlock, _) => block.html = HtmlBlock::default(),
            (Tag::TableHead | Tag::TableRow, OpenKind::Table { row, .. }) => {
                *row = Some(TableRow {
                    span,
                    cells: Vec::new(),
                    excess: false,
                });
            }
            (Tag::TableCell, OpenKind::Table { cell, .. }) => {
                *cell = Some((span, block.links.len()));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd, span: Range<usize>) {
        if let TagEnd::Heading(_) = tag {
            self.check_setext(&span);
        }
        let text = self.text;
        let Some(block) = &mut self.block else {
            return;
        };
        match (tag, &mut block.kind) {
            (TagEnd::Link, _) => {
                if let Some(mut link) = block.open_links.pop() {
                    // The parser's span of a collapsed reference `[label][]` ends before `[]`.
                    if link.collapsed && text[link.span.end..].starts_with("[]") {
                        link.span.end += 2;
                    }
                    let line = self.lines.line(link.span.start);
                    let link = Link::new(link.text, &link.destination, link.span, line);
                    block.links.push(link);
                }
            }
            (TagEnd::TableCell, OpenKind::Table { row, cell, .. }) => {
                if let (Some(row), Some((span, first_link))) = (row, cell.take()) {
                    // The parser adds empty cells at the end of the row for missing ones.
                    if span.start < row.span.end {
                        let span = trim(text, &span);
                        row.cells.push(Cell {
                            text: text[span.clone()].replace("\\|", "|"),
                            span,
                            links: first_link..block.links.len(),
                        });
                    }
                }
            }
            (TagEnd::TableHead | TagEnd::TableRow, OpenKind::Table { rows, row, .. }) => {
                if let Some(mut row) = row.take() {
                    let last_end = row
                        .cells
                        .last()
                        .map_or(row.span.start, |cell| cell.span.end);
                    let rest = &text[last_end.min(row.span.end)..row.span.end];
                    let rest = rest.trim_start_matches([' ', '\t']);
                    row.excess = !rest.strip_prefix('|').unwrap_or(rest).trim().is_empty();
                    rows.push(row);
                }
            }
            _ => {}
        }
    }

    /// Reports a setext heading: its underline is the last line of its source (DES-REGIONS).
    fn check_setext(&mut self, span: &Range<usize>) {
        let source = self.text[span.clone()].trim_end_matches(['\n', '\r']);
        if let Some(end) = source.rfind(['\n', '\r']) {
            let underline = span.start + end + 1;
            self.problem(
                "setext-heading",
                underline,
                "a line of `-` or `=` directly under text makes a setext heading; put a blank \
                 line above it, or write the heading with `#`"
                    .to_owned(),
            );
        }
    }

    /// Adds a finished top-level block to the region, reporting the MID anchors it holds in a
    /// place other than a paragraph of their own.
    fn push(&mut self, open: OpenBlock) {
        let mut block = open.finish();
        block.span = trim(self.text, &block.span);
        for (mid, offset) in &block.anchors {
            let message = format!(
                "the anchor of MID {mid} must be a paragraph of its own, `<a id=\"{mid}\"></a>`, \
                 at the start of a region"
            );
            self.problem("merged-anchor", *offset, message);
        }
        if let Kind::Anchor(mid) = &block.kind
            && !self.region.is_empty()
        {
            self.problem(
                "missing-separator",
                block.span.start,
                format!(
                    "the anchor of MID {mid} is not the first block of its region; put a `---` \
                     line, with a blank line on each side, before it"
                ),
            );
            // Read on as if the separator were there, so that the item still loads.
            self.end_region(Some(block.span.start));
        }
        self.region.push(block);
    }

    /// Reads the finished region. `next` is where the next region starts; `None` at the end
    /// of the document.
    fn end_region(&mut self, next: Option<usize>) {
        self.break_pending = false;
        let blocks = std::mem::take(&mut self.region);
        let head = Head::of(&blocks);
        if head.is_item(&blocks) {
            self.read_item(head, blocks, next);
        } else {
            self.read_mentions(blocks, None);
        }
    }

    /// Adds the mentions of `blocks`, except those in the block at `table`, to the narrative.
    fn read_mentions(&mut self, blocks: Vec<Block>, table: Option<usize>) {
        for (index, block) in blocks.into_iter().enumerate() {
            if Some(index) != table {
                let mentions = block.links.into_iter().filter(Link::is_mention);
                self.document.mentions.extend(mentions);
            }
        }
    }

    /// Reads a region meant as an item, reporting each problem of its structure, and keeps it
    /// as an item when it has an anchor, a heading and a `Type` row (DES-ITEM-STRUCTURE).
    fn read_item(&mut self, head: Head, blocks: Vec<Block>, next: Option<usize>) {
        // An item starts at its anchor, even one merged into its first block.
        let start = head
            .anchor
            .as_ref()
            .map_or(blocks[0].span.start, |anchor| anchor.offset);
        if let Some(Anchor {
            mid, exact: true, ..
        }) = &head.anchor
            && !self.blank_line_follows(blocks[0].span.end)
        {
            let message = format!("the anchor of MID {mid} needs a blank line after it");
            self.problem("merged-anchor", start, message);
        }
        let title = head.heading.map(|index| match &blocks[index].kind {
            Kind::Heading(title) => title.clone(),
            _ => String::new(),
        });
        let name = match (&title, &head.anchor) {
            (Some(title), _) => format!("`{title}`"),
            (None, Some(anchor)) => format!("with MID {}", anchor.mid),
            (None, None) => String::new(),
        };
        let rows = match head.table.map(|index| (index, &blocks[index])) {
            Some((
                index,
                table @ Block {
                    kind: Kind::Table(rows),
                    ..
                },
            )) => {
                let after = blocks.get(index + 1).map(|block| block.span.start);
                let rows = self.metadata_rows(table, rows, after.or(next));
                if after.is_none() {
                    let message = format!("the item {name} has no body after its metadata table");
                    self.problem("empty-body", start, message);
                }
                rows
            }
            _ => Vec::new(),
        };

        let mut missing = Vec::new();
        match &head.anchor {
            // An anchor merged into another block is reported as `merged-anchor` already.
            Some(_) => {}
            None if !blocks[0].anchors.is_empty() => {}
            None if matches!(blocks[0].kind, Kind::Html) => missing.push(
                "the HTML before its heading is not a MID anchor, `<a id=\"<MID>\"></a>` with a \
                 26-character uppercase ULID",
            ),
            None => missing.push("it has no MID anchor before its heading"),
        }
        if title.is_none() {
            missing.push("it has no heading after its anchor");
        }
        if !rows.iter().any(|row| row.name == "type") {
            missing.push("it has no metadata table with a `Type` row after its heading");
        }
        let (Some(Anchor { mid, .. }), Some(title), Some(table), true) =
            (&head.anchor, title, head.table, missing.is_empty())
        else {
            if !missing.is_empty() {
                let message = format!("the item {name} is not read: {}", missing.join("; "));
                self.problem("incomplete-item", start, message);
            }
            return self.read_mentions(blocks, head.table);
        };

        let end = blocks.last().map_or(start, |block| block.span.end);
        let body = match blocks.get(table + 1) {
            Some(first) => first.span.start..end,
            None => end..end,
        };
        let mid = mid.clone();
        let mut mentions = Vec::new();
        for (index, block) in blocks.into_iter().enumerate() {
            if index != table {
                for link in block.links.into_iter().filter(Link::is_mention) {
                    // Text before a merged anchor is narrative.
                    if link.span.start < start {
                        self.document.mentions.push(link);
                    } else {
                        mentions.push(link);
                    }
                }
            }
        }
        self.document.items.push(Item {
            mid,
            title,
            rows,
            mentions,
            span: start..end,
            line: self.lines.line(start),
            body,
        });
    }

    /// The rows of the metadata table `table` that have a key and a value, reporting malformed
    /// ones. `next` is where the block after the table starts, if any (DES-METADATA-TABLE).
    fn metadata_rows(&mut self, table: &Block, rows: &[TableRow], next: Option<usize>) -> Vec<Row> {
        let mut read = Vec::with_capacity(rows.len());
        for row in rows {
            let Some((key, value)) = row.key_value() else {
                let message = "this metadata row lacks a key or a value: a row is \
                               `| Key | Value |`, and the table ends only at a blank line"
                    .to_owned();
                self.problem("malformed-row", row.span.start, message);
                continue;
            };
            if row.cells.len() > 2 || row.excess {
                let message = "this metadata row has more than two cells, and only its first \
                               two are read; write `|` inside a value as `\\|`"
                    .to_owned();
                self.problem("malformed-row", row.span.start, message);
            }
            let link = (value.links.len() == 1)
                .then(|| &table.links[value.links.start])
                .filter(|link| link.span == value.span)
                .cloned();
            read.push(Row {
                key: key.text.clone(),
                name: label_name(&key.text),
                value: value.text.clone(),
                link,
                line: self.lines.line(row.span.start),
            });
        }
        if let Some(next) = next
            && self.lines.line(next) == self.lines.line(table.span.end.saturating_sub(1)) + 1
        {
            let message =
                "the metadata table ends at a blank line, but this line follows it directly"
                    .to_owned();
            self.problem("malformed-row", next, message);
        }
        read
    }

    /// Whether the line after the one that holds byte `offset` exists and is blank.
    fn blank_line_follows(&self, offset: usize) -> bool {
        let rest = &self.text[offset..];
        let Some(end) = rest.find(['\n', '\r']) else {
            return false;
        };
        let after = &rest[end..];
        let after = after.strip_prefix("\r\n").unwrap_or(&after[1..]);
        let line = &after[..after.find(['\n', '\r']).unwrap_or(after.len())];
        !after.is_empty() && line.trim_matches([' ', '\t']).is_empty()
    }

    fn problem(&mut self, code: &'static str, offset: usize, message: String) {
        let line = self.lines.line(offset);
        self.document.problems.push(Problem {
            code,
            message,
            line,
        });
    }
}

/// `span` without the spaces, tabs and line endings around its text.
fn trim(text: &str, span: &Range<usize>) -> Range<usize> {
    let source = &text[span.clone()];
    let blank = [' ', '\t', '\r', '\n'];
    let start = span.start + (source.len() - source.trim_start_matches(blank).len());
    let end = span.end - (source.len() - source.trim_end_matches(blank).len());
    start..end.max(start)
}
