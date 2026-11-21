//! The event pass: top-level blocks grouped into regions, each read as an item or as narrative.

use std::collections::BTreeMap;
use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Tag, TagEnd};

use super::html::{HtmlBlock, HtmlLine};
use super::{
    Anchorless, Document, Item, LineIndex, Link, Node, Posting, Problem, Row, Unit, Word, Zone,
    is_mid, label_name, lines, word_spans,
};

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
    /// The search words of the nodes read so far, with their postings.
    words: BTreeMap<String, Vec<Posting>>,
    document: Document,
}

/// A finished top-level block.
struct Block {
    kind: Kind,
    /// Without trailing blank lines.
    span: Range<usize>,
    /// Every link in the block, in source order.
    links: Vec<Link>,
    /// The MID anchors in the block, unless the block is one.
    anchors: Vec<BlockAnchor>,
    /// The block's text, without HTML, link destinations or the text of nested headings.
    plain: Prose,
    /// The text of the headings inside the block, such as one in a block quote or a list.
    headings: Prose,
}

/// A MID anchor inside a block: its MID, its offset, and how much of the block's text precedes
/// it.
struct BlockAnchor {
    mid: String,
    offset: usize,
    before: usize,
}

/// Text without Markdown syntax, link destinations or HTML, with where each piece of it starts
/// in the source, so that each word has a source offset (DES-SNAPSHOT).
#[derive(Clone, Default)]
struct Prose {
    text: String,
    /// Each piece of `text` in order: where it starts in `text`, and its range in the source.
    marks: Vec<Mark>,
}

/// A piece of collected text: where it starts in the text, the source range it was read from,
/// and whether that range holds exactly its characters.
#[derive(Clone, Copy, Debug)]
struct Mark {
    at: usize,
    source: (usize, usize),
    exact: bool,
}

impl Prose {
    /// The piece `text`, read from `source`, which holds exactly its characters when `exact`.
    fn push(&mut self, text: &str, source: Range<usize>, exact: bool) {
        self.marks.push(Mark {
            at: self.text.len(),
            source: (source.start, source.end),
            exact,
        });
        self.text.push_str(text);
    }

    /// A space that ends the word before it.
    fn space(&mut self) {
        self.text.push(' ');
    }

    /// The text up to byte `end`, with its marks.
    fn until(&self, end: usize) -> Words<'_> {
        Words {
            text: &self.text[..end],
            start: 0,
            marks: &self.marks,
        }
    }

    /// The text from byte `start` on, with its marks.
    fn after(&self, start: usize) -> Words<'_> {
        Words {
            text: &self.text,
            start,
            marks: &self.marks,
        }
    }

    fn words(&self) -> Words<'_> {
        self.until(self.text.len())
    }
}

/// A text whose words are indexed, with the source offset of each of its pieces.
#[derive(Clone, Copy)]
struct Words<'t> {
    text: &'t str,
    /// Where the indexed part of `text` starts.
    start: usize,
    /// The pieces of `text`, in order.
    marks: &'t [Mark],
}

impl Words<'_> {
    /// The source offset of byte `at` of the text, where a word starts: in an exact piece, at the
    /// same distance from the start of its source range as in the text; in any other piece, at
    /// the start of its source range.
    fn source(&self, at: usize) -> usize {
        let piece = self.marks.partition_point(|mark| mark.at <= at);
        // Cannot fire: text grows only by `Prose::push`, which marks it, and by spaces, which
        // hold no word, so a word starts at or after the first mark.
        let mark = self.marks[piece.checked_sub(1).expect("a word lies in a piece")];
        let (start, end) = mark.source;
        let distance = at - mark.at;
        match mark.exact && distance < end - start {
            true => start + distance,
            false => start,
        }
    }
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
    /// The cell's text without Markdown syntax, link destinations or HTML.
    plain: Prose,
    /// The indices of the cell's links in [`Block::links`].
    links: Range<usize>,
}

struct OpenBlock {
    kind: OpenKind,
    span: Range<usize>,
    links: Vec<Link>,
    open_links: Vec<OpenLink>,
    /// The MID anchors `<a id="<MID>"></a>` in the block.
    anchors: Vec<BlockAnchor>,
    /// The HTML block being read, if any.
    html: HtmlBlock,
    /// The block's text, with a space at each boundary inside it.
    plain: Prose,
    /// The text of the headings nested in the block.
    headings: Prose,
    /// A nested heading is open.
    in_heading: bool,
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
        /// The open cell's text.
        cell_plain: Prose,
    },
    Other,
}

impl OpenBlock {
    /// A boundary inside the block, which separates the words on either side of it.
    /// Every text being collected gets it: the block's, and an open metadata cell's.
    fn boundary(&mut self) {
        self.prose().space();
        if let OpenKind::Table {
            cell: Some(_),
            cell_plain,
            ..
        } = &mut self.kind
        {
            cell_plain.space();
        }
    }

    /// Text that is not inline HTML, in an inline context.
    fn not_html(&mut self) {
        if let OpenKind::Paragraph { html_only, .. } = &mut self.kind {
            *html_only = false;
        }
    }

    /// The text being collected: a nested heading's, or the block's own.
    fn prose(&mut self) -> &mut Prose {
        if self.in_heading {
            &mut self.headings
        } else {
            &mut self.plain
        }
    }

    /// The MID anchor `mid` at `offset`, after the text collected so far.
    fn anchor(&mut self, mid: &str, offset: usize) {
        let before = self.plain.text.len();
        self.anchors.push(BlockAnchor {
            mid: mid.to_owned(),
            offset,
            before,
        });
    }

    /// `rendered`, the text the Markdown parser read, in the runs `runs` with their source
    /// ranges ([`align`]). The runs together are the whole rendered text.
    fn text(&mut self, rendered: &str, runs: &[Run]) {
        self.not_html();
        for run in runs {
            let piece = &rendered[run.rendered.clone()];
            self.prose().push(piece, run.source.clone(), run.exact);
            if let OpenKind::Table {
                cell: Some(_),
                cell_plain,
                ..
            } = &mut self.kind
            {
                cell_plain.push(piece, run.source.clone(), run.exact);
            }
        }
        if let OpenKind::Heading(title) = &mut self.kind {
            title.push_str(rendered);
        }
        if let Some(link) = self.open_links.last_mut() {
            link.text.push_str(rendered);
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
            plain: self.plain,
            headings: self.headings,
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
    /// How much of the first block's text precedes it.
    before: usize,
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
                before: 0,
            }),
            Some(Block {
                kind: Kind::Paragraph,
                anchors,
                ..
            }) if anchors.len() == 1 => Some(Anchor {
                mid: anchors[0].mid.clone(),
                offset: anchors[0].offset,
                exact: false,
                before: anchors[0].before,
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
            words: BTreeMap::new(),
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
            Event::Rule => {
                self.break_pending = true;
                if let Some(block) = &mut self.block {
                    block.boundary();
                }
            }
            Event::Text(text) => {
                let runs = align(self.text, &text, span, false);
                if let Some(block) = &mut self.block {
                    block.text(&text, &runs);
                }
            }
            Event::Code(text) => {
                // The span holds the backtick runs around the code.
                let source = &self.text[span.clone()];
                let opening = source.bytes().take_while(|&b| b == b'`').count();
                let closing = source[opening..]
                    .bytes()
                    .rev()
                    .take_while(|&b| b == b'`')
                    .count();
                let content = span.start + opening..span.end - closing;
                // A code span over several lines is one run at its start (DES-SEARCH-REQUESTS).
                let runs = match source.contains(['\n', '\r']) {
                    true => vec![inexact(0..text.len(), span.start..span.start + 1)],
                    false => align(self.text, &text, content, true),
                };
                if let Some(block) = &mut self.block {
                    block.text(&text, &runs);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(block) = &mut self.block {
                    block.text(" ", &[inexact(0..1, span)]);
                }
            }
            Event::InlineHtml(html) => {
                if let Some(block) = &mut self.block {
                    block.boundary();
                    // Only the full spelling `<a id="<MID>"></a>` is an anchor (DES-MID-ANCHOR).
                    let mid = html
                        .strip_prefix("<a id=\"")
                        .and_then(|rest| rest.strip_suffix("\">"))
                        .filter(|mid| is_mid(mid) && self.text[span.end..].starts_with("</a>"));
                    if let Some(mid) = mid {
                        block.anchor(mid, span.start);
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
                            HtmlLine::Anchor(mid) => block.anchor(mid, offset),
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
        self.document.words = std::mem::take(&mut self.words)
            .into_iter()
            .map(|(text, postings)| Word { text, postings })
            .collect();
        self.document
    }

    fn open(&self, tag: &Tag<'_>, span: Range<usize>) -> OpenBlock {
        let kind = match tag {
            Tag::Paragraph => OpenKind::Paragraph {
                html: String::new(),
                html_only: true,
            },
            Tag::Heading { .. } => OpenKind::Heading(String::new()),
            // A table after a heading may make an item that lacks its separator.
            Tag::Table(_) if self.region.len() <= 2 || self.after_heading() => OpenKind::Table {
                rows: Vec::new(),
                row: None,
                cell: None,
                cell_plain: Prose::default(),
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
            plain: Prose::default(),
            headings: Prose::default(),
            in_heading: false,
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
        if !is_inline(&tag) {
            block.boundary();
        }
        // A heading inside another block, such as a block quote or a list, collects its text
        // apart, for the heading zone (DES-SNAPSHOT).
        if nested && matches!(tag, Tag::Heading { .. }) {
            block.in_heading = true;
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
        if !is_inline_end(&tag) {
            block.boundary();
        }
        if matches!(tag, TagEnd::Heading(_)) && block.in_heading {
            block.in_heading = false;
            block.boundary();
        }
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
            (
                TagEnd::TableCell,
                OpenKind::Table {
                    row,
                    cell,
                    cell_plain,
                    ..
                },
            ) => {
                let plain = std::mem::take(cell_plain);
                if let (Some(row), Some((span, first_link))) = (row, cell.take()) {
                    // The parser adds empty cells at the end of the row for missing ones.
                    if span.start < row.span.end {
                        let span = trim(text, &span);
                        row.cells.push(Cell {
                            text: text[span.clone()].replace("\\|", "|"),
                            plain,
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
        self.document.blocks.push(block.span.clone());
        for BlockAnchor { mid, offset, .. } in &block.anchors {
            let message = format!(
                "the anchor of MID {mid} must be a paragraph of its own, `<a id=\"{mid}\"></a>`, \
                 at the start of a region"
            );
            self.problem("merged-anchor", *offset, message);
        }
        if let Kind::Table(rows) = &block.kind
            && is_item_table(rows)
            && self.after_heading()
            && Head::of(&self.region).heading != Some(self.region.len() - 1)
            && self.region[self.region.len() - 2].anchors.is_empty()
        {
            self.split_before_heading();
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

    /// Starts a new region at the region's last block, a heading followed by an item's table
    /// that lacks the `---` before it, or at the paragraph of inline HTML right before that
    /// heading, which stands in the anchor's place (DES-REGIONS). Reports `missing-separator`
    /// where the `---` belongs.
    fn split_before_heading(&mut self) {
        let heading = self.region.pop().expect("a heading precedes the table");
        let html = match self.region.last() {
            Some(Block {
                kind: Kind::Html, ..
            }) if self.region.len() >= 2 => self.region.pop(),
            _ => None,
        };
        let title = match &heading.kind {
            Kind::Heading(title) => title.as_str(),
            _ => "",
        };
        let (start, place) = match &html {
            Some(html) => (html.span.start, "before the HTML above the heading"),
            None => (heading.span.start, "before the heading"),
        };
        let message = format!(
            "the heading `{title}` and the table after it, with `Type` and `ID` or `Status` rows, \
             make an item, but it is not the first block of its region, so it reads as text of \
             what comes before. If it is an item, put a `---` line, with a blank line on each \
             side, {place}. If it is an example, put it in a fenced code block, or put a \
             paragraph between the heading and the table"
        );
        self.problem("missing-separator", start, message);
        // Read on as if the separator were there, so that what comes before ends at it.
        self.end_region(Some(start));
        self.document.unseparated.push(start);
        self.region.extend(html);
        self.region.push(heading);
    }

    /// Whether the last finished block of the region is a heading.
    fn after_heading(&self) -> bool {
        matches!(
            self.region.last(),
            Some(Block {
                kind: Kind::Heading(_),
                ..
            })
        )
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
            self.read_narrative(blocks, None);
        }
    }

    /// Reads a region that is no item as narrative units, each a heading and the blocks up to the
    /// next one, or the blocks before the first heading, and adds its mentions, except those in
    /// the block at `table`, to the narrative (DES-NARRATIVE-UNITS).
    fn read_narrative(&mut self, mut blocks: Vec<Block>, table: Option<usize>) {
        let mut start = 0;
        for end in 1..=blocks.len() {
            if end == blocks.len() || matches!(blocks[end].kind, Kind::Heading(_)) {
                let table = table.and_then(|table| table.checked_sub(start));
                self.read_unit(&mut blocks[start..end], table);
                start = end;
            }
        }
    }

    /// Adds the narrative unit of `blocks`, the first of which may be its heading, with the
    /// mentions of every block but the one at `table`.
    fn read_unit(&mut self, blocks: &mut [Block], table: Option<usize>) {
        let (Some(first), Some(last)) = (blocks.first(), blocks.last()) else {
            return;
        };
        let span = first.span.start..last.span.end;
        let heading = match &first.kind {
            Kind::Heading(title) => Some(title.clone()),
            _ => None,
        };
        let mentions = self.document.mentions.len();
        for (index, block) in blocks.iter_mut().enumerate() {
            if Some(index) != table {
                let links = std::mem::take(&mut block.links);
                let mentions = links.into_iter().filter(Link::is_mention);
                self.document.mentions.extend(mentions);
            }
        }
        let (mut headings, mut body) = (Vec::new(), Vec::new());
        for (index, block) in blocks.iter().enumerate() {
            match index == 0 && heading.is_some() {
                true => headings.push(block.plain.words()),
                false => body.push(block.plain.words()),
            }
            headings.push(block.headings.words());
        }
        let node = Node::Unit(self.document.units.len());
        self.add_words(node, [(Zone::Heading, headings), (Zone::Body, body)]);
        self.document.units.push(Unit {
            heading,
            line: self.lines.line(span.start),
            span,
            mentions: mentions..self.document.mentions.len(),
        });
    }

    /// Reads a region meant as an item, reporting each problem of its structure, and keeps it
    /// as an item when it has an anchor, a heading and a `Type` row (DES-ITEM-STRUCTURE).
    fn read_item(&mut self, head: Head, mut blocks: Vec<Block>, next: Option<usize>) {
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
        let (rows, values) = match head.table.map(|index| (index, &blocks[index])) {
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
            _ => (Vec::new(), Vec::new()),
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
        let (Some(Anchor { mid, .. }), Some(title), Some(heading), Some(table), true) = (
            &head.anchor,
            title,
            head.heading,
            head.table,
            missing.is_empty(),
        ) else {
            if !missing.is_empty() {
                let message = format!("the item {name} is not read: {}", missing.join("; "));
                self.problem("incomplete-item", start, message);
            }
            // Only the anchor is missing, and the heading comes first, so the anchor goes
            // right before it.
            let only_anchor = missing.len() == 1 && blocks[0].anchors.is_empty();
            if head.anchor.is_none() && head.heading == Some(0) && only_anchor {
                let end = blocks.last().map_or(start, |block| block.span.end);
                self.document.anchorless.push(Anchorless {
                    span: start..end,
                    heading: blocks[0].span.clone(),
                    rows,
                    line: self.lines.line(start),
                });
            }
            return self.read_narrative(blocks, head.table);
        };

        let end = blocks.last().map_or(start, |block| block.span.end);
        let body = match blocks.get(table + 1) {
            Some(first) => first.span.start..end,
            None => end..end,
        };
        let mid = mid.clone();
        // Text before a merged anchor is narrative: a unit of its own. The text after it in its
        // paragraph starts the item's body (DES-NARRATIVE-UNITS).
        let merged = head.anchor.as_ref().filter(|anchor| !anchor.exact);
        let prefix = merged.map_or(0, |anchor| anchor.before);
        let first = &blocks[0];
        let before = trim(self.text, &(first.span.start..start));
        let lead = match merged {
            Some(anchor) => {
                // The anchor is exactly `<a id="<MID>"></a>` (DES-MID-ANCHOR).
                let anchor_end = start + format!("<a id=\"{}\"></a>", anchor.mid).len();
                trim(self.text, &(anchor_end.min(first.span.end)..first.span.end))
            }
            None => start..start,
        };
        let narrative = self.document.mentions.len();
        let mut mentions = Vec::new();
        for (index, block) in blocks.iter_mut().enumerate() {
            if index != table {
                let links = std::mem::take(&mut block.links);
                for link in links.into_iter().filter(Link::is_mention) {
                    // Text before a merged anchor is narrative.
                    if link.span.start < start {
                        self.document.mentions.push(link);
                    } else {
                        mentions.push(link);
                    }
                }
            }
        }
        if !before.is_empty() {
            let node = Node::Unit(self.document.units.len());
            self.add_words(node, [(Zone::Body, vec![blocks[0].plain.until(prefix)])]);
            self.document.units.push(Unit {
                heading: None,
                line: self.lines.line(before.start),
                span: before,
                mentions: narrative..self.document.mentions.len(),
            });
        }
        let words = ItemWords {
            mid: &mid,
            anchor: start,
            heading: &blocks[heading],
            rows: &rows,
            values: &values,
            lead: merged.map(|_| blocks[0].plain.after(prefix)),
            body: &blocks[table + 1..],
        };
        self.add_item_words(&words);
        self.document.items.push(Item {
            mid,
            title,
            rows,
            mentions,
            span: start..end,
            line: self.lines.line(start),
            end_line: self.lines.line(end.saturating_sub(1).max(start)),
            lead,
            table: blocks[table].span.clone(),
            body,
        });
    }

    // @mara implements REQ-INVALID-ROWS-REPORTED
    /// The rows of the metadata table `table` that have a key and a value, reporting malformed
    /// ones, with the text of each value and where its pieces start. `next` is where the block
    /// after the table
    /// starts, if any (DES-METADATA-TABLE).
    fn metadata_rows(
        &mut self,
        table: &Block,
        rows: &[TableRow],
        next: Option<usize>,
    ) -> (Vec<Row>, Vec<Prose>) {
        let mut read = Vec::with_capacity(rows.len());
        let mut values = Vec::with_capacity(rows.len());
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
                text: value
                    .plain
                    .text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                link,
                line: self.lines.line(row.span.start),
                span: trim(self.text, &row.span),
                value_span: value.span.clone(),
            });
            values.push(value.plain.clone());
        }
        if let Some(next) = next
            && self.lines.line(next) == self.lines.line(table.span.end.saturating_sub(1)) + 1
        {
            let message =
                "the metadata table ends at a blank line, but this line follows it directly"
                    .to_owned();
            self.problem("malformed-row", next, message);
        }
        (read, values)
    }

    /// Adds the search words of the next item, zone by zone (DES-SNAPSHOT). Every text comes
    /// from the event pass: no link destination and no HTML.
    fn add_item_words(&mut self, item: &ItemWords<'_>) {
        let node = Node::Item(self.document.items.len());
        // The MID stands after `<a id="` in its anchor.
        let mid = item.anchor + "<a id=\"".len();
        let anchor = [Mark {
            at: 0,
            source: (mid, mid + item.mid.len()),
            exact: true,
        }];
        let rows = |id: bool| -> Vec<Words<'_>> {
            let rows = item.rows.iter().zip(item.values);
            rows.filter(|(row, _)| (row.name == "id") == id)
                .map(|(_, value)| value.words())
                .collect()
        };
        let mut ids = vec![Words {
            text: item.mid,
            start: 0,
            marks: &anchor,
        }];
        ids.extend(rows(true));
        let (mut headings, mut other) = (Vec::new(), Vec::new());
        other.extend(item.lead);
        for block in item.body {
            match block.kind {
                Kind::Heading(_) => headings.push(block.plain.words()),
                _ => other.push(block.plain.words()),
            }
            headings.push(block.headings.words());
        }
        self.add_words(
            node,
            [
                (Zone::Id, ids),
                (Zone::Title, vec![item.heading.plain.words()]),
                (Zone::Heading, headings),
                (Zone::Metadata, rows(false)),
                (Zone::Body, other),
            ],
        );
    }

    /// Adds the words of `node`'s texts, zone by zone in zone order, so that each word's
    /// postings stay in node order, then zone order. A posting keeps the source offset of the
    /// word's first occurrence in its zone.
    fn add_words<const N: usize>(&mut self, node: Node, zones: [(Zone, Vec<Words<'_>>); N]) {
        for (zone, texts) in zones {
            for text in texts {
                let part = &text.text[text.start..];
                for (at, word) in word_spans(part).map(|(at, word)| (text.start + at, word)) {
                    let word = word.to_lowercase();
                    let postings = match self.words.get_mut(&word) {
                        Some(postings) => postings,
                        None => self.words.entry(word).or_default(),
                    };
                    let seen = postings.last();
                    if seen.is_none_or(|last| (last.node, last.zone) != (node, zone)) {
                        // A parsed document holds at most `MAX_DOCUMENT_BYTES` bytes.
                        let at = u32::try_from(text.source(at))
                            .expect("an offset in a parsed document fits 32 bits");
                        postings.push(Posting { node, zone, at });
                    }
                }
            }
        }
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

/// A run of rendered text, and the source range it was read from: exactly its characters when
/// `exact`, else something CommonMark renders as it.
struct Run {
    rendered: Range<usize>,
    source: Range<usize>,
    exact: bool,
}

/// The run of the rendered text `rendered`, read from `source` in some way not followed here.
fn inexact(rendered: Range<usize>, source: Range<usize>) -> Run {
    Run {
        rendered,
        source,
        exact: false,
    }
}

/// Aligns `rendered`, the text the Markdown parser read from `source`, a range of `text` within
/// one line or a code block's lines, with that range, by the only differences CommonMark allows
/// there: a backslash escape read as its character (`\|` read as `|` in a table cell) and, in a
/// code span (`code`), one space stripped at each end when both ends hold one and not every
/// character is one. The runs it returns are the whole rendered text, each run that equals its
/// source exact. Rendered text that does not follow from its source by these rules, such as a
/// decoded entity, is one run at the start of its source.
fn align(text: &str, rendered: &str, source: Range<usize>, code: bool) -> Vec<Run> {
    let whole = || vec![inexact(0..rendered.len(), source.clone())];
    let Some(slice) = text.get(source.clone()) else {
        return whole();
    };
    // Each character of the source, with its range.
    let units: Vec<(Range<usize>, char)> = slice
        .char_indices()
        .map(|(at, c)| (source.start + at..source.start + at + c.len_utf8(), c))
        .collect();
    let mut units = &units[..];
    let space = |unit: &(Range<usize>, char)| unit.1 == ' ';
    if code
        && units.len() >= 2
        && space(&units[0])
        && space(&units[units.len() - 1])
        && !units.iter().all(space)
    {
        units = &units[1..units.len() - 1];
    }
    let mut runs: Vec<Run> = Vec::new();
    let mut next = 0;
    for (at, c) in rendered.char_indices() {
        let end = at + c.len_utf8();
        let Some((range, char)) = units.get(next) else {
            return whole();
        };
        if *char == c {
            match runs.last_mut() {
                Some(run) if run.rendered.end == at && run.source.end == range.start => {
                    run.rendered.end = end;
                    run.source.end = range.end;
                }
                _ => runs.push(Run {
                    rendered: at..end,
                    source: range.clone(),
                    exact: true,
                }),
            }
            next += 1;
        } else if *char == '\\' && units.get(next + 1).is_some_and(|escaped| escaped.1 == c) {
            runs.push(Run {
                rendered: at..end,
                source: units[next + 1].0.clone(),
                exact: true,
            });
            next += 2;
        } else {
            return whole();
        }
    }
    if next != units.len() {
        return whole();
    }
    runs
}

/// What an item's search words come from.
struct ItemWords<'i> {
    mid: &'i str,
    /// Where the item, and its anchor, start.
    anchor: usize,
    heading: &'i Block,
    rows: &'i [Row],
    /// The text of each row's value.
    values: &'i [Prose],
    /// The text after a merged anchor in its paragraph.
    lead: Option<Words<'i>>,
    body: &'i [Block],
}

/// Whether a table after a heading that is not the first block of its region makes an item
/// that lacks its separator: its first row is a `Type` row, and it has an `ID` or a `Status`
/// row, whatever their values (DES-REGIONS). A table of field types with a `Type` column and
/// neither row is no item's.
fn is_item_table(rows: &[TableRow]) -> bool {
    let key = |row: &TableRow| row.cells.first().map(|key| label_name(&key.text));
    rows.first().is_some_and(TableRow::is_type_row)
        && rows[1..]
            .iter()
            .any(|row| matches!(key(row).as_deref(), Some("id" | "status")))
}

/// `span` without the spaces, tabs and line endings around its text.
fn trim(text: &str, span: &Range<usize>) -> Range<usize> {
    let source = &text[span.clone()];
    let blank = [' ', '\t', '\r', '\n'];
    let start = span.start + (source.len() - source.trim_start_matches(blank).len());
    let end = span.end - (source.len() - source.trim_end_matches(blank).len());
    start..end.max(start)
}

/// Whether `tag` is inline, so that its text continues the text around it.
fn is_inline(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. }
    )
}

fn is_inline_end(tag: &TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::Link
            | TagEnd::Image
    )
}
